"use strict";
// A current read and an original command receipt are separate observations.
globalThis.KehilaCurrentWork = ({document, request, requestWire, run, checkBinding,
  recovery = globalThis.KehilaSelectionRecovery(),
  randomBytes = count => globalThis.crypto.getRandomValues(new Uint8Array(count))}) => {
  const el = id => document.getElementById(id), encoder = new TextEncoder();
  const invalid = () => { throw new Error("Invalid or mismatched current-work response; outcome remains unconfirmed."); };
  const require = value => { if (!value) invalid(); };
  const record = value => value !== null && typeof value === "object" && !Array.isArray(value);
  const keys = (value, names) => record(value) && Object.keys(value).length === names.length && names.every(name => Object.hasOwn(value,name));
  const id = value => typeof value === "string" && value.length > 0 && !value.includes("\0")
    && !Array.from(value).some(symbol => symbol.length === 1 && /[\ud800-\udfff]/.test(symbol)) && encoder.encode(value).length <= 128;
  const uint = value => typeof value === "string" && /^(0|[1-9][0-9]*)$/.test(value)
    && (value.length < 20 || value.length === 20 && value <= "18446744073709551615");
  const target = value => keys(value,["project_id","item_id"]) && id(value.project_id) && id(value.item_id);
  const sameTarget = (a,b) => a === null || b === null ? a === b : a.project_id === b.project_id && a.item_id === b.item_id;
  const sameOwner = (a,b) => a.installation_id === b.installation_id && a.actor_id === b.actor_id;
  const ownerValue = value => keys(value,["installation_id","actor_id"]) && id(value.installation_id) && id(value.actor_id);
  const label = value => value === null ? "No current work item." : "Project: " + value.project_id + " / Work item: " + value.item_id;
  let busy = false, active = false, working = false, restoring = false, epoch = 0;
  let suspended = true, blocked = false, cleanupFailed = false, signedOut = false;
  let owner = null, session = null, retained = null, retryClosed = false, current = null, offered = null;
  const message = value => { el("current-work-status").textContent = value; };
  const recoveryMessage = value => { el("selection-recovery-status").textContent = value; };
  function controls(nextBusy = busy, nextActive = active) {
    busy = nextBusy; active = nextActive;
    const unavailable = busy || working || restoring || !active || suspended || signedOut;
    const mutation = unavailable || blocked || !session || retained !== null || current === null;
    el("current-work-refresh").disabled = unavailable;
    el("item-select-current").disabled = mutation || offered === null;
    el("current-work-clear").disabled = mutation || current?.target.state === "none";
    el("selection-check").disabled = unavailable || blocked || retained === null;
    el("selection-retry").disabled = unavailable || blocked || retained === null || retryClosed;
    el("selection-forget").disabled = unavailable || blocked || retained === null;
    el("selection-recovery").hidden = retained === null && !blocked;
  }
  function clearView() {
    current = null; offered = null;
    el("current-work-detail").textContent = "Refresh to read your current work item.";
    el("selection-receipt").textContent = "";
  }
  function startSession() {
    epoch++; signedOut = false; restoring = false; suspended = true; blocked = cleanupFailed;
    owner = null; session = null; retained = null; retryClosed = false; clearView(); controls();
  }
  function suspendRecovery() {
    epoch++; suspended = true; restoring = false; session = null; current = null; offered = null; controls();
  }
  async function checkedOwner(observed) {
    const value = await checkBinding();
    if (observed !== epoch) return null;
    require(ownerValue(value));
    if (owner && !sameOwner(owner,value)) {
      startSession();
      throw new Error("The installation or owner changed. Restore recovery before continuing.");
    }
    return Object.freeze({...value});
  }
  async function restoreRecovery() {
    if (cleanupFailed || signedOut) { blocked = true; controls(); return false; }
    const observed = epoch; restoring = true; controls();
    try {
      const ticket = await recovery.prepare();
      if (observed !== epoch) return false;
      const binding = await checkedOwner(observed);
      if (!binding) return false;
      const opened = await recovery.open(binding,ticket);
      if (observed !== epoch) return false;
      const saved = await opened.restore();
      if (observed !== epoch) return false;
      owner = binding; session = opened; retained = saved; retryClosed = saved?.retryClosed ?? false;
      if (saved) el("selection-receipt").textContent = "";
      blocked = false; suspended = false;
      recoveryMessage(saved ? "An exact current-work request was recovered. Check its outcome or explicitly retry it; nothing was resent."
        : opened.notice || "No current-work request is being tracked in this browser profile.");
      return true;
    } catch (_) {
      if (observed !== epoch) return false;
      blocked = true;
      recoveryMessage("Current-work recovery storage or owner binding could not be confirmed. No request was sent.");
      return false;
    } finally { if (observed === epoch) { restoring = false; controls(); } }
  }
  async function journalCleanup(work) {
    const observed = epoch;
    try { return await work(); }
    catch (_) {
      // A failed confirmation may follow a committed local transaction. Keep
      // this latch across ordinary restoration and later sign-ins in this page.
      cleanupFailed = true; blocked = true; controls();
      if (observed === epoch) recoveryMessage("Local current-work recovery cleanup could not be confirmed. Reload and inspect the saved identity before continuing.");
      throw new Error("Local current-work recovery cleanup could not be confirmed.");
    }
  }
  async function signOut(binding) {
    epoch++; signedOut = true; suspended = true; restoring = false; blocked = true;
    session = null; retained = null; owner = null; clearView(); controls();
    const observed = epoch;
    try {
      if (!ownerValue(binding)) throw new Error("Owner binding unavailable for recovery cleanup.");
      await recovery.clear(binding);
      if (observed === epoch) recoveryMessage("Local selection tracking was cleared. This does not cancel a request or prove server signout.");
    } catch (error) {
      cleanupFailed = true; blocked = true;
      if (observed === epoch) recoveryMessage("Local selection recovery deletion could not be confirmed. Reload before preparing another choice.");
      throw error;
    } finally { controls(); }
  }
  function offerItem(value) {
    offered = value && value.archived === false && id(value.project_id) && id(value.item_id)
      ? {project_id:value.project_id,item_id:value.item_id} : null;
    controls();
  }
  async function refresh() {
    const observed = epoch; current = null; controls();
    const binding = await checkedOwner(observed);
    if (!binding) return;
    const reply = await request("/selection/read",{});
    if (observed !== epoch) return;
    require(reply.status === 200);
    const data = reply.data;
    require(keys(data,["codec_version","outcome","version","target"]) && data.codec_version === 1
      && data.outcome === "read" && uint(data.version));
    const value = data.target;
    require(keys(value,["state"]) && ["none","unavailable"].includes(value.state)
      || keys(value,["state","project_id","item_id"]) && value.state === "available" && id(value.project_id) && id(value.item_id));
    require(data.version !== "0" || value.state === "none");
    owner = binding; current = data;
    el("current-work-detail").textContent = value.state === "available" ? label(value)
      : value.state === "none" ? label(null) : "Your current work item is unavailable to read. You can clear this choice.";
    message("Current selection version " + data.version + " read separately from any saved receipt."); controls();
  }
  function success(data, command, binding, kind) {
    require(keys(data,["codec_version","outcome","selection"]) && data.codec_version === 1
      && (data.outcome === "replayed" || kind === "retry" && data.outcome === "accepted"));
    const value = data.selection;
    require(keys(value,["user_id","version","requested"]) && value.user_id === binding.actor_id && uint(value.version)
      && (value.requested === null || target(value.requested)) && sameTarget(value.requested,command.requested));
    const version = BigInt(value.version), expected = BigInt(command.expected_version);
    require(version === expected || version === expected + 1n);
    require(value.version !== "0" || value.requested === null);
    return value;
  }
  async function transmit(kind) {
    const observed = epoch, saved = retained, opened = session;
    if (!saved || !opened || suspended || blocked || kind === "retry" && retryClosed) return;
    const binding = await checkedOwner(observed);
    if (!binding || retained !== saved) return;
    const value = await opened.admit(saved,binding,kind);
    if (observed !== epoch || retained !== saved) return;
    const reply = await requestWire(kind === "check" ? "/selection/reconcile" : "/selection/set",value.wire);
    if (observed !== epoch || retained !== saved || !await opened.current(saved,binding)) return;
    const data = reply.data;
    if (reply.status === 200 && data?.outcome === "absent" && kind === "check") {
      require(keys(data,["codec_version","outcome"]) && data.codec_version === 1);
      recoveryMessage("Absent at this lookup. An in-flight request may still commit; an explicit retry uses the same exact request."); return;
    }
    if (reply.status === 410 && data?.outcome === "replay_expired") {
      require(keys(data,["codec_version","outcome"]) && data.codec_version === 1);
      await journalCleanup(() => opened.closeRetry(saved,binding));
      if (observed !== epoch) return;
      retryClosed = true; recoveryMessage("Recorded identity replay expired. Retry is closed; read current work separately."); return;
    }
    if ([404,409].includes(reply.status) && data?.outcome === "rejected") {
      const conflict = reply.status === 409 && data.code === "selection_version_conflict";
      const archived = reply.status === 409 && data.code === "archived_target";
      const missing = reply.status === 404 && data.code === "invalid_reference";
      const reused = reply.status === 409 && data.code === "operation_id_reused";
      require(keys(data,conflict ? ["codec_version","outcome","code","current_version"] : ["codec_version","outcome","code"])
        && data.codec_version === 1 && (conflict && uint(data.current_version) || archived || missing || reused));
      await journalCleanup(() => opened.closeRetry(saved,binding));
      if (observed !== epoch) return;
      retryClosed = true; current = null;
      message(conflict ? "The selection changed. Refresh before making a separate choice; the saved request was not rebased."
        : archived ? "This work item was archived before selection. Its request was rejected and retry is closed."
        : missing ? "The work item could not be found. This request was rejected and retry is closed."
        : "The operation identity was reused. Its retry is closed; no replacement was sent.");
      recoveryMessage("This request was rejected. You may check its identity or explicitly stop tracking it."); return;
    }
    require(reply.status === 200);
    const result = success(data,JSON.parse(value.wire),binding,kind);
    await journalCleanup(() => opened.settle(saved,binding));
    if (observed !== epoch || retained !== saved) return;
    retained = null; retryClosed = false;
    el("selection-receipt").textContent = "Original " + data.outcome + " result, version " + result.version + ": " + label(result.requested);
    recoveryMessage("The original request result was confirmed. Current work is read separately.");
    try { await refresh(); }
    catch (_) { if (observed === epoch) { current = null; message("Original result confirmed; the separate current read is unavailable. Refresh to check it."); } }
  }
  async function choose(value) {
    if (!current || !session || retained || blocked || suspended) return;
    const observed = epoch, opened = session;
    const binding = await checkedOwner(observed);
    if (!binding) return;
    const random = Array.from(randomBytes(16));
    require(random.length === 16 && random.every(byte => Number.isInteger(byte) && byte >= 0 && byte <= 255));
    const command = {operation_token:random.map(byte => byte.toString(16).padStart(2,"0")).join(""),
      expected_version:current.version,requested:value};
    let saved;
    try { saved = await opened.retain({binding,family:"selection",wire:JSON.stringify(command)}); }
    catch (error) {
      // An unconfirmed local commit cannot establish an empty journal slot.
      if (observed === epoch) blocked = true;
      throw error;
    }
    if (observed !== epoch) return;
    retained = saved; retryClosed = false; el("selection-receipt").textContent = ""; controls();
    await transmit("retry");
  }
  async function forget() {
    const observed = epoch, saved = retained, opened = session;
    if (!saved || !opened) return;
    const binding = await checkedOwner(observed);
    if (!binding) return;
    await journalCleanup(() => opened.discard(saved,binding));
    if (observed !== epoch || retained !== saved) return;
    retained = null; retryClosed = false; current = null;
    const notice = "Stopped tracking this request. This does not cancel it or prove rollback; it may have committed. Refresh before a separate choice.";
    message(notice); recoveryMessage(notice);
  }
  const action = work => {
    if (working || busy || !active || suspended || signedOut) return;
    return run(async () => {
      const observed = epoch; working = true; controls();
      try { await work(); }
      catch (_) {
        if (observed === epoch) {
          const text = cleanupFailed ? "Local current-work recovery cleanup could not be confirmed. Reload and inspect the same saved identity; the request may have committed."
            : retained ? "Current-work outcome remains unconfirmed. Check or explicitly retry the exact retained request."
            : "Current-work read or recovery storage commit could not be confirmed. No replacement request was sent.";
          message(text); recoveryMessage(text);
        }
      } finally { working = false; controls(); }
    });
  };
  for (const [name,work] of [["current-work-refresh",refresh],["item-select-current",() => offered && choose({...offered})],
    ["current-work-clear",() => choose(null)],["selection-check",() => transmit("check")],
    ["selection-retry",() => transmit("retry")],["selection-forget",forget]]) {
    el(name).addEventListener("click",() => action(work));
  }
  clearView(); controls();
  return {controls,offerItem,restoreRecovery,startSession,suspendRecovery,signOut};
};
