// Transactional-port policy controls, not actual IndexedDB/browser qualification.
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
  '../../src/io/kehila_authority/src/listener/recovery_journal.js'), 'utf8'), context);
vm.runInContext(fs.readFileSync(path.join(__dirname,
  '../../src/io/kehila_authority/src/listener/selection_recovery.js'), 'utf8'), context);
const owner = {installation_id:'installation', actor_id:'owner'};
const copy = value => JSON.parse(JSON.stringify(value));
function selection(token = '1'.repeat(32), requested = {project_id:'p', item_id:'a'}, version = '7') {
  return {binding:{...owner}, family:'selection', wire:JSON.stringify({
    operation_token:token, expected_version:version, requested})};
}
function fixture(store = memoryStore()) {
  const clock = {now:1000000};
  const journal = context.KehilaSelectionRecovery({store, now:() => clock.now});
  return {journal, store, clock};
}

test('strict transaction completion precedes selection dispatch eligibility', async () => {
  const f = fixture(), session = await f.journal.open(owner);
  let release, entered;
  const waiting = new Promise(resolve => { entered = resolve; });
  f.store.hook(() => { entered(); return new Promise(resolve => { release = resolve; }); });
  let completed = false;
  const retention = session.retain(selection()).then(handle => { completed = true; return handle; });
  await waiting; assert.equal(completed, false);
  release(); const handle = await retention;
  assert.equal((await session.admit(handle, owner, 'retry')).wire, selection().wire);
});

test('retention abort leaves no dispatchable selection', async () => {
  const f = fixture(), session = await f.journal.open(owner);
  f.store.hook(() => { throw new Error('strict commit aborted'); });
  await assert.rejects(session.retain(selection()), /aborted/);
  assert.equal(await session.restore(), null);
});

test('new page restores the exact original identity including clear and maximum version', async () => {
  for (const original of [selection(), selection('2'.repeat(32), null, '18446744073709551615')]) {
    const f = fixture(), first = await f.journal.open(owner);
    await first.retain(original);
    const restoredSession = await fixture(f.store).journal.open(owner);
    const handle = await restoredSession.restore();
    assert.deepEqual(copy(handle.retained), original);
    assert.equal((await restoredSession.admit(handle, owner, 'check')).wire, original.wire);
  }
});

test('another tab cannot overwrite an unresolved selection or settle a replacement', async () => {
  const f = fixture(), a = await f.journal.open(owner), b = await f.journal.open(owner);
  const first = await a.retain(selection());
  await assert.rejects(b.retain(selection('2'.repeat(32))), /unresolved|changed|stale/i);
  const current = await b.restore();
  await b.discard(current, owner);
  const replacement = await b.retain(selection('2'.repeat(32)));
  await assert.rejects(a.settle(first, owner), /changed|stale/i);
  assert.equal((await b.admit(replacement, owner, 'check')).wire, selection('2'.repeat(32)).wire);
});

test('seven-day expiry stops new dispatch without extending recovery on retry', async () => {
  const f = fixture(), session = await f.journal.open(owner);
  const handle = await session.retain(selection());
  f.clock.now += 7 * 86400000 - 1;
  await session.admit(handle, owner, 'retry');
  f.clock.now += 2;
  await assert.rejects(session.admit(handle, owner, 'check'), /expired/i);
  await assert.rejects(session.admit(handle, owner, 'retry'), /expired/i);
  // A previously admitted matching response may settle after local expiry.
  await session.settle(handle, owner);
  assert.equal(await session.restore(), null);
});

test('owner change or signout invalidation never resends or erases a foreign replacement', async () => {
  const f = fixture(), session = await f.journal.open(owner);
  const handle = await session.retain(selection());
  const foreign = {...owner, actor_id:'different-owner'};
  await assert.rejects(session.admit(handle, foreign, 'retry'), /owner|binding|changed/i);
  const ticket = await f.journal.prepare();
  await f.journal.clear(owner);
  await assert.rejects(f.journal.open(owner, ticket), /changed|stale/i);
  const replacementSession = await f.journal.open(foreign);
  const next = selection(); next.binding = {...foreign};
  const replacement = await replacementSession.retain(next);
  await f.journal.clear(owner);
  assert.equal((await replacementSession.admit(replacement, foreign, 'check')).wire, next.wire);
});

test('failed logical deletion is visible and preserves the unresolved record', async () => {
  const f = fixture(), session = await f.journal.open(owner);
  await session.retain(selection()); const original = await f.store.read();
  f.store.hook(() => { throw new Error('delete commit aborted'); });
  await assert.rejects(f.journal.clear(owner), /aborted/i);
  assert.equal(await f.store.read(), original);
});

test('changed or closed server replay identity never becomes a fresh request', async () => {
  const f = fixture(), session = await f.journal.open(owner);
  const handle = await session.retain(selection());
  await session.closeRetry(handle, owner);
  const restored = await session.restore();
  await assert.rejects(session.admit(restored, owner, 'retry'), /closed/i);
  await assert.rejects(session.retain(selection('2'.repeat(32))), /unresolved/i);
});

test('retained wire rejects caller authority malformed identity and oversized bytes', async () => {
  const f = fixture(), session = await f.journal.open(owner);
  for (const command of [
    {operation_token:'1'.repeat(32), expected_version:'7', requested:null, actor_id:'owner'},
    {operation_token:'new-token', expected_version:'7', requested:null},
    {operation_token:'1'.repeat(32), expected_version:'07', requested:null},
    {operation_token:'1'.repeat(32), expected_version:'7', requested:{project_id:'p'}},
    {operation_token:'1'.repeat(32), expected_version:'7', requested:{project_id:'p',item_id:'x'.repeat(129)}},
  ]) {
    const retained = selection(); retained.wire = JSON.stringify(command);
    await assert.rejects(session.retain(retained));
  }
  const duplicate = selection();
  duplicate.wire = '{"operation_token":"'+'1'.repeat(32)+'","expected_version":"7","expected_version":"8","requested":null}';
  await assert.rejects(session.retain(duplicate));
  const oversized = selection(); oversized.wire += ' '.repeat(4096);
  await assert.rejects(session.retain(oversized));
  const wrongFamily = selection(); wrongFamily.family = 'item_edit';
  await assert.rejects(session.retain(wrongFamily));
});

test('local clock rollback prevents retry without silently discarding identity', async () => {
  const f = fixture(), session = await f.journal.open(owner);
  const handle = await session.retain(selection()), raw = await f.store.read();
  f.clock.now -= 1;
  await assert.rejects(session.admit(handle, owner, 'retry'), /clock/i);
  assert.equal(await f.store.read(), raw);
});
