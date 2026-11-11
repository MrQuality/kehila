// Deterministic page-state tests. This is not real-browser/TLS qualification.
const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const {webcrypto} = require('node:crypto');
const memoryStore = require('./journal_store.cjs');
const root = path.resolve(__dirname, '../..');
const page = root + '/src/io/kehila_authority/src/listener/';
const entry = (state = 'prepared') => ({request_id: '9007199254740993', token: 'a'.repeat(32),
  state, form: {name: '  <script>saved</script>  ', prefix: 'SAV', estimate_unit: 'hours'},
  registered_ms: '1000', expires_ms: '2000', project_id: state === 'completed' ? 'native-project' : null});
class Element {
  constructor(tag = 'div') { this.tag = tag; this.hidden = false; this.disabled = false; this.value = ''; this.textContent = ''; this.children = []; this.events = {}; }
  addEventListener(type, fn) { this.events[type] = fn; }
  append(child) { this.children.push(child); }
  replaceChildren(...children) { this.children = children; }
  fire(type = 'click') { if (!this.disabled) this.events[type]?.({preventDefault() {}}); }
}
async function fixture(handler = () => undefined, store = memoryStore()) {
  const elements = new Map(), buttons = [], lifecycle = {}, documentEvents = {};
  const html = fs.readFileSync(page + 'page.html', 'utf8');
  for (const match of html.matchAll(/<(\w+)\b[^>]*\bid="([^"]+)"[^>]*>/g)) {
    const el = new Element(match[1]); elements.set(match[2], el);
    if (match[1] === 'button') buttons.push(el);
  }
  elements.get('project-unit').value = 'hours';
  const calls = [], owner = {installation_id: 'installation', actor_id: 'owner'}, readers = {cancelled: 0, released: 0};
  const fetch = async (url, options) => {
    const body = options.body === undefined ? undefined : JSON.parse(options.body);
    calls.push({url, body, options});
    let reply = await handler(url, body, owner);
    if (!reply) {
      if (url.endsWith('/csrf')) reply = [200, {csrf: 'csrf'}];
      else if (url.endsWith('/state')) reply = [200, {active: true}];
      else if (url.endsWith('/binding')) reply = [200, {...owner}];
      else if (url.endsWith('/list')) reply = [200, {entries: [entry()], before: null}];
      else if (url.endsWith('/open')) reply = [200, {entry: entry()}];
      else throw new Error('Unexpected endpoint: ' + url);
    }
    const bytes = reply[1] instanceof Uint8Array ? reply[1] : new TextEncoder().encode(JSON.stringify(reply[1])); let offset = 0;
    return {status: reply[0], body: {getReader: () => ({
      async read() { if (offset === bytes.length) return {done: true}; const value = bytes.subarray(offset, offset + 4093); offset += value.length; return {done: false, value}; },
      async cancel() { readers.cancelled++; }, releaseLock() { readers.released++; }
    })}};
  };
  const context = {
    document: {visibilityState:'visible', addEventListener:(type,fn) => { documentEvents[type]=fn; },
      getElementById: id => { assert.ok(elements.has(id), id); return elements.get(id); },
      createElement: tag => { const el = new Element(tag); if (tag === 'button') buttons.push(el); return el; },
      querySelectorAll: selector => selector === 'button' ? buttons : []},
    fetch, AbortController, TextDecoder, TextEncoder, Uint8Array, setTimeout, clearTimeout, crypto:webcrypto,
    addEventListener:(type,fn) => { lifecycle[type]=fn; }
  };
  vm.createContext(context);
  vm.runInContext(fs.readFileSync(page + 'edit_recovery.js', 'utf8'), context);
  const recoveryFactory = context.KehilaEditRecovery;
  let journal;
  context.KehilaEditRecovery = options => { journal = recoveryFactory({...options, store}); return journal; };
  vm.runInContext(fs.readFileSync(page + 'work_items.js', 'utf8'), context);
  vm.runInContext(fs.readFileSync(page + 'page.js', 'utf8'), context);
  const idle = async () => {
    for (let i = 0; i < 500; i++) { await new Promise(resolve => setTimeout(resolve,1)); if (!elements.get('logout').disabled) return; }
    throw new Error('Page did not settle');
  };
  await idle();
  const fire = async (id, event = 'click') => { elements.get(id).fire(event); await idle(); };
  const select = async () => { elements.get('requests').children[0].children[0].fire(); await idle(); };
  const visibility = async state => {
    context.document.visibilityState=state;
    assert.equal(typeof documentEvents.visibilitychange,'function');documentEvents.visibilitychange();
    if(state==='visible')await idle();
  };
  return {elements, calls, owner, fire, select, readers, store, journal, visibility};
}

test('failed recovery deletion cannot prevent the server logout attempt', async () => {
  const f = await fixture(url => url === '/auth/browser/logout' ? [200, {}] : undefined);
  f.store.hook(() => { throw new Error('storage abort'); });
  await f.fire('logout');
  assert.equal(f.calls.filter(call => call.url === '/auth/browser/logout').length, 1);
  assert.equal(f.elements.get('login').hidden, false);
  assert.match(f.elements.get('status').textContent, /deletion could not be confirmed/i);
});

test('transient binding failure alone preserves the durable journal', async () => {
  let failed = false;
  const f = await fixture(url => url === '/projects/binding' && failed ? [503, {error:'unavailable'}] : undefined);
  const before = await f.store.read(); failed = true;
  await f.fire('discover');
  assert.equal(await f.store.read(), before);
  assert.equal(f.calls.some(call => call.url === '/auth/browser/logout'), false);
});

test('automatic session refresh after failed deletion and uncertain logout cannot restore abandoned tracking', async () => {
  const f = await fixture(url => url === '/auth/browser/logout' ? [503, {signed_out:false}] : undefined);
  const request = {binding:{...f.owner},project:'p',item:'item',family:'item_edit',version:'1',configuration:'1',
    lifecycle:{type:'task',workflow:'flow',status:'new',archived:false,phase:'new'},estimate:null,
    wire:JSON.stringify({project_id:'p',item_id:'item',operation_id:'1'.repeat(32),expected_item_version:'1',
      expected_configuration_revision:'1',estimate:{action:'keep'},fields:[]})};
  await (await f.journal.open(f.owner)).retain(request);
  f.store.hook(() => { throw new Error('deletion abort'); });
  await f.fire('logout');
  assert.equal(f.elements.get('item-edit-check').disabled,true);
  assert.equal(f.elements.get('item-edit-retry').disabled,true);
  assert.equal(f.calls.some(call => call.url === '/items/edit' || call.url === '/items/reconcile'),false);
});

test('a resumed page hides recovery until a newly observed owner binding is available', async () => {
  let hold=false,release;
  const f=await fixture(url=>{
    if(url==='/projects/binding'&&hold){hold=false;return new Promise(resolve=>{release=resolve;});}
  });
  const request={binding:{...f.owner},project:'p',item:'item',family:'item_edit',version:'1',configuration:'1',
    lifecycle:{type:'task',workflow:'flow',status:'new',archived:false,phase:'new'},estimate:null,
    wire:JSON.stringify({project_id:'p',item_id:'item',operation_id:'1'.repeat(32),expected_item_version:'1',
      expected_configuration_revision:'1',estimate:{action:'keep'},fields:[]})};
  await (await f.journal.open(f.owner)).retain(request);
  await f.visibility('hidden');hold=true;const resuming=f.visibility('visible');
  for(let i=0;i<500&&!release;i++)await new Promise(resolve=>setTimeout(resolve,1));
  assert.equal(typeof release,'function');
  assert.equal(f.elements.get('item-recovery').hidden,true);
  assert.equal(f.elements.get('item-edit-retry').disabled,true);
  release([200,{...f.owner}]);await resuming;
  assert.equal(f.elements.get('item-recovery').hidden,false);
  assert.equal(f.calls.some(call=>call.url==='/items/edit'||call.url==='/items/reconcile'),false);
});

test('unchanged-owner resumption preserves an unsaved project form and selected request', async () => {
  const f=await fixture();await f.select();
  const selected=f.elements.get('request-detail').textContent;
  await f.visibility('hidden');await f.visibility('visible');
  assert.equal(f.elements.get('request-detail').textContent,selected);
  await f.fire('new-request');f.elements.get('project-name').value='  unsaved project  ';
  await f.visibility('hidden');await f.visibility('visible');
  assert.equal(f.elements.get('project-form').hidden,false);
  assert.equal(f.elements.get('project-name').value,'  unsaved project  ');
});
test('startup discovers without executing; execution echoes immutable entry and string ID', async () => {
  const f = await fixture(url => url.endsWith('/execute') ? [201, {outcome: 'created', entry: entry('completed'), result: {version: 1}}] : undefined);
  assert.equal(f.calls.some(c => /register|execute|cancel/.test(c.url)), false);
  await f.select(); f.elements.get('project-name').value = 'edited';
  await f.fire('execute');
  const call = f.calls.find(c => c.url.endsWith('/execute'));
  assert.deepEqual(call.body, {request_id: '9007199254740993', token: 'a'.repeat(32), ...entry().form});
  assert.equal(call.options.credentials, 'same-origin');
  assert.equal(call.options.redirect, 'error');
  assert.equal(f.elements.get('read-project').disabled, false);
});
test('lost registration acknowledgement requires discovery, never automatic repeat or equal-form selection', async () => {
  const f = await fixture(url => { if (url.endsWith('/register')) throw new Error('lost response'); });
  await f.fire('new-request');
  f.elements.get('project-name').value = 'P'; f.elements.get('project-prefix').value = 'PR';
  await f.fire('project-form', 'submit'); await f.fire('discover');
  assert.equal(f.calls.filter(c => c.url.endsWith('/register')).length, 1);
  assert.equal(f.elements.get('selected').hidden, true);
  assert.match(f.elements.get('project-status').textContent, /unconfirmed|identify/i);
});
test('confirming a separate save cannot erase an earlier unknown registration', async () => {
  let saves = 0;
  const f = await fixture(url => {
    if (url.endsWith('/register')) {
      if (++saves === 1) throw new Error('lost first acknowledgement');
      return [201, {outcome: 'registered', entry: {...entry(), request_id: '9007199254740994'}}];
    }
  });
  await f.fire('new-request'); await f.fire('project-form', 'submit');
  await f.fire('new-request'); await f.fire('project-form', 'submit');
  assert.match(f.elements.get('project-status').textContent, /earlier save remains unconfirmed/i);
  assert.equal(f.elements.get('identify-save').hidden, false);
  await f.fire('identify-save');
  assert.equal(f.elements.get('identify-save').hidden, true);
  assert.match(f.elements.get('project-status').textContent, /No request identity was merged or executed/);
  assert.equal(f.calls.filter(c => c.url.endsWith('/register')).length, 2);
});
test('lost execution retains identity, prevents blind retry, and reconciles only that locator', async () => {
  const f = await fixture(url => {
    if (url.endsWith('/execute')) throw new Error('lost response');
    if (url.endsWith('/reconcile')) return [200, {outcome: 'not_completed_at_lookup', entry: entry()}];
  });
  await f.select(); await f.fire('execute');
  assert.equal(f.elements.get('execute').disabled, true);
  await f.fire('execute'); await f.fire('reconcile');
  assert.equal(f.calls.filter(c => c.url.endsWith('/execute')).length, 1);
  assert.deepEqual(f.calls.find(c => c.url.endsWith('/reconcile')).body, {request_id: '9007199254740993'});
  assert.match(f.elements.get('project-status').textContent, /may still complete/i);
});
test('changed binding clears actionable state before sending a mutation', async () => {
  const f = await fixture(); await f.select(); f.owner.actor_id = 'different-owner';
  await f.fire('execute');
  assert.equal(f.calls.some(c => c.url.endsWith('/execute')), false);
  assert.equal(f.elements.get('selected').hidden, true);
  assert.match(f.elements.get('project-status').textContent, /changed/i);
});
test('discovery and reopening cannot erase an unconfirmed execution', async () => {
  const f = await fixture(url => { if (url.endsWith('/execute')) throw new Error('lost response'); });
  await f.select(); await f.fire('execute'); await f.fire('discover'); await f.select();
  assert.equal(f.elements.get('execute').disabled, true);
  assert.match(f.elements.get('project-status').textContent, /may still complete/i);
  await f.fire('new-request');
  assert.equal(f.elements.get('project-form').hidden, true);
  assert.equal(f.calls.filter(c => c.url.endsWith('/execute')).length, 1);
});
test('completed cancellation preserves project identity through failed fresh read', async () => {
  const f = await fixture(url => {
    if (url.endsWith('/cancel')) return [200, {entry: entry('completed')}];
    if (url.endsWith('/read')) return [503, {error: 'unavailable'}];
  });
  await f.select(); await f.fire('cancel'); await f.fire('confirm-cancel');
  assert.match(f.elements.get('project-status').textContent, /confirmed/i);
  await f.fire('read-project');
  assert.deepEqual(f.calls.find(c => c.url.endsWith('/read')).body, {project_id: 'native-project'});
  assert.equal(f.elements.get('read-project').disabled, false);
  assert.match(f.elements.get('request-detail').textContent, /native-project/);
  assert.equal(f.calls.some(c => c.url.endsWith('/register')), false);
});
test('known registration rejection restores exact editable values', async () => {
  for (const status of [400, 429]) {
    const f = await fixture(url => url.endsWith('/register') ? [status, {error: 'rejected'}] : undefined);
    await f.fire('new-request'); f.elements.get('project-name').value = '  exact  ';
    await f.fire('project-form', 'submit');
    assert.equal(f.elements.get('project-form').hidden, false);
    assert.equal(f.elements.get('project-name').value, '  exact  ');
    assert.match(f.elements.get('project-status').textContent, /reject/i);
  }
});
test('lost cancellation acknowledgement remains unresolved after reopening', async () => {
  const f = await fixture(url => { if (url.endsWith('/cancel')) throw new Error('lost response'); });
  await f.select(); await f.fire('cancel'); await f.fire('confirm-cancel');
  await f.fire('discover'); await f.select();
  assert.equal(f.elements.get('execute').disabled, true);
  assert.match(f.elements.get('project-status').textContent, /unconfirmed|may still complete/i);
});
test('replay expiry retains completed project identity and enables fresh read', async () => {
  const f = await fixture(url => url.endsWith('/reconcile') ? [410, {error: 'replay_expired', entry: entry('completed')}] : undefined);
  await f.select(); await f.fire('reconcile');
  assert.equal(f.elements.get('execute').disabled, true);
  assert.equal(f.elements.get('read-project').disabled, false);
  assert.match(f.elements.get('project-status').textContent, /expired/i);
});
test('binding unavailability disables all project actions without discarding uncertainty', async () => {
  let unavailable = false;
  const f = await fixture(url => {
    if (url.endsWith('/execute')) { unavailable = true; throw new Error('lost response'); }
    if (url.endsWith('/binding') && unavailable) return [503, {error: 'unavailable'}];
  });
  await f.select(); await f.fire('execute'); await f.fire('reconcile');
  for (const id of ['execute', 'cancel', 'discover', 'new-request', 'save-request']) assert.equal(f.elements.get(id).disabled, true, id);
  assert.match(f.elements.get('project-status').textContent, /9007199254740993/);
});
test('response byte reader accepts exact limit across chunks and releases the reader', async () => {
  const bytes = new Uint8Array(1048576 + 8192).fill(32);
  bytes.set(new TextEncoder().encode(JSON.stringify({entry: entry(), outcome: 'registered'})));
  const f = await fixture(url => url.endsWith('/register') ? [201, bytes] : undefined);
  await f.fire('new-request'); await f.fire('project-form', 'submit');
  assert.equal(f.elements.get('selected').hidden, false);
  assert.equal(f.readers.cancelled, 0);
  assert.ok(f.readers.released >= 1);
});
test('binding transport failure disables project controls while preserving known locator', async () => {
  let unavailable = false;
  const f = await fixture(url => {
    if (url.endsWith('/execute')) { unavailable = true; throw new Error('lost response'); }
    if (url.endsWith('/binding') && unavailable) throw new Error('network unavailable');
  });
  await f.select(); await f.fire('execute'); await f.fire('reconcile');
  assert.equal(f.elements.get('execute').disabled, true);
  assert.equal(f.elements.get('discover').disabled, true);
  assert.match(f.elements.get('project-status').textContent, /9007199254740993/);
});
test('over-limit, invalid UTF-8 and malformed acknowledgement preserve save recovery', async () => {
  for (const bytes of [new Uint8Array(1048576 + 8193).fill(32), Uint8Array.of(255), new TextEncoder().encode('{')]) {
    const f = await fixture(url => url.endsWith('/register') ? [201, bytes] : undefined);
    await f.fire('new-request'); await f.fire('project-form', 'submit');
    assert.match(f.elements.get('project-status').textContent, /Save remains unconfirmed/);
    assert.equal(f.elements.get('project-form').hidden, true);
    assert.equal(f.readers.cancelled, 1);
  }
});

test('fresh current project read enables scoped item browsing and a separate project request clears it', async () => {
  const f = await fixture((url, body) => {
    if (url === '/projects/list') return [200, {entries:[entry('completed')], before:null}];
    if (url === '/projects/open') return [200, {entry:entry('completed')}];
    if (url === '/projects/read') return [200, {outcome:'read',
      project:{codec_version:1,kind:'project_snapshot',value:{id:'native-project',name:'Fresh project',estimate_unit:'points',configuration_revision:'1'}},
      configuration:{codec_version:1,kind:'configuration_snapshot',value:{project_id:'native-project',revision:'1',fields:[]}}}];
    if (url === '/items/list') { assert.deepEqual(body,{project_id:'native-project',before:null});return [200,{outcome:'listed',entries:[],before:null}]; }
  });
  assert.equal(f.elements.get('work-items').hidden,true);await f.select();await f.fire('read-project');assert.equal(f.elements.get('work-items').hidden,false);
  await f.fire('items-refresh');assert.equal(f.calls.filter(call=>call.url==='/items/list').length,1);await f.fire('new-request');assert.equal(f.elements.get('work-items').hidden,true);
});
