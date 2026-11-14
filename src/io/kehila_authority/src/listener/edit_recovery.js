"use strict";

// One origin/profile journal. Credentials stay in the live transport, never here.
globalThis.KehilaEditRecoveryStore = ({indexedDB, timeoutMs = 5000} = {}) => {
  const databaseName = "kehila-edit-recovery", table = "journal", key = "current";
  let opening = null;
  const unavailable = () => new Error("Edit recovery storage could not be confirmed. Reload before saving; no replacement was sent.");
  function open() {
    if (opening) return opening;
    opening = new Promise((resolve, reject) => {
      let request, finished = false;
      const fail = () => { if (!finished) { finished = true; clearTimeout(timer); reject(unavailable()); } };
      const timer = setTimeout(fail, timeoutMs);
      try {
        request = (indexedDB || globalThis.indexedDB).open(databaseName, 1);
        request.onupgradeneeded = () => {
          if (finished) { request.transaction.abort(); return; }
          request.result.createObjectStore(table);
        };
        request.onerror = fail; request.onblocked = fail;
        request.onsuccess = () => {
          const database = request.result;
          if (finished) { database.close(); return; }
          if (database.objectStoreNames.length !== 1 || !database.objectStoreNames.contains(table)) {
            database.close(); fail(); return;
          }
          finished = true; clearTimeout(timer);
          database.onversionchange = () => { database.close(); opening = null; };
          database.onclose = () => { opening = null; };
          resolve(database);
        };
      } catch (_) { fail(); }
    });
    // A rejected open stays rejected for this page; it cannot later start work.
    return opening;
  }
  async function transaction(write, expected, replacement, admit = () => {}) {
    const database = await open();
    return new Promise((resolve, reject) => {
      let tx, finished = false, result, rejection = unavailable();
      const fail = () => { if (!finished) { finished = true; clearTimeout(timer); reject(rejection); } };
      const timer = setTimeout(() => { try { tx?.abort(); } catch (_) {} fail(); }, timeoutMs);
      try {
        tx = write ? database.transaction(table, "readwrite", {durability:"strict"}) : database.transaction(table, "readonly");
        tx.onabort = fail; tx.onerror = fail;
        tx.oncomplete = () => { if (!finished) { finished = true; clearTimeout(timer); resolve(result); } };
        if (write && tx.durability !== "strict") { tx.abort(); fail(); return; }
        const objectStore = tx.objectStore(table), request = objectStore.get(key);
        request.onerror = () => { try { tx.abort(); } catch (_) {} fail(); };
        request.onsuccess = () => {
          if (finished) return;
          const current = request.result === undefined ? null : request.result;
          if (!write) { result = current; return; }
          result = current === expected;
          if (result) {
            try { admit(); objectStore.put(replacement, key); }
            catch (error) { rejection = error; try { tx.abort(); } catch (_) {} fail(); }
          }
        };
      } catch (_) { try { tx?.abort(); } catch (_) {} fail(); }
    });
  }
  return {read:() => transaction(false), compare:(expected, replacement, admit) => transaction(true, expected, replacement, admit)};
};

globalThis.KehilaEditRecovery = ({store = globalThis.KehilaEditRecoveryStore(), now = Date.now,
  crypto = globalThis.crypto} = {}) => {
  const encoder = new TextEncoder(), lifetime = 7 * 86400000, wireLimit = 1048576, metadataLimit = 2048;
  const failure = message => { throw new Error(message); };
  class InvalidRecord extends Error {}
  const invalid = () => { throw new InvalidRecord("Invalid or unsupported edit recovery record."); };
  const object = value => value !== null && typeof value === "object" && !Array.isArray(value);
  const keys = (value, names) => object(value) && Object.keys(value).length === names.length && names.every(name => Object.hasOwn(value, name));
  const bytes = value => encoder.encode(value).length;
  const require = value => { if (!value) invalid(); };
  function text(value, maximum) {
    require(typeof value === "string" && !value.includes("\0"));
    for (const symbol of value) {
      const code = symbol.charCodeAt(0);
      require(symbol.length !== 1 || code < 0xd800 || code > 0xdfff);
    }
    require(bytes(value) <= maximum); return value;
  }
  const id = value => typeof value === "string" && value.length > 0 && text(value, 128) === value;
  const uint = (value, zero = false, maximum = "18446744073709551615") => typeof value === "string" && /^(0|[1-9][0-9]*)$/.test(value)
    && (zero || value !== "0") && (value.length < maximum.length || value.length === maximum.length && value <= maximum);
  const amount = value => value === null || uint(value, true, "999999999");
  const timestamp = value => Number.isSafeInteger(value) && value >= 0;
  function binding(value) { require(keys(value, ["installation_id","actor_id"]) && id(value.installation_id) && id(value.actor_id)); }
  const sameOwner = (a, b) => a.installation_id === b.installation_id && a.actor_id === b.actor_id;
  function retained(value) {
    require(keys(value, ["binding","project","item","family","version","configuration","lifecycle","estimate","wire"]));
    binding(value.binding);
    require(value.family === "item_edit" && id(value.project) && id(value.item) && uint(value.version) && uint(value.configuration) && amount(value.estimate));
    const state = value.lifecycle;
    require(keys(state, ["type","workflow","status","archived","phase"]) && id(state.type) && id(state.workflow) && id(state.status)
      && typeof state.archived === "boolean" && ["new","active","done"].includes(state.phase));
    text(value.wire, wireLimit);
    let body;
    try { body = JSON.parse(value.wire); } catch (_) { invalid(); }
    require(keys(body, ["project_id","item_id","operation_id","expected_item_version","expected_configuration_revision","estimate","fields"])
      && body.project_id === value.project && body.item_id === value.item && typeof body.operation_id === "string" && /^[0-9a-f]{32}$/.test(body.operation_id)
      && body.expected_item_version === value.version && body.expected_configuration_revision === value.configuration);
    const estimate = body.estimate;
    require(object(estimate) && ["keep","clear","set"].includes(estimate.action));
    require(estimate.action === "set" ? keys(estimate, ["action","value"]) && uint(estimate.value, true, "999999999") : keys(estimate, ["action"]));
    require(Array.isArray(body.fields) && body.fields.length <= 128);
    const seen = new Set();
    for (const field of body.fields) {
      require(object(field) && id(field.id) && !seen.has(field.id)); seen.add(field.id);
      require(["keep","clear","set"].includes(field.action));
      require(keys(field, field.action === "set" ? ["id","action","value"] : ["id","action"]));
      if (field.action === "set") text(field.value, 16384);
    }
  }
  const clone = value => JSON.parse(JSON.stringify(value));
  function freeze(value) {
    if (object(value) || Array.isArray(value)) { for (const child of Object.values(value)) freeze(child); Object.freeze(value); }
    return value;
  }
  const hex = value => Array.from(value, byte => byte.toString(16).padStart(2, "0")).join("");
  const generation = () => hex(crypto.getRandomValues(new Uint8Array(16)));
  const hash = async value => hex(new Uint8Array(await crypto.subtle.digest("SHA-256", encoder.encode(value))));
  const time = () => { const value = now(); require(timestamp(value)); return value; };
  function validate(root) {
    require(keys(root, ["schema","generation","observed_ms","entry","checksum"]) && root.schema === 1
      && typeof root.generation === "string" && /^[0-9a-f]{32}$/.test(root.generation) && timestamp(root.observed_ms)
      && typeof root.checksum === "string" && /^[0-9a-f]{64}$/.test(root.checksum));
    if (root.entry !== null) {
      const entry = root.entry;
      require(keys(entry, ["created_ms","expires_ms","retry_closed","retained"]) && timestamp(entry.created_ms) && timestamp(entry.expires_ms)
        && entry.expires_ms - entry.created_ms === lifetime && entry.created_ms <= root.observed_ms && typeof entry.retry_closed === "boolean");
      retained(entry.retained);
    }
    const metadata = clone(root);
    if (metadata.entry) metadata.entry.retained.wire = "";
    require(bytes(JSON.stringify(metadata)) <= metadataLimit);
  }
  const payload = root => JSON.stringify({schema:root.schema, generation:root.generation, observed_ms:root.observed_ms, entry:root.entry});
  async function encode(root) {
    const snapshot = clone(root);
    snapshot.checksum = "0".repeat(64); validate(snapshot);
    snapshot.checksum = await hash(payload(snapshot));
    return JSON.stringify(snapshot);
  }
  async function decode(raw) {
    require(typeof raw === "string" && bytes(raw) <= 2 * wireLimit + metadataLimit + 1024);
    let root;
    try { root = JSON.parse(raw); } catch (_) { invalid(); }
    validate(root); require(await hash(payload(root)) === root.checksum); return root;
  }
  function checkClock(root) { if (time() < root.observed_ms) failure("The local clock moved backwards. Edit recovery is unavailable until its time can be checked."); }
  const empty = (observed = time()) => ({schema:1, generation:generation(), observed_ms:observed, entry:null, checksum:"0".repeat(64)});
  async function replace(raw, root, admit) {
    const encoded = await encode(root);
    if (!await store.compare(raw, encoded, () => {
      if (time() < root.observed_ms) failure("The local clock moved backwards before edit recovery admission.");
      admit?.();
    })) failure("Edit recovery changed in another tab. Restore it before preparing or retrying work.");
    return root;
  }
  async function open(owner, ticket) {
    binding(owner); owner = freeze(clone(owner));
    let raw = await store.read(), root, notice = "";
    if (ticket !== undefined) {
      require(keys(ticket, ["raw"]));
      if (ticket.raw !== raw) {
        let before, after;
        try { before = await decode(ticket.raw); after = await decode(raw); }
        catch (_) { failure("Edit recovery changed during sign-in. Restore it again."); }
        if (before.generation !== after.generation) failure("Edit recovery changed during sign-in. The old binding is stale.");
      }
    }
    if (raw === null) root = await replace(null, empty());
    else {
      try { root = await decode(raw); }
      catch (error) {
        if (!(error instanceof InvalidRecord)) throw error;
        notice = "An invalid or unsupported recovery record was discarded. Losing tracking does not cancel work that may have completed.";
        root = await replace(raw, empty());
      }
      checkClock(root);
      if (root.entry && (!sameOwner(root.entry.retained.binding, owner) || time() >= root.entry.expires_ms)) {
        notice = sameOwner(root.entry.retained.binding, owner)
          ? "The seven-day edit recovery record expired. Losing tracking does not cancel a save or prove rollback."
          : "The installation or owner changed. The foreign recovery record was discarded without resending it.";
        root = await replace(raw, empty(Math.max(time(), root.observed_ms)));
      }
    }
    let currentGeneration = root.generation, observed = root.observed_ms;
    function handle(current) { return current.entry === null ? null : freeze({generation:current.generation,
      retained:clone(current.entry.retained), retryClosed:current.entry.retry_closed}); }
    async function read(expected, freshOwner = owner, allowExpired = false) {
      binding(freshOwner);
      if (!sameOwner(owner, freshOwner)) failure("The installation or owner changed. No old save was resent.");
      const value = await store.read();
      if (value === null) failure("Edit recovery changed or was cleared. Restore before continuing.");
      const current = await decode(value);
      const checkedAt = time();
      if (checkedAt < Math.max(observed, current.observed_ms)) failure("The local clock moved backwards. Edit recovery is unavailable until its time can be checked.");
      observed = checkedAt;
      if (current.generation !== currentGeneration || expected && expected.generation !== current.generation)
        failure("Edit recovery changed in another tab. The old action is stale.");
      if (current.entry && !sameOwner(current.entry.retained.binding, owner)) failure("The installation or owner changed.");
      if (expected && (!current.entry || JSON.stringify(expected.retained) !== JSON.stringify(current.entry.retained)))
        failure("The retained edit changed. The old action is stale.");
      if (!allowExpired && current.entry && time() >= current.entry.expires_ms) failure("The seven-day edit recovery record expired.");
      return {raw:value, root:current};
    }
    async function write(snapshot, changed, allowExpired = false) {
      await replace(snapshot.raw, changed, () => {
        // Runs synchronously at the actual readwrite admission point, after
        // hashing and transaction queuing. Completion is still required to send.
        const admittedAt = time();
        if (changed.observed_ms < snapshot.root.observed_ms || admittedAt < observed || admittedAt < changed.observed_ms)
          failure("The local clock moved backwards before edit recovery admission.");
        if (!allowExpired && changed.entry && admittedAt >= changed.entry.expires_ms) failure("The seven-day edit recovery record expired before admission.");
      });
      currentGeneration = changed.generation; observed = Math.max(observed, changed.observed_ms); return handle(changed);
    }
    return {
      get notice() { return notice; },
      async restore() {
        const snapshot = await read(null, owner, true);
        // Empty observations need no durable mutation. This session still
        // remembers their clock high-water mark; another page sees durable time.
        if (snapshot.root.entry === null) return null;
        if (snapshot.root.entry && time() >= snapshot.root.entry.expires_ms) {
          notice = "The seven-day edit recovery record expired. Losing tracking does not cancel work.";
          return write(snapshot, empty());
        }
        return write(snapshot, {...snapshot.root, observed_ms:time()});
      },
      async retain(value) {
        retained(value); value = freeze(clone(value));
        if (!sameOwner(value.binding, owner)) invalid();
        const snapshot = await read(null);
        if (snapshot.root.entry) failure("An unresolved save cannot be overwritten. Check it or explicitly stop tracking it.");
        const created = time(); require(created <= Number.MAX_SAFE_INTEGER - lifetime);
        return write(snapshot, {...snapshot.root, observed_ms:created,
          entry:{created_ms:created, expires_ms:created + lifetime, retry_closed:false, retained:value}});
      },
      async admit(expected, freshOwner, kind) {
        require(["check","retry"].includes(kind));
        const snapshot = await read(expected, freshOwner);
        if (kind === "retry" && snapshot.root.entry.retry_closed) failure("Retry is closed for this expired server replay identity.");
        const admitted = await write(snapshot, {...snapshot.root, observed_ms:time()});
        return admitted.retained;
      },
      async current(expected, freshOwner) {
        try { await read(expected, freshOwner, true); return true; }
        catch (error) { if (/changed|cleared|stale|expired/.test(error.message)) return false; throw error; }
      },
      async closeRetry(expected, freshOwner) {
        const snapshot = await read(expected, freshOwner, true);
        await write(snapshot, {...snapshot.root, observed_ms:time(), entry:{...snapshot.root.entry, retry_closed:true}}, true);
      },
      async settle(expected, freshOwner) { const snapshot = await read(expected, freshOwner, true); await write(snapshot, empty()); },
      async discard(expected, freshOwner) {
        if (!expected) failure("Select the saved edit before stopping tracking it.");
        binding(freshOwner);
        const snapshot = await read(expected, freshOwner, true);
        await write(snapshot, empty());
      }
    };
  }
  async function clear(owner) {
    binding(owner);
    const raw = await store.read();
    if (raw === null) { await replace(null, empty()); return; }
    let root;
    try { root = await decode(raw); }
    catch (error) {
      if (!(error instanceof InvalidRecord)) throw error;
      await replace(raw, empty()); return;
    }
    // A delayed old-owner signout must not erase a new owner's record.
    if (root.entry && !sameOwner(root.entry.retained.binding, owner)) return;
    await replace(raw, empty(Math.max(time(), root.observed_ms)));
  }
  // Capture before the fresh binding request, so a delayed reply cannot reopen
  // a generation invalidated by signout in another tab.
  return {prepare:async () => freeze({raw:await store.read()}), open, clear};
};
