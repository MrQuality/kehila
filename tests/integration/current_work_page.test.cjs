// Prepared controller specifications. No actual browser/IndexedDB qualification.
const {test:nodeTest} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const {webcrypto} = require('node:crypto');
const memoryStore = require('./journal_store.cjs');
const test = (name, work) => nodeTest(name, {timeout:5000}, work);
const page = path.resolve(__dirname, '../../src/io/kehila_authority/src/listener');
const clone = value => JSON.parse(JSON.stringify(value));
const owner = {installation_id:'installation', actor_id:'owner'};
const item = (name = 'a', project = 'p') => ({project_id:project, item_id:name});
const read = (version = '0', target = null) => ({codec_version:1, outcome:'read', version,
  target:target === null ? {state:'none'} : {state:'available', ...target}});
const receipt = (outcome, version, target) => ({codec_version:1, outcome,
  selection:{user_id:'owner', version, requested:target}});
const mutations = calls => calls.filter(call => ['/selection/set','/selection/reconcile'].includes(call.url));

async function bounded(work, label) {
  let timer;
  try {
    return await Promise.race([work, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(label)), 2000);
    })]);
  } finally { clearTimeout(timer); }
}

class Element {
  constructor() { this.hidden = false; this.disabled = false; this.textContent = ''; this.children = []; this.events = {}; }
  set innerHTML(_) { throw new Error('Current-work identifiers must be rendered as text'); }
  addEventListener(type, work) { this.events[type] = work; }
  append(...children) { this.children.push(...children); }
  replaceChildren(...children) { this.children = children; }
}

function fixture(handler = () => undefined, store = memoryStore()) {
  const elements = new Map(), calls = [], jobs = [];
  const binding = {value:{...owner}, failed:false, checks:0, seed:1};
  const server = {version:'0', target:null};
  const clock = {now:1000000};
  const document = {
    getElementById(id) { if (!elements.has(id)) elements.set(id, new Element()); return elements.get(id); },
    createElement() { return new Element(); },
  };
  async function exchange(url, body, wire) {
    const call = {url, body:clone(body)};
    if (wire !== undefined) call.wire = wire;
    calls.push(call);
    const custom = await handler(url, body, wire);
    if (custom !== undefined) return custom;
    if (url === '/selection/read') return {status:200, data:read(server.version, server.target)};
    if (url === '/selection/reconcile') return {status:200, data:{codec_version:1, outcome:'absent'}};
    assert.equal(url, '/selection/set', 'selection must not mutate another command family');
    if (body.expected_version !== server.version) return {status:409, data:{codec_version:1,
      outcome:'rejected', code:'selection_version_conflict', current_version:server.version}};
    if (JSON.stringify(server.target) !== JSON.stringify(body.requested)) {
      server.version = (BigInt(server.version) + 1n).toString();
    }
    server.target = clone(body.requested);
    return {status:200, data:receipt('accepted', server.version, server.target)};
  }
  const context = {TextEncoder, TextDecoder, crypto:webcrypto, setTimeout, clearTimeout};
  vm.createContext(context);
  for (const name of ['recovery_journal.js','selection_recovery.js']) {
    vm.runInContext(fs.readFileSync(path.join(page, name), 'utf8'), context);
  }
  const source = path.join(page, 'current_work.js');
  assert.ok(fs.existsSync(source), 'current-work controller implementation is required');
  vm.runInContext(fs.readFileSync(source, 'utf8'), context);
  assert.equal(typeof context.KehilaCurrentWork, 'function');
  const recovery = context.KehilaSelectionRecovery({store, now:() => clock.now});
  const view = context.KehilaCurrentWork({
    document, recovery,
    async checkBinding() {
      binding.checks++;
      if (binding.failed) throw new Error('Owner binding unavailable');
      return {...binding.value};
    },
    randomBytes() { return new Uint8Array(16).fill(binding.seed++); },
    run(work) { const job = work(); jobs.push(job); return job; },
    request:(url, body) => exchange(url, body),
    requestWire:(url, wire) => exchange(url, JSON.parse(wire), wire),
  });
  const flush = async () => {
    while (jobs.length) await bounded(Promise.all(jobs.splice(0)), 'controller jobs did not settle');
  };
  const fire = async id => {
    const element = document.getElementById(id);
    if (!element.disabled) {
      assert.equal(typeof element.events.click, 'function', 'enabled handler ' + id);
      element.events.click({preventDefault() {}});
    }
    await flush();
  };
  const text = id => {
    const collect = element => [element.textContent, ...element.children.map(collect)].join('\n');
    return collect(document.getElementById(id));
  };
  return {view, recovery, binding, calls, elements, store, server, clock, fire, flush, text};
}

async function ready(f) {
  f.view.controls(false, true);
  await f.view.restoreRecovery();
  await f.fire('current-work-refresh');
}

function offer(f, target = item()) { f.view.offerItem({...target, archived:false}); }

async function observed(predicate, label) {
  const end = Date.now() + 2000;
  while (!predicate() && Date.now() < end) await new Promise(resolve => setTimeout(resolve, 1));
  assert.ok(predicate(), label);
}

test('current-work controls are explicit and separate from selected creation requests', () => {
  const html = fs.readFileSync(path.join(page, 'page.html'), 'utf8');
  for (const id of ['current-work','current-work-status','current-work-detail','current-work-refresh',
    'current-work-clear','item-select-current','selection-recovery','selection-recovery-status',
    'selection-check','selection-retry','selection-forget','selection-receipt']) {
    assert.match(html, new RegExp('\\bid="' + id + '"'), 'visible control ' + id);
  }
  const sections = [];
  let currentParents = null;
  for (const match of html.matchAll(/<\/section\s*>|<section\b[^>]*>/g)) {
    if (match[0].startsWith('</')) sections.pop();
    else {
      const id = /\bid="([^"]+)"/.exec(match[0])?.[1] ?? null;
      if (id === 'current-work') currentParents = [...sections];
      sections.push(id);
    }
  }
  assert.ok(currentParents?.includes('session'), 'current work belongs to the authenticated owner view');
  assert.ok(!currentParents.includes('work-items') && !currentParents.includes('selected'),
    'current work must survive hiding a project or a selected creation request');
  assert.match(html, /work item/i);
});

test('a current-work read does not select or clear an item', async () => {
  const f = fixture(); await ready(f);
  assert.equal(mutations(f.calls).length, 0);
  assert.equal(f.elements.get('item-select-current').disabled, true);
  assert.equal(f.elements.get('current-work-clear').disabled, true);
  assert.match(f.text('current-work-detail'), /no current|none|not chosen/i);
});

test('an explicit existing-item choice sends only user-context selection fields', async () => {
  const f = fixture(); await ready(f); offer(f); await f.fire('item-select-current');
  const [sent] = mutations(f.calls);
  assert.equal(sent.url, '/selection/set');
  assert.deepEqual(sent.body, {operation_token:'01'.repeat(16), expected_version:'0', requested:item()});
  assert.deepEqual(f.server.target, item());
  assert.equal(f.server.version, '1');
  assert.equal(f.calls.some(call => /^\/(items\/(edit|pending\/(register|execute|cancel))|projects\/(register|execute|cancel))$/.test(call.url)), false);
});

test('current-work identifiers are rendered as text rather than executable markup', async () => {
  const target = item('<script>private-item</script>', '<img onerror=private-project>');
  const f = fixture(); f.server.version = '4'; f.server.target = target;
  await ready(f);
  assert.ok(f.text('current-work-detail').includes(target.project_id));
  assert.ok(f.text('current-work-detail').includes(target.item_id));
});

test('malformed current responses cannot supply a version for a mutation', async () => {
  for (const data of [
    {...read(), codec_version:2},
    {...read(), version:'01'},
    {...read(), version:'18446744073709551616'},
    {...read(), unexpected:true},
    {codec_version:1, outcome:'read', version:'2', target:{state:'unavailable', item_id:'hidden-item'}},
  ]) {
    const f = fixture(url => url === '/selection/read' ? {status:200, data} : undefined);
    await ready(f); offer(f); await f.fire('item-select-current'); await f.fire('current-work-clear');
    assert.equal(mutations(f.calls).length, 0);
    assert.doesNotMatch(f.text('current-work-detail'), /hidden-item/);
  }
});

test('strict retention completion precedes the first selection transmission', async () => {
  const f = fixture(); await ready(f); offer(f);
  let release;
  f.store.hook(() => new Promise(done => { release = done; }));
  const choosing = f.fire('item-select-current');
  choosing.catch(() => {});
  const failures = [];
  try {
    await observed(() => typeof release === 'function', 'strict retention boundary reached');
    assert.equal(mutations(f.calls).length, 0);
  } catch (error) { failures.push(error); }
  finally {
    release?.();
    try { await bounded(choosing, 'retention control did not finish'); }
    catch (error) { failures.push(error); }
  }
  if (failures.length) throw new AggregateError(failures, 'retention assertion/cleanup failed');
  assert.equal(mutations(f.calls).length, 1);
});

test('a failed retention transaction prevents transmission and reports storage failure', async () => {
  const f = fixture(); await ready(f); offer(f);
  f.store.hook(() => { throw new Error('strict storage commit failed'); });
  await f.fire('item-select-current');
  assert.equal(mutations(f.calls).length, 0);
  assert.match(f.text('selection-recovery-status') + f.text('current-work-status'), /storage|commit|retain/i);
});

test('project navigation clears the offered item without clearing current work or the journal', async () => {
  const f = fixture(); f.server.version = '7'; f.server.target = item(); await ready(f);
  offer(f, item('b', 'q')); f.view.offerItem(null);
  await f.fire('item-select-current');
  assert.equal(mutations(f.calls).length, 0);
  assert.deepEqual(f.server.target, item());
  assert.equal(f.elements.get('current-work-clear').disabled, false);
});

test('a redacted current choice retains its exact version and can be explicitly cleared', async () => {
  const f = fixture(url => url === '/selection/read' && f.server.target !== null ? {status:200,
    data:{codec_version:1, outcome:'read', version:'9007199254740993', target:{state:'unavailable'}}} : undefined);
  f.server.version = '9007199254740993'; f.server.target = item('private-target');
  await ready(f);
  assert.match(f.text('current-work-detail'), /unavailable|cannot.*(access|display|read)/i);
  assert.doesNotMatch(f.text('current-work-detail'), /private-target/);
  await f.fire('current-work-clear');
  assert.deepEqual(mutations(f.calls)[0].body.requested, null);
  assert.equal(mutations(f.calls)[0].body.expected_version, '9007199254740993');
  assert.equal(f.server.target, null);
  assert.equal(f.server.version, '9007199254740994');
});

test('reload restores exact uncertain identity without resending it', async () => {
  const store = memoryStore();
  const first = fixture(url => { if (url === '/selection/set') throw new Error('reply lost'); }, store);
  await ready(first); offer(first); await first.fire('item-select-current');
  const original = mutations(first.calls)[0];
  const next = fixture((url, body) => url === '/selection/reconcile' ? {status:200,
    data:receipt('replayed','1',body.requested)} : undefined, store);
  await ready(next);
  assert.equal(mutations(next.calls).length, 0);
  await next.fire('selection-check');
  assert.equal(mutations(next.calls)[0].url, '/selection/reconcile');
  assert.equal(mutations(next.calls)[0].wire, original.wire);
});

test('lookup absence never executes automatically and explicit retry preserves original wire', async () => {
  const f = fixture(url => { if (url === '/selection/set') throw new Error('reply unknown'); });
  await ready(f); offer(f); await f.fire('item-select-current');
  const original = mutations(f.calls)[0];
  await f.fire('selection-check');
  assert.deepEqual(mutations(f.calls).map(call => call.url), ['/selection/set','/selection/reconcile']);
  assert.match(f.text('selection-recovery-status'), /absent|not.*recorded|not.*found/i);
  await f.fire('selection-retry');
  assert.equal(mutations(f.calls).at(-1).wire, original.wire);
  assert.equal(mutations(f.calls).at(-1).url, '/selection/set');
});

test('a replay receipt remains original while a separate current read shows a later item', async () => {
  const store = memoryStore();
  const original = item('original-item-31415','original-project');
  const later = item('later-item-92653','later-project');
  const first = fixture(url => { if (url === '/selection/set') throw new Error('lost'); }, store);
  await ready(first); offer(first, original); await first.fire('item-select-current');
  const next = fixture(url => url === '/selection/read' ? {status:200, data:read('2',later)}
    : url === '/selection/reconcile' ? {status:200, data:receipt('replayed','1',original)} : undefined, store);
  await ready(next); await next.fire('selection-check'); await next.fire('current-work-refresh');
  assert.ok(next.text('selection-receipt').includes(original.item_id));
  assert.ok(next.text('selection-receipt').includes(original.project_id));
  assert.ok(next.text('current-work-detail').includes(later.item_id));
  assert.ok(next.text('current-work-detail').includes(later.project_id));
  assert.ok(!next.text('current-work-detail').includes(original.item_id));
  assert.equal(mutations(next.calls).filter(call => call.url === '/selection/set').length, 0);
});

test('a changed selection version does not silently rebase or generate another token', async () => {
  const f = fixture(url => url === '/selection/set' ? {status:409,
    data:{codec_version:1, outcome:'rejected', code:'selection_version_conflict', current_version:'2'}} : undefined);
  await ready(f); offer(f); await f.fire('item-select-current');
  assert.equal(mutations(f.calls).length, 1);
  assert.equal(f.binding.seed, 2);
  assert.match(f.text('current-work-status') + f.text('selection-recovery-status'), /conflict|changed|refresh/i);
  await f.fire('selection-retry');
  assert.equal(mutations(f.calls).length, 1);
});

test('an unresolved choice blocks another choice until explicit abandonment', async () => {
  const f = fixture(url => { if (url === '/selection/set') throw new Error('unknown'); });
  await ready(f); offer(f); await f.fire('item-select-current');
  offer(f, item('b')); await f.fire('item-select-current');
  assert.equal(mutations(f.calls).length, 1);
  const calls = f.calls.length;
  await f.fire('selection-forget');
  assert.equal(f.calls.length, calls);
  assert.match(f.text('selection-recovery-status'), /not.*cancel|not.*rollback|may.*commit/i);
  await f.fire('current-work-refresh'); offer(f, item('b')); await f.fire('item-select-current');
  assert.equal(mutations(f.calls).length, 2);
  assert.deepEqual(mutations(f.calls).at(-1).body.requested, item('b'));
  assert.notEqual(mutations(f.calls)[0].body.operation_token, mutations(f.calls).at(-1).body.operation_token);
});

test('owner replacement invalidates a late current read without showing its identifiers', async () => {
  let release;
  const f = fixture(url => url === '/selection/read'
    ? new Promise(resolve => { release = resolve; }) : undefined);
  f.view.controls(false, true); await f.view.restoreRecovery();
  const reading = f.fire('current-work-refresh');
  reading.catch(() => {});
  const failures = [];
  try {
    await observed(() => typeof release === 'function', 'current read entered transport');
    f.binding.value = {installation_id:'installation', actor_id:'another-owner'};
    f.view.startSession();
  } catch (error) { failures.push(error); }
  finally {
    release?.({status:200, data:read('9',item('old-private-item','old-private-project'))});
    try { await bounded(reading, 'late current read did not finish'); }
    catch (error) { failures.push(error); }
  }
  if (failures.length) throw new AggregateError(failures, 'late-read assertion/cleanup failed');
  assert.doesNotMatch(f.text('current-work-detail'), /old-private/);
});

test('suspension prevents an explicit retry until fresh binding and journal validation', async () => {
  const f = fixture(url => { if (url === '/selection/set') throw new Error('unknown'); });
  await ready(f); offer(f); await f.fire('item-select-current');
  f.view.suspendRecovery(); const before = mutations(f.calls).length;
  await f.fire('selection-retry');
  assert.equal(mutations(f.calls).length, before);
  f.binding.failed = true;
  await f.view.restoreRecovery().catch(() => false);
  await f.fire('selection-retry');
  assert.equal(mutations(f.calls).length, before);
  f.binding.failed = false;
  await f.view.restoreRecovery(); f.view.controls(false, true);
  await f.fire('selection-retry');
  assert.equal(mutations(f.calls).length, before + 1);
  assert.equal(mutations(f.calls).at(-1).wire, mutations(f.calls)[0].wire);
});

test('a mismatched successful receipt cannot settle the retained identity', async () => {
  for (const corrupt of [
    data => { data.selection.user_id = 'different-owner'; },
    data => { data.selection.requested = item('another-item'); },
    data => { data.selection.version = '99'; },
    data => { data.codec_version = 2; },
    data => { data.unexpected = true; },
  ]) {
    const data = receipt('accepted','1',item()); corrupt(data);
    const f = fixture(url => url === '/selection/set' ? {status:200, data} : undefined);
    await ready(f); offer(f); await f.fire('item-select-current');
    const sent = mutations(f.calls)[0];
    const inspected = await f.recovery.open(owner);
    const handle = await inspected.restore();
    assert.ok(handle, 'untrusted response cannot prove the retained operation settled');
    assert.equal(handle.retained.wire, sent.wire);
    assert.match(f.text('selection-recovery-status') + f.text('current-work-status'), /unknown|unconfirmed|invalid|mismatch/i);
  }
});

test('an archived target rejection closes retry without claiming an unknown commit', async () => {
  const f = fixture(url => url === '/selection/set' ? {status:409,
    data:{codec_version:1,outcome:'rejected',code:'archived_target'}} : undefined);
  await ready(f); offer(f); await f.fire('item-select-current');
  assert.equal(mutations(f.calls).length,1,'the live candidate reached the server before the archive race');
  assert.equal(f.elements.get('selection-retry').disabled,true,'an authoritative archive rejection closes this retry');
  assert.match(f.text('current-work-status') + f.text('selection-recovery-status'),/archived/i);
  await f.fire('selection-retry');
  assert.equal(mutations(f.calls).length,1);
});

test('an absent item rejection closes the exact retry and explains the missing work item', async () => {
  const f = fixture(url => url === '/selection/set' ? {status:404,
    data:{codec_version:1,outcome:'rejected',code:'invalid_reference'}} : undefined);
  await ready(f); offer(f); await f.fire('item-select-current');
  assert.equal(mutations(f.calls).length,1);
  const inspected = await f.recovery.open(owner);
  const saved = await inspected.restore();
  assert.ok(saved,'the rejected identity remains available for explicit outcome checking');
  assert.equal(saved.retained.wire,mutations(f.calls)[0].wire);
  assert.equal(f.elements.get('selection-retry').disabled,true,'a defined absent-item rejection closes retry');
  assert.equal(saved.retryClosed,true,'terminal closure persists in the real journal');
  await assert.rejects(inspected.admit(saved,owner,'retry'),/closed/i);
  assert.match(f.text('current-work-status') + f.text('selection-recovery-status'),/not found|could not be found|missing work item/i);
  await f.fire('selection-retry');
  assert.equal(mutations(f.calls).length,1,'terminal rejection never invents another request');
});

test('untrusted missing-item envelopes retain an unconfirmed retry', async () => {
  for (const response of [
    {status:404,data:{error:'not found'}},
    {status:404,data:{codec_version:2,outcome:'rejected',code:'invalid_reference'}},
    {status:404,data:{codec_version:1,outcome:'rejected',code:'archived_target'}},
    {status:409,data:{codec_version:1,outcome:'rejected',code:'invalid_reference'}},
    {status:404,data:{codec_version:1,outcome:'rejected',code:'invalid_reference',extra:true}},
  ]) {
    const f=fixture(url=>url==='/selection/set' ? response : undefined);
    await ready(f);offer(f);await f.fire('item-select-current');
    const saved=await (await f.recovery.open(owner)).restore();
    assert.ok(saved);
    assert.equal(saved.retained.wire,mutations(f.calls)[0].wire);
    assert.equal(saved.retryClosed,false,'an untrusted envelope cannot permanently close this identity');
    assert.equal(mutations(f.calls).length,1);
  }
});

test('contracted expiry replies durably close the exact retained retry', async () => {
  for (const route of ['/selection/set','/selection/reconcile']) {
    const store = memoryStore();
    const f = fixture(url => {
      if (url === route) return {status:410,data:{codec_version:1,outcome:'replay_expired'}};
      if (url === '/selection/set') throw new Error('original reply lost');
    },store);
    await ready(f); offer(f); await f.fire('item-select-current');
    const original = mutations(f.calls)[0];
    if (route === '/selection/reconcile') await f.fire('selection-check');
    assert.equal(mutations(f.calls).at(-1).url,route,'the contracted expiry must reach the controller');
    assert.equal(mutations(f.calls).at(-1).wire,original.wire);
    const inspected = await f.recovery.open(owner);
    const saved = await inspected.restore();
    assert.ok(saved,'expiry retains the original identity for explicit checks');
    assert.equal(saved.retained.wire,original.wire);
    assert.equal(saved.retryClosed,true,'contracted expiry durably closes retry');
    await assert.rejects(inspected.admit(saved,owner,'retry'),/closed/i);
    assert.match(f.text('selection-recovery-status'),/expired/i);
    const reloaded = fixture(undefined,store); await ready(reloaded);
    assert.equal(mutations(reloaded.calls).length,0,'restoration never resends');
    assert.equal(reloaded.elements.get('selection-retry').disabled,true);
    await reloaded.fire('selection-retry');
    assert.equal(mutations(reloaded.calls).length,0,'durable expiry remains closed after reload');
  }
});

test('malformed expiry replies keep exact recovery identity and retry open', async () => {
  for (const route of ['/selection/set','/selection/reconcile']) {
    const store = memoryStore();
    const f = fixture(url => {
      if (url === route) return {status:200,data:{codec_version:1,outcome:'replay_expired'}};
      if (url === '/selection/set') throw new Error('original reply lost');
    },store);
    await ready(f); offer(f); await f.fire('item-select-current');
    const original = mutations(f.calls)[0];
    if (route === '/selection/reconcile') await f.fire('selection-check');
    assert.equal(mutations(f.calls).at(-1).url,route,'the malformed expiry must reach the controller');
    assert.equal(mutations(f.calls).at(-1).wire,original.wire);
    const saved = await (await f.recovery.open(owner)).restore();
    assert.ok(saved,'an untrusted status cannot settle the original identity');
    assert.equal(saved.retained.wire,original.wire);
    assert.equal(saved.retryClosed,false,'an unsupported expiry status cannot close retry');
    assert.equal(f.elements.get('selection-retry').disabled,false);
    assert.match(f.text('selection-recovery-status') + f.text('current-work-status'),/unconfirmed|invalid|mismatch/i);
    const reloaded = fixture(url => {
      if (url === '/selection/set') throw new Error('reply remains unknown');
    },store);
    await ready(reloaded);
    assert.equal(mutations(reloaded.calls).length,0,'restoration never resends');
    await reloaded.fire('selection-retry');
    assert.equal(mutations(reloaded.calls).length,1,'explicit retry remains available after reload');
    assert.equal(mutations(reloaded.calls)[0].wire,original.wire);
  }
});

function uncertainCommitStore(kind) {
  const backing = memoryStore();
  let armed = false, fired = false;
  return {
    read:() => backing.read(),
    async compare(expected,replacement,admit) {
      const committed = await backing.compare(expected,replacement,admit);
      const root = JSON.parse(replacement);
      if (committed && armed && (kind === 'close' ? root.entry?.retry_closed === true : root.entry === null)) {
        armed = false; fired = true;
        throw new Error('strict commit completed but its confirmation was lost');
      }
      return committed;
    },
    arm() { armed = true; },
    get fired() { return fired; },
  };
}

for (const kind of ['settle','discard','close']) {
  test('an unconfirmed ' + kind + ' cleanup stays blocked until a fresh page', async () => {
    const store = uncertainCommitStore(kind);
    const f = fixture(url => {
      if (url !== '/selection/set') return undefined;
      if (kind === 'discard') throw new Error('unknown server result');
      store.arm();
      if (kind === 'close') return {status:409,data:{codec_version:1,outcome:'rejected',
        code:'selection_version_conflict',current_version:'2'}};
    },store);
    await ready(f); offer(f); await f.fire('item-select-current');
    if (kind === 'discard') { store.arm(); await f.fire('selection-forget'); }
    assert.equal(store.fired,true,'the underlying cleanup must actually commit before losing confirmation');
    const durable = JSON.parse(await store.read());
    if (kind === 'close') assert.equal(durable.entry.retry_closed,true);
    else assert.equal(durable.entry,null);
    assert.equal(await f.view.restoreRecovery(),false,'failed cleanup must stay latched across ordinary restoration');
    await f.fire('selection-retry');
    assert.equal(mutations(f.calls).length,1,'a failed cleanup cannot enable transmission');
    assert.match(f.text('selection-recovery-status'),/cleanup|deletion|reload/i);
    const reloaded = fixture(undefined,store);
    reloaded.view.controls(false,true);
    assert.equal(await reloaded.view.restoreRecovery(),true,'a fresh page can inspect the actual durable journal');
  });
}
