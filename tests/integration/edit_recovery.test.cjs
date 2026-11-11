// Journal policy with a transactional port; not IndexedDB or browser qualification.
const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const {webcrypto} = require('node:crypto');
const path = require('node:path');
const memoryStore = require('./journal_store.cjs');
const context = {TextEncoder, TextDecoder, crypto:webcrypto, setTimeout, clearTimeout};
vm.createContext(context);
vm.runInContext(fs.readFileSync(path.join(__dirname,
  '../../src/io/kehila_authority/src/listener/edit_recovery.js'), 'utf8'), context);

const owner = {installation_id:'installation', actor_id:'owner'};
const copy = value => JSON.parse(JSON.stringify(value));
function edit(token = '1'.repeat(32)) {
  return {binding:{...owner}, project:'p', item:'item', family:'item_edit', version:'9',
    configuration:'3', lifecycle:{type:'task', workflow:'flow', status:'new', archived:false, phase:'new'},
    estimate:null, wire:JSON.stringify({project_id:'p', item_id:'item', operation_id:token,
      expected_item_version:'9', expected_configuration_revision:'3', estimate:{action:'keep'},
      fields:[{id:'title', action:'set', value:'  exact\r\n\u05e9\u05dc\u05d5\u05dd  '}]})};
}
function fixture(store = memoryStore()) {
  const clock = {now:1000000};
  const journal = context.KehilaEditRecovery({store, now:() => clock.now});
  return {journal, store, clock};
}

test('durable commit is the retention barrier; abort after put preparation retains no edit', async () => {
  const f = fixture(), session = await f.journal.open(owner);
  let release, entered;
  const waiting = new Promise(resolve => { entered = resolve; });
  f.store.hook(() => { entered(); return new Promise(resolve => { release = resolve; }); });
  let complete = false;
  const retaining = session.retain(edit()).then(value => { complete = true; return value; });
  await waiting; assert.equal(complete, false);
  release(); const handle = await retaining;
  assert.equal((await session.admit(handle, owner, 'retry')).wire, edit().wire);
  await session.settle(handle, owner);
  f.store.hook(() => { throw new Error('transaction aborted after request success'); });
  await assert.rejects(session.retain(edit()), /aborted/);
  assert.equal(await session.restore(), null);
});

test('a new page restores the original wire and response-validation facts', async () => {
  const f = fixture(), first = await f.journal.open(owner), original = edit();
  await first.retain(original);
  const reopened = await fixture(f.store).journal.open(owner);
  const restored = await reopened.restore();
  assert.deepEqual(copy(restored.retained), original);
  assert.equal((await reopened.admit(restored, owner, 'check')).wire, original.wire);
});

test('concurrent pages cannot overwrite the one retained edit', async () => {
  const f = fixture(), a = await f.journal.open(owner), b = await f.journal.open(owner);
  const results = await Promise.allSettled([a.retain(edit('a'.repeat(32))), b.retain(edit('b'.repeat(32)))]);
  assert.equal(results.filter(value => value.status === 'fulfilled').length, 1);
  const restored = await (await f.journal.open(owner)).restore();
  assert.ok(['a'.repeat(32), 'b'.repeat(32)].includes(JSON.parse(restored.retained.wire).operation_id));
});

test('logout rotates the empty-slot generation and rejects earlier preparation', async () => {
  const f = fixture(), old = await f.journal.open(owner);
  await f.journal.clear(owner);
  await assert.rejects(old.retain(edit()), /changed|stale/i);
  assert.equal(await (await f.journal.open(owner)).restore(), null);
});

test('a binding reply begun before cross-tab logout cannot open a new session afterward', async () => {
  const f = fixture(); await f.journal.open(owner);
  const beforeBinding = await f.journal.prepare();
  await f.journal.clear(owner);
  await assert.rejects(f.journal.open(owner, beforeBinding), /changed|stale/i);
});

test('cross-tab abandon invalidates old admissions and old success cannot erase a replacement', async () => {
  const f = fixture(), a = await f.journal.open(owner), old = await a.retain(edit());
  const b = await f.journal.open(owner);
  await b.discard();
  const replacement = await b.retain(edit('2'.repeat(32)));
  await assert.rejects(a.admit(old, owner, 'retry'), /changed|stale/i);
  assert.equal(await a.current(old, owner), false);
  await assert.rejects(a.settle(old, owner), /changed|stale/i);
  assert.equal((await b.restore()).retained.wire, replacement.retained.wire);
});

test('seven-day eligibility is not renewed by checks, retry closure or reauthentication', async () => {
  const f = fixture(), session = await f.journal.open(owner), handle = await session.retain(edit());
  f.clock.now += 6 * 86400000;
  await session.admit(handle, owner, 'check');
  await session.closeRetry(handle, owner);
  const restored = await session.restore();
  assert.equal(restored.retryClosed, true);
  await assert.rejects(session.admit(restored, owner, 'retry'), /expired|closed/i);
  assert.equal((await session.admit(restored, owner, 'check')).wire, edit().wire);
  f.clock.now += 86400000;
  const expired = await f.journal.open(owner);
  assert.equal(await expired.restore(), null);
  assert.match(expired.notice, /expired/i);
});

test('a known clock rollback is visible and does not reset the high-water state', async () => {
  const f = fixture(), session = await f.journal.open(owner), handle = await session.retain(edit());
  f.clock.now += 500; await session.admit(handle, owner, 'check');
  f.clock.now -= 100;
  await assert.rejects(f.journal.open(owner), /clock/i);
  f.clock.now += 100;
  assert.equal((await (await f.journal.open(owner)).restore()).retained.wire, edit().wire);
});

test('expiry is checked after an admission digest was delayed, not just before hashing', async () => {
  const f = fixture(); let hashes = 0, holdAt = Infinity, entered, release;
  const held = new Promise(resolve => { entered = resolve; });
  const crypto = {getRandomValues:value => webcrypto.getRandomValues(value), subtle:{digest:async (...args) => {
    if (++hashes === holdAt) { entered(); await new Promise(resolve => { release = resolve; }); }
    return webcrypto.subtle.digest(...args);
  }}};
  const journal = context.KehilaEditRecovery({store:f.store, now:() => f.clock.now, crypto});
  const session = await journal.open(owner), handle = await session.retain(edit());
  holdAt = hashes + 2; // Read checksum first, then encoded admission checksum.
  const attempt = session.admit(handle, owner, 'retry');
  await held; f.clock.now += 7 * 86400000; release();
  await assert.rejects(attempt, /expired/i);
});

test('queued transaction admission rejects expiry and clock rollback without replacing the record', async () => {
  for (const delta of [7 * 86400000, -1]) {
    const f = fixture(), session = await f.journal.open(owner), handle = await session.retain(edit());
    const before = await f.store.read();
    f.store.hook(() => { f.clock.now += delta; });
    await assert.rejects(session.admit(handle, owner, 'retry'), /expired|clock/i);
    assert.equal(await f.store.read(), before);
  }
  const f = fixture(), session = await f.journal.open(owner), handle = await session.retain(edit());
  f.clock.now += 7 * 86400000 - 1;
  assert.equal((await session.admit(handle, owner, 'retry')).wire, edit().wire);
});

test('foreign installation or owner cannot restore a retained request', async () => {
  for (const binding of [{...owner, actor_id:'other'}, {...owner, installation_id:'other'}]) {
    const f = fixture(); await (await f.journal.open(owner)).retain(edit());
    const foreign = await f.journal.open(binding);
    assert.equal(await foreign.restore(), null);
    assert.match(foreign.notice, /owner|installation/i);
  }
});

test('old-owner logout cannot discard the new owner journal', async () => {
  const f = fixture(), other = {...owner, actor_id:'other'};
  await (await f.journal.open(owner)).retain(edit());
  const current = await f.journal.open(other), request = edit(); request.binding = other;
  const handle = await current.retain(request);
  await f.journal.clear(owner);
  assert.equal((await current.restore()).retained.wire, handle.retained.wire);
});

test('corrupt and unsupported journal roots are distinguished from absence and visibly discarded', async () => {
  for (const change of [() => '{broken', raw => raw.replace('exact', 'wrong'),
    raw => { const root = JSON.parse(raw); root.schema = 900; return JSON.stringify(root); }]) {
    const f = fixture(); await (await f.journal.open(owner)).retain(edit());
    await f.store.corrupt(change);
    const restored = await f.journal.open(owner);
    assert.equal(await restored.restore(), null);
    assert.match(restored.notice, /invalid|unsupported|corrupt/i);
  }
});

test('closed durable format rejects secrets, mismatched identity and oversized wire before retention', async () => {
  const changes = [value => { value.csrf = 'secret'; }, value => { value.item = 'other'; },
    value => { value.binding.cookie = 'secret'; }, value => { value.wire += ' '.repeat(1048576); },
    value => { const body = JSON.parse(value.wire); body.password = 'secret'; value.wire = JSON.stringify(body); }];
  for (const change of changes) {
    const f = fixture(), session = await f.journal.open(owner), request = edit(); change(request);
    await assert.rejects(session.retain(request));
    assert.equal(await session.restore(), null);
    assert.doesNotMatch(await f.store.read(), /secret/);
  }
});

test('the exact UTF-8 wire boundary is accepted and a coherent overlong owner ID is rejected', async () => {
  const f = fixture(), session = await f.journal.open(owner), request = edit();
  request.wire += ' '.repeat(1048576 - new TextEncoder().encode(request.wire).length);
  const handle = await session.retain(request);
  assert.equal((await session.admit(handle, owner, 'check')).wire, request.wire);
  const other = fixture();
  await assert.rejects(other.journal.open({...owner, actor_id:'x'.repeat(129)}), /invalid/i);
});

// Deliberately controlled API events, not an implementation of IndexedDB.
function idbEvents({late = false, durability = 'strict'} = {}) {
  let value, request;
  const transactions = [];
  const database = {
    closed:0, objectStoreNames:{length:1, contains:name => name === 'journal'},
    close() { this.closed++; },
    transaction(name, mode, options) {
      let pending = value;
      const get = {};
      const tx = {mode, options, durability,
        objectStore:() => ({get:() => get, put:next => { pending = next; }}),
        read() { get.result = value; get.onsuccess(); },
        complete() { value = pending; tx.oncomplete(); },
        abort() { tx.onabort?.(); }
      };
      transactions.push(tx); return tx;
    }
  };
  const indexedDB = {open() {
    request = {result:database};
    if (!late) setImmediate(() => request.onsuccess());
    return request;
  }};
  return {indexedDB, database, transactions, succeedOpen:() => request.onsuccess(), value:() => value};
}
async function nextTransaction(events, index = 0) {
  for (let i = 0; i < 10 && !events.transactions[index]; i++) await new Promise(setImmediate);
  assert.ok(events.transactions[index]); return events.transactions[index];
}

test('IndexedDB adapter waits for completion and rejects request-success followed by abort', async () => {
  const events = idbEvents(), store = context.KehilaEditRecoveryStore({indexedDB:events.indexedDB});
  let resolved = false;
  const aborted = store.compare(null, 'retained').then(value => { resolved = true; return value; });
  const first = await nextTransaction(events);
  assert.deepEqual(copy(first.options), {durability:'strict'});
  first.read(); await new Promise(setImmediate);
  assert.equal(resolved, false); assert.equal(events.value(), undefined);
  first.abort(); await assert.rejects(aborted, /storage/i);
  const committed = store.compare(null, 'retained');
  const second = await nextTransaction(events, 1); second.read(); second.complete();
  assert.equal(await committed, true); assert.equal(events.value(), 'retained');
});

test('unsupported strict durability fails closed rather than silently using a weaker transaction', async () => {
  const events = idbEvents({durability:'default'});
  const store = context.KehilaEditRecoveryStore({indexedDB:events.indexedDB});
  await assert.rejects(store.compare(null, 'retained'), /storage/i);
  assert.equal(events.value(), undefined);
});

test('IndexedDB admission guard runs after the transaction was queued and before writing', async () => {
  const events = idbEvents(), store = context.KehilaEditRecoveryStore({indexedDB:events.indexedDB});
  let eligible = true;
  const admission = store.compare(null, 'retained', () => { if (!eligible) throw new Error('expired at admission'); });
  const tx = await nextTransaction(events); eligible = false; tx.read();
  await assert.rejects(admission, /expired at admission/);
  assert.equal(events.value(), undefined);
});

test('late IndexedDB open after timeout closes its connection without starting a transaction', async () => {
  const events = idbEvents({late:true});
  const store = context.KehilaEditRecoveryStore({indexedDB:events.indexedDB, timeoutMs:5});
  await assert.rejects(store.read(), /storage/i);
  events.succeedOpen();
  assert.equal(events.database.closed, 1); assert.equal(events.transactions.length, 0);
});
