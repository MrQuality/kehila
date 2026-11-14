// Deterministic controller checks; not browser or native qualification.
const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const {webcrypto} = require('node:crypto');
const memoryStore = require('./journal_store.cjs');
const source = path.resolve(__dirname, '../../src/io/kehila_authority/src/listener/work_items.js');
const wrap = (kind,value) => ({codec_version:1,kind,value});
const current = (id='p') => ({outcome:'read',project:wrap('project_snapshot',{id,name:'Project '+id,estimate_unit:'hours',configuration_revision:'1'}),configuration:wrap('configuration_snapshot',{project_id:id,revision:'1',fields:[],types:[],statuses:[]})});
const summary = (sequence='9007199254740993', project='p') => ({item_id:'item-'+sequence,version:'9007199254740995',type_id:'task',workflow_id:'flow',status_id:'new',archived:false,estimate:'1',name:'  <script>exact</script>  ',readable_id:{project_id:project,item_id:'item-'+sequence,prefix:'PR',sequence}});
const pending = (state='prepared',project='p') => ({project_id:project,request_id:'9007199254740997',intent_handle:'a'.repeat(32),token:'b'.repeat(32),target_item_id:'target',registered_ms:'0',expires_ms:'86400000',state});
const opened = (entry=pending()) => ({entry,request:entry.state==='completed'?null:wrap('item_create_request',{project_id:entry.project_id,operation_id:entry.token,item_id:entry.target_item_id,type_id:'task',workflow_id:null,expected_configuration_revision:'1',estimate:'0',values:[{id:'title',value:{kind:'text',value:'  frozen title  '}}]})});
const itemRead = (row=summary()) => ({outcome:'read',readable_id:row.readable_id,result:wrap('item_edit_result',{project:{id:row.readable_id.project_id,estimate_unit:'hours'},item:{id:row.item_id,project_id:row.readable_id.project_id,type_id:'task',workflow_id:'flow',status_id:'new',version:row.version,archived:false},data:{estimate:row.estimate,values:[{id:'title',value:{kind:'text',value:'  exact text  '}}]},phase:'new'})});
class Element {
 constructor(tag='div'){this.tag=tag;this.hidden=false;this.disabled=false;this.textContent='';this.children=[];this.events={};}
 get value(){return this._value;}
 set value(value){this._value=this.tag==='textarea'?String(value).replace(/\r\n?/g,'\n'):value;}
 addEventListener(type,fn){this.events[type]=fn;}
 append(...children){this.children.push(...children);}
 replaceChildren(...children){this.children=children;}
}
function fixture(handler=()=>undefined, store=memoryStore()){
 const clock={now:1000000};
 const elements=new Map(); const calls=[]; const jobs=[]; const binding={failure:false,checks:0,value:{installation_id:'installation',actor_id:'owner'},randomFailure:false,encodeFailure:false,randomSeed:17};
 const document={getElementById(id){if(!elements.has(id))elements.set(id,new Element());return elements.get(id);},createElement(tag){return new Element(tag);}};
 const ports={document,async checkBinding(){binding.checks++;if(binding.wait)await binding.wait;if(binding.failure)throw new Error('Owner binding unavailable');return {...binding.value};},randomBytes(){if(binding.randomFailure)throw new Error('Randomness unavailable');return new Uint8Array(16).fill(binding.randomSeed);},encode(value){if(binding.encodeFailure)throw new Error('Encoding unavailable');return JSON.stringify(value);},async requestWire(url,wire){const body=JSON.parse(wire);calls.push({url,body,wire});return await handler(url,body)||{status:503,data:{error:'unavailable'}};},run(work){const job=work();jobs.push(job);return job;},async request(url,body){calls.push({url,body});return await handler(url,body) || {status:200,data:url==='/items/list'?{outcome:'listed',entries:[summary()],before:null}:url==='/items/read'?itemRead():url==='/items/pending/list'?{entries:[pending()],before:null}:opened()};}};
 const context={TextEncoder,crypto:webcrypto,setTimeout,clearTimeout};vm.createContext(context);
 vm.runInContext(fs.readFileSync(path.join(path.dirname(source),'edit_recovery.js'),'utf8'),context);
 ports.recovery=context.KehilaEditRecovery({store,now:()=>clock.now});
 vm.runInContext(fs.readFileSync(source,'utf8'),context);
 const view=context.KehilaWorkItems(ports);
 const fire=async(id,type='click')=>{const el=document.getElementById(id);if(!el.disabled){assert.equal(typeof el.events[type],'function','enabled handler '+id);el.events[type]({preventDefault(){}});}await Promise.all(jobs.splice(0));};
 const row=async(id,index=0)=>{document.getElementById(id).children[index].children[0].events.click();await Promise.all(jobs.splice(0));};
 const text=id=>{const collect=el=>[el.textContent,...el.children.map(collect)].join('\n');return collect(document.getElementById(id));};
 return {view,ports,calls,elements,fire,row,text,binding,store,clock};
}
test('current authorized project context enables bounded browsing without a mutation',async()=>{
 const f=fixture();assert.equal(f.calls.length,0);f.view.useProject(current(),'p');await f.fire('items-refresh');await f.row('item-rows');
 assert.deepEqual(f.calls.map(c=>c.url),['/items/list','/items/read']);
 assert.equal(f.calls[1].body.item_id,'item-9007199254740993');
 assert.match(f.text('item-detail'),/9007199254740995/);assert.match(f.text('item-detail'),/0\.001 hours/);assert.match(f.text('item-detail'),/  exact text  /);
 assert.match(f.text('item-rows'),/<script>exact<\/script>/);
});
test('item and pending cursors remain separate exact decimal strings',async()=>{
 const f=fixture((url,body)=>url.endsWith('/list')?{status:200,data:url==='/items/list'?{outcome:'listed',entries:[summary(body.before?'9':'9007199254740993')],before:body.before?null:'9007199254740993'}:{entries:[{...pending(),request_id:body.before?'8':'9007199254740997'}],before:body.before?null:'9007199254740997'}}:undefined);
 f.view.useProject(current(),'p');await f.fire('items-refresh');await f.fire('pending-refresh');await f.fire('items-older');await f.fire('pending-older');
 assert.equal(f.calls[2].body.before,'9007199254740993');assert.equal(f.calls[3].body.before,'9007199254740997');
});
test('frozen pending command is inspected without execution or an issued number',async()=>{
 const f=fixture();f.view.useProject(current(),'p');await f.fire('pending-refresh');await f.row('pending-rows');
 assert.equal(f.calls[1].url,'/items/pending/open');assert.equal(f.calls[1].body.intent_handle,'a'.repeat(32));
 assert.match(f.text('pending-detail'),/  frozen title  /);assert.match(f.text('pending-detail'),/0 hours/);assert.doesNotMatch(f.text('pending-detail'),/PR-/);
});
test('pruned completed locator remains available for an explicit current item read',async()=>{
 const entry=pending('completed');const f=fixture(url=>url==='/items/pending/list'?{status:200,data:{entries:[entry],before:null}}:url==='/items/pending/open'?{status:200,data:opened(entry)}:url==='/items/read'?{status:200,data:itemRead({...summary(),item_id:'target',readable_id:{...summary().readable_id,item_id:'target'}})}:undefined);
 f.view.useProject(current(),'p');await f.fire('pending-refresh');await f.row('pending-rows');assert.match(f.text('pending-detail'),/target/);assert.equal(f.calls.length,2);await f.fire('pending-read-item');assert.equal(f.calls[2].body.item_id,'target');
});
test('failed or mismatched current read clears the earlier displayed payload',async()=>{
 let reads=0;const f=fixture(url=>url==='/items/read'&&++reads>1?{status:503,data:{error:'unavailable'}}:undefined);
 f.view.useProject(current(),'p');await f.fire('items-refresh');await f.row('item-rows');assert.match(f.text('item-detail'),/exact text/);await f.row('item-rows');assert.equal(f.text('item-detail'),'');assert.match(f.text('items-status'),/unavailable/i);
});
test('wrong project, handle, or command attribution cannot be displayed',async()=>{
 for(const corrupt of [data=>{data.entry.project_id='other';},data=>{data.entry.intent_handle='c'.repeat(32);},data=>{data.request.value.item_id='wrong';}]){
  const data=opened();corrupt(data);const f=fixture(url=>url==='/items/pending/open'?{status:200,data}:undefined);f.view.useProject(current(),'p');await f.fire('pending-refresh');await f.row('pending-rows');assert.equal(f.text('pending-detail'),'');assert.match(f.text('items-status'),/invalid|mismatch/i);
 }
});
test('scope replacement discards an in-flight old page and disables old detail actions',async()=>{
 let release;const f=fixture(url=>url==='/items/list'?new Promise(resolve=>{release=resolve;}):undefined);f.view.useProject(current(),'p');const job=f.fire('items-refresh');await new Promise(setImmediate);f.view.useProject(current('q'),'q');release({status:200,data:{outcome:'listed',entries:[summary()],before:null}});await job;assert.equal(f.elements.get('item-rows').children.length,0);assert.match(f.text('items-project'),/Project q/);
});
test('malformed pages and binding failures cannot leave actionable cached data',async()=>{
 const f=fixture(url=>url==='/items/list'?{status:200,data:{outcome:'listed',entries:Array(17).fill(summary()),before:null}}:undefined);f.view.useProject(current(),'p');await f.fire('items-refresh');assert.equal(f.elements.get('item-rows').children.length,0);assert.match(f.text('items-status'),/invalid/i);
 f.binding.failure=true;await f.fire('pending-refresh');assert.equal(f.elements.get('work-items').hidden,true);assert.equal(f.calls.length,1);
});

test('detail field values use whitespace-preserving elements rather than display fallback',async()=>{
 const f=fixture();f.view.useProject(current(),'p');await f.fire('items-refresh');await f.row('item-rows');
 const descendants=el=>[el,...el.children.flatMap(descendants)];
 assert.ok(descendants(f.elements.get('item-detail')).some(el=>el.tag==='pre'&&el.textContent==='  exact text  '));
 await f.fire('pending-refresh');await f.row('pending-rows');assert.ok(descendants(f.elements.get('pending-detail')).some(el=>el.tag==='pre'&&el.textContent==='  frozen title  '));
});
test('late transport failure cannot overwrite the replacement project status',async()=>{
 let reject;const f=fixture(url=>url==='/items/list'?new Promise((resolve,fail)=>{reject=fail;}):undefined);
 f.view.useProject(current(),'p');const job=f.fire('items-refresh');await new Promise(setImmediate);f.view.useProject(current('q'),'q');const message=f.text('items-status');reject(new Error('old project failed'));await job;assert.equal(f.text('items-status'),message);
});

// Creation request and cancellation behavior specifications. Execute only when
// the separately held resource boundary permits container checks again.
function formCurrent() {
 const value=current();value.project.value.configuration_revision='9007199254740995';
 value.configuration.value={project_id:'p',revision:'9007199254740995',project_archived:false,
  types:[{id:'configured-task',permitted_workflows:['flow','alternate'],default_workflow_id:'flow',title_field_id:'task-title',archived:false},{id:'configured-milestone',permitted_workflows:['flow'],default_workflow_id:'flow',title_field_id:'milestone-title',archived:false},{id:'archived-type',permitted_workflows:['flow'],default_workflow_id:'flow',title_field_id:null,archived:true}],
  workflows:[{id:'flow',initial_status_id:'new',status_ids:['new'],archived:false},{id:'alternate',initial_status_id:'new',status_ids:['new'],archived:false}],
  statuses:[{id:'new',name:'New',phase:'new',archived:false}],
  fields:[{id:'task-title',owner_type:'configured-task',name:'Task title',kind:'text',usage:'optional',archived:false},{id:'task-description',owner_type:'configured-task',name:'Description',kind:'text',usage:'optional',archived:false},{id:'hidden-text',owner_type:'configured-task',name:'Hidden text',kind:'text',usage:'hidden',archived:false},{id:'archived-text',owner_type:'configured-task',name:'Archived text',kind:'text',usage:'optional',archived:true},{id:'milestone-title',owner_type:'configured-milestone',name:'Milestone title',kind:'text',usage:'required',archived:false}]};
 return value;
}
const descendants=element=>[element,...element.children.flatMap(descendants)];
function includeField(f,id,text) {
 const nodes=descendants(f.elements.get('item-create-fields'));
 const input=nodes.find(node=>node.tag==='textarea'&&node.name===id);assert.ok(input,'configured textarea '+id);
 const included=nodes.find(node=>node.tag==='input'&&node.name===id+'_included');assert.ok(included,'explicit presence '+id);
 included.checked=true;included.events.change?.();input.value=text;return input;
}
async function startForm(f,configuration=formCurrent()) { f.view.useProject(configuration,'p');await f.fire('item-new-request'); }
const registered=(index=0)=>({...pending(),request_id:index?'9007199254740998':'9007199254740997',intent_handle:(index?'c':'a').repeat(32),token:(index?'d':'b').repeat(32),target_item_id:index?'second-target':'target'});

test('registration snapshots configured exact text, zero estimate and string revision without caller identities',async()=>{
 const f=fixture(url=>url==='/items/pending/register'?{status:201,data:{entry:registered()}}:undefined);
 await startForm(f);includeField(f,'task-title','  exact\n\u{1f642}  ');includeField(f,'task-description','');f.elements.get('item-estimate').value='0';await f.fire('item-create-form','submit');
 const call=f.calls.find(call=>call.url==='/items/pending/register');assert.ok(call);
 assert.deepEqual(JSON.parse(JSON.stringify(call.body)),{project_id:'p',type_id:'configured-task',workflow_id:null,expected_configuration_revision:'9007199254740995',estimate:'0',values:[{id:'task-title',value:'  exact\n\u{1f642}  '},{id:'task-description',value:''}]});
 assert.equal(f.calls.some(call=>/execute|reconcile|edit/.test(call.url)),false);
});
test('omitted optional fields and absent estimate differ from included empty text and zero',async()=>{
 let count=0;const f=fixture(url=>url==='/items/pending/register'?{status:201,data:{entry:registered(count++)}}:undefined);
 await startForm(f);await f.fire('item-create-form','submit');await f.fire('item-new-request');includeField(f,'task-title','');f.elements.get('item-estimate').value='0';await f.fire('item-create-form','submit');
 const calls=f.calls.filter(call=>call.url==='/items/pending/register');assert.equal(calls.length,2);assert.equal(calls[0].body.estimate,null);assert.equal(calls[0].body.values.length,0);assert.equal(calls[1].body.values[0].value,'');assert.equal(calls[1].body.estimate,'0');
});
test('type/workflow selection uses configuration and cannot submit hidden, archived or other-type fields',async()=>{
 const f=fixture(url=>url==='/items/pending/register'?{status:201,data:{entry:registered()}}:undefined);await startForm(f);
 const nodes=descendants(f.elements.get('item-create-fields'));assert.equal(nodes.some(node=>['hidden-text','archived-text','milestone-title'].includes(node.name)),false);
 includeField(f,'task-title','must not leak');f.elements.get('item-kind').value='configured-milestone';await f.fire('item-kind','change');includeField(f,'milestone-title','  milestone  ');f.elements.get('item-workflow').value='flow';await f.fire('item-create-form','submit');
 const body=f.calls.find(call=>call.url==='/items/pending/register').body;assert.equal(body.type_id,'configured-milestone');assert.equal(body.workflow_id,'flow');assert.deepEqual(JSON.parse(JSON.stringify(body.values)),[{id:'milestone-title',value:'  milestone  '}]);
});
test('required nontext or archived project is surfaced without knowingly sending an unusable text form',async()=>{
 for(const unsupported of [true,false]){
  const configuration=formCurrent();if(unsupported)configuration.configuration.value.fields.push({id:'required-number',owner_type:'configured-task',name:'Number',kind:'number',usage:'required',archived:false});else {configuration.project.value.archived=true;configuration.configuration.value.project_archived=true;}
  const f=fixture();await startForm(f,configuration);f.elements.get('item-kind').value='configured-task';await f.fire('item-kind','change');await f.fire('item-create-form','submit');assert.equal(f.calls.some(call=>call.url==='/items/pending/register'),false);assert.match(f.text('items-status'),/required|unsupported|archived|unavailable/i);
 }
});
test('exact decimal estimates reject rounding, exponent, sign and noncanonical integer syntax',async()=>{
 for(const [text,expected] of [['999999.999','999999999'],['0.001','1'],['1.010','1010'],['1e3',null],['-1',null],['01',null],['0.0001',null],['1000000',null]]){
  const f=fixture(url=>url==='/items/pending/register'?{status:201,data:{entry:registered()}}:undefined);await startForm(f);f.elements.get('item-estimate').value=text;await f.fire('item-create-form','submit');const call=f.calls.find(call=>call.url==='/items/pending/register');if(expected===null)assert.equal(call,undefined,text);else assert.equal(call.body.estimate,expected,text);
 }
});
test('pre-transmission domain UTF8 and encoded-body failures preserve editable input without claiming an uncertain save',async()=>{
 for(const encodedOverflow of [false,true]){
  const configuration=formCurrent();if(encodedOverflow)for(let i=0;i<11;i++)configuration.configuration.value.fields.push({id:'escape-'+i,owner_type:'configured-task',name:'Escape '+i,kind:'text',usage:'optional',archived:false});
  const f=fixture();await startForm(f,configuration);let input;if(encodedOverflow)for(let i=0;i<11;i++)input=includeField(f,'escape-'+i,'\u0001'.repeat(16384));else input=includeField(f,'task-title','\u{1f642}'.repeat(4097));
  const original=input.value;if(encodedOverflow){assert.equal(new TextEncoder().encode(original).byteLength,16384);assert.ok(new TextEncoder().encode(JSON.stringify(original)).byteLength*11>1048576);}else assert.equal(new TextEncoder().encode(original).byteLength,16388);await f.fire('item-create-form','submit');assert.equal(f.calls.some(call=>call.url==='/items/pending/register'),false);assert.equal(input.value,original);assert.equal(f.elements.get('item-dismiss-save').hidden,true);assert.match(f.text('items-status'),/limit|large|size|bytes/i);
 }
});
test('lost registration acknowledgement survives discovery and a separately confirmed save without content adoption',async()=>{
 let count=0;const f=fixture(url=>{if(url==='/items/pending/register'){if(count++===0)throw new Error('lost save acknowledgement');return {status:201,data:{entry:registered(1)}};}});
 await startForm(f);includeField(f,'task-title','same form');await f.fire('item-create-form','submit');await f.fire('pending-refresh');assert.equal(f.calls.filter(call=>call.url==='/items/pending/register').length,1);assert.equal(f.calls.filter(call=>call.url==='/items/pending/open').length,0);
 await f.fire('item-new-request');includeField(f,'task-title','same form');await f.fire('item-create-form','submit');assert.equal(f.calls.filter(call=>call.url==='/items/pending/register').length,2);assert.match(f.text('item-uncertainty'),/unconfirmed/i);assert.equal(f.elements.get('item-dismiss-save').hidden,false);
 await f.fire('item-dismiss-save');assert.equal(f.elements.get('item-dismiss-save').hidden,true);assert.match(f.text('items-status'),/dismiss|identity|reject/i);
});
test('known registration rejection restores exact form without deleting an earlier unknown-save warning',async()=>{
 for(const status of [400,429]){let saves=0;const f=fixture(url=>{if(url==='/items/pending/register'){if(saves++===0)throw new Error('first unknown');return {status,data:{error:'rejected'}};}});await startForm(f);await f.fire('item-create-form','submit');await f.fire('item-new-request');const input=includeField(f,'task-title','  retain exact  ');await f.fire('item-create-form','submit');assert.equal(f.elements.get('item-create-form').hidden,false);assert.equal(input.value,'  retain exact  ');assert.match(f.text('item-uncertainty'),/unconfirmed/i);}
});
test('cancellation requires confirmation bound to the selected locator and project',async()=>{
 const first=pending(),second={...pending(),request_id:'9007199254740996',intent_handle:'c'.repeat(32),token:'d'.repeat(32),target_item_id:'second-target'};
 const f=fixture((url,body)=>url==='/items/pending/list'?{status:200,data:{entries:[first,second],before:null}}:url==='/items/pending/open'?{status:200,data:opened(body.intent_handle===first.intent_handle?first:second)}:undefined);
 f.view.useProject(formCurrent(),'p');await f.fire('pending-refresh');await f.row('pending-rows');await f.fire('pending-cancel');assert.equal(f.calls.some(call=>call.url==='/items/pending/cancel'),false);await f.row('pending-rows',1);await f.fire('pending-confirm-cancel');assert.equal(f.calls.some(call=>call.url==='/items/pending/cancel'),false);
});
test('completed cancellation response preserves target for a separate current read and never claims undo',async()=>{
 const f=fixture(url=>url==='/items/pending/cancel'?{status:200,data:{entry:pending('completed')}}:url==='/items/read'?{status:200,data:itemRead({...summary(),item_id:'target',readable_id:{...summary().readable_id,item_id:'target'}})}:undefined);
 f.view.useProject(formCurrent(),'p');await f.fire('pending-refresh');await f.row('pending-rows');await f.fire('pending-cancel');await f.fire('pending-confirm-cancel');const cancel=f.calls.find(call=>call.url==='/items/pending/cancel');assert.deepEqual(JSON.parse(JSON.stringify(cancel.body)),{project_id:'p',intent_handle:'a'.repeat(32)});assert.equal(f.elements.get('pending-read-item').disabled,false);await f.fire('pending-read-item');assert.equal(f.calls.at(-1).body.item_id,'target');assert.equal(f.calls.some(call=>call.url.includes('register')),false);
});
test('lost cancellation retains its exact locator through prepared Open and only an explicit same-locator retry',async()=>{
 let cancels=0;const f=fixture(url=>{if(url==='/items/pending/cancel'){if(cancels++===0)throw new Error('lost cancellation');return {status:200,data:{entry:pending('cancelled')}};}});
 f.view.useProject(formCurrent(),'p');await f.fire('pending-refresh');await f.row('pending-rows');await f.fire('pending-cancel');await f.fire('pending-confirm-cancel');assert.match(f.text('item-uncertainty'),/unconfirmed/i);await f.fire('pending-check-cancel');assert.match(f.text('item-uncertainty'),/unconfirmed/i);assert.equal(f.calls.filter(call=>call.url==='/items/pending/cancel').length,1);
 await f.fire('pending-cancel');await f.fire('pending-confirm-cancel');assert.equal(f.calls.filter(call=>call.url==='/items/pending/cancel').length,2);assert.doesNotMatch(f.text('item-uncertainty'),/unconfirmed/i);assert.match(f.text('pending-detail'),/cancelled/);
});

test('required text checks Unicode whitespace without trimming or normalizing the saved value',async()=>{
 const f=fixture(url=>url==='/items/pending/register'?{status:201,data:{entry:registered()}}:undefined);await startForm(f);f.elements.get('item-kind').value='configured-milestone';await f.fire('item-kind','change');
 const input=includeField(f,'milestone-title',' \u0085 ');await f.fire('item-create-form','submit');assert.equal(f.calls.some(call=>call.url==='/items/pending/register'),false);input.value='\uFEFF';await f.fire('item-create-form','submit');assert.equal(f.calls.find(call=>call.url==='/items/pending/register').body.values[0].value,'\uFEFF');
});

test('the exact 16KiB UTF8 domain boundary remains accepted without counting UTF16 code units',async()=>{
 const f=fixture(url=>url==='/items/pending/register'?{status:201,data:{entry:registered()}}:undefined);await startForm(f);const text='\u{1f642}'.repeat(4096);assert.equal(new TextEncoder().encode(text).byteLength,16384);includeField(f,'task-title',text);await f.fire('item-create-form','submit');assert.equal(f.calls.find(call=>call.url==='/items/pending/register').body.values[0].value,text);
});
test('changing project or explicitly forgetting invalidates a pending cancellation confirmation',async()=>{
 for(const forget of [false,true]){const f=fixture();f.view.useProject(formCurrent(),'p');await f.fire('pending-refresh');await f.row('pending-rows');await f.fire('pending-cancel');assert.equal(f.elements.get('pending-cancel-confirmation').hidden,false);
  if(forget)f.view.clear(true);else {const next=formCurrent();next.project.value.id='q';next.configuration.value.project_id='q';f.view.useProject(next,'q');}
  await f.fire('pending-confirm-cancel');assert.equal(f.calls.some(call=>call.url==='/items/pending/cancel'),false);}
});
test('unknown save warning survives ordinary context clearing but explicit forgetting clears owner state',async()=>{
 const f=fixture(url=>{if(url==='/items/pending/register')throw new Error('unknown save');});await startForm(f);await f.fire('item-create-form','submit');assert.match(f.text('item-uncertainty'),/unconfirmed/i);f.view.clear(false);f.view.useProject(formCurrent(),'p');assert.match(f.text('item-uncertainty'),/unconfirmed/i);
 f.view.clear(true);f.view.useProject(formCurrent(),'p');assert.doesNotMatch(f.text('item-uncertainty'),/unconfirmed/i);assert.equal(f.calls.filter(call=>call.url==='/items/pending/register').length,1);
});
test('one unknown cancellation blocks a different locator and survives project replacement until explicitly forgotten',async()=>{
 const first=pending(),second={...pending(),request_id:'9007199254740996',intent_handle:'c'.repeat(32),token:'d'.repeat(32),target_item_id:'second-target'};
 const f=fixture((url,body)=>{if(url==='/items/pending/list')return {status:200,data:{entries:[first,second],before:null}};if(url==='/items/pending/open')return {status:200,data:opened(body.intent_handle===first.intent_handle?first:second)};if(url==='/items/pending/cancel')throw new Error('unknown cancel');});
 f.view.useProject(formCurrent(),'p');await f.fire('pending-refresh');await f.row('pending-rows');await f.fire('pending-cancel');await f.fire('pending-confirm-cancel');assert.match(f.text('item-uncertainty'),/unconfirmed/i);
 const next=formCurrent();next.project.value.id='q';next.configuration.value.project_id='q';f.view.useProject(next,'q');assert.equal(f.elements.get('pending-check-cancel').disabled,true);
 f.view.useProject(formCurrent(),'p');assert.equal(f.elements.get('pending-check-cancel').disabled,false);await f.fire('pending-check-cancel');assert.equal(f.calls.at(-1).body.intent_handle,first.intent_handle);assert.match(f.text('item-uncertainty'),/unconfirmed/i);
 await f.fire('pending-refresh');await f.row('pending-rows',1);assert.equal(f.elements.get('pending-cancel').disabled,true);await f.fire('pending-cancel');await f.fire('pending-confirm-cancel');assert.equal(f.calls.filter(call=>call.url==='/items/pending/cancel').length,1);
 f.view.clear(true);f.view.useProject(formCurrent(),'p');assert.doesNotMatch(f.text('item-uncertainty'),/unconfirmed/i);
});


test('explicitly stopping cancellation tracking after missing Open frees the slot without claiming rollback or sending another mutation',async()=>{
 const first=pending(),second={...pending(),request_id:'9007199254740996',intent_handle:'c'.repeat(32),token:'d'.repeat(32),target_item_id:'second-target'};
 let missing=false;const f=fixture((url,body)=>{if(url==='/items/pending/list')return {status:200,data:{entries:[first,second],before:null}};if(url==='/items/pending/open')return missing&&body.intent_handle===first.intent_handle?{status:404,data:{error:'not found'}}:{status:200,data:opened(body.intent_handle===first.intent_handle?first:second)};if(url==='/items/pending/cancel')throw new Error('unknown cancel');});
 f.view.useProject(formCurrent(),'p');await f.fire('pending-refresh');await f.row('pending-rows');await f.fire('pending-cancel');await f.fire('pending-confirm-cancel');missing=true;await f.fire('pending-check-cancel');assert.match(f.text('item-uncertainty'),/unconfirmed/i);
 await f.fire('pending-refresh');await f.row('pending-rows',1);assert.equal(f.elements.get('pending-cancel').disabled,true);
 const before=f.calls.length,beforeBinding=f.binding.checks;await f.fire('pending-stop-tracking');assert.equal(f.binding.checks,beforeBinding+1);assert.equal(f.calls.length,before);assert.equal(f.elements.get('pending-cancel').disabled,false);assert.equal(f.elements.get('pending-check-cancel').disabled,true);assert.match(f.text('item-uncertainty'),/unconfirmed/i);assert.match(f.text('items-status'),/not prove|does not prove/i);
 f.view.clear(false);f.view.useProject(formCurrent(),'p');assert.match(f.text('item-uncertainty'),/unconfirmed/i);f.view.clear(true);f.view.useProject(formCurrent(),'p');assert.doesNotMatch(f.text('item-uncertainty'),/unconfirmed/i);
});

// Edit recovery source specifications; DOM simulation, not browser qualification.
function editableRead() {
 const value=itemRead();value.result.value.project={id:'p',name:'Project p',prefix:'PR',estimate_unit:'hours',configuration_revision:'9007199254740995',next_sequence:'2',ever_estimated:true,archived:false};
 value.result.value.item.type_id='configured-task';value.result.value.data.values=[{id:'task-title',value:{kind:'text',value:'  initial\r\n\u{e9}  '}}];return value;
}
async function editForm(f) {
 f.view.useProject(formCurrent(),'p');await f.fire('items-refresh');await f.row('item-rows');await f.fire('item-edit-start');
}
async function observed(predicate) {
 for(let i=0;i<500&&!predicate();i++)await new Promise(resolve=>setTimeout(resolve,1));
 assert.ok(predicate(),'expected asynchronous boundary was reached');
}

test('an edit is not transmitted before its durable journal commit',async()=>{
 const f=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:undefined);
 await editForm(f);let release;
 f.store.hook(()=>new Promise(resolve=>{release=resolve;}));
 const save=f.fire('item-edit-form','submit');
 for(let i=0;i<100&&!release&&!f.calls.some(call=>call.wire);i++)await new Promise(resolve=>setTimeout(resolve,1));
 assert.equal(f.calls.some(call=>call.wire),false);assert.equal(typeof release,'function');
 release();await save;assert.equal(f.calls.filter(call=>call.url==='/items/edit').length,1);
});

test('a second page restores exact edit identity without automatically transmitting it',async()=>{
 const store=memoryStore();
 const first=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:url==='/items/edit'?Promise.reject(new Error('reply lost')):undefined,store);
 await editForm(first);await first.fire('item-edit-form','submit');
 const original=first.calls.find(call=>call.wire);
 const next=fixture((url,body)=>url==='/items/reconcile'?editSuccess(body):undefined,store);
 await next.view.restoreRecovery();assert.equal(next.calls.length,0);
 assert.equal(next.elements.get('item-edit-retry').disabled,false);
 await next.fire('item-edit-check');assert.equal(next.calls.at(-1).wire,original.wire);
 assert.match(next.text('item-mutation-receipt'),/historical|recorded/i);
});

test('two pages preparing edits cannot overwrite the single durable slot or both transmit',async()=>{
 const store=memoryStore(), handler=url=>url==='/items/read'?{status:200,data:editableRead()}:undefined;
 const a=fixture(handler,store),b=fixture(handler,store);
 await editForm(a);await editForm(b);b.binding.randomSeed=34;
 await Promise.all([a.fire('item-edit-form','submit'),b.fire('item-edit-form','submit')]);
 assert.equal([...a.calls,...b.calls].filter(call=>call.url==='/items/edit').length,1);
});

test('a delayed binding cannot restore recovery actions after local signout',async()=>{
 const f=fixture();let release;
 f.binding.wait=new Promise(resolve=>{release=resolve;});
 const restoring=f.view.restoreRecovery();
 for(let i=0;i<100&&f.binding.checks===0;i++)await new Promise(resolve=>setTimeout(resolve,1));
 assert.ok(f.binding.checks>0);
 await f.view.signOut(f.binding.value);
 release();await restoring;
 assert.equal(f.elements.get('item-edit-retry').disabled,true);
});

test('failed local abandon blocks recovery sends and automatic restoration in the detecting page',async()=>{
 const f=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:url==='/items/edit'?Promise.reject(new Error('unknown')):undefined);
 await editForm(f);await f.fire('item-edit-form','submit');
 f.store.hook(()=>{throw new Error('delete aborted');});
 await f.fire('item-edit-forget');
 assert.equal(f.elements.get('item-edit-check').disabled,true);assert.equal(f.elements.get('item-edit-retry').disabled,true);
 const count=f.calls.length;await f.fire('item-edit-retry');assert.equal(f.calls.length,count);
 await f.view.restoreRecovery();assert.equal(f.elements.get('item-edit-retry').disabled,true);
});

test('missing-binding signout failure remains blocked after an explicit sign-in callback',async()=>{
 const f=fixture();await f.view.restoreRecovery();
 await assert.rejects(f.view.signOut(null),/binding/i);
 f.view.startSession();const before=f.binding.checks;
 await f.view.restoreRecovery();
 assert.equal(f.binding.checks,before);
 assert.equal(f.elements.get('item-edit-restore').disabled,true);
 assert.match(f.text('item-recovery-status'),/reload/i);
 const control=fixture();await control.view.restoreRecovery();
 await control.view.signOut(control.binding.value);control.view.startSession();
 await control.view.restoreRecovery();assert.equal(control.elements.get('item-edit-restore').disabled,false);
});

test('suspending a page hides retained content and requires fresh binding before recovery actions resume',async()=>{
 const f=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:url==='/items/edit'?Promise.reject(new Error('unknown')):undefined);
 await editForm(f);await f.fire('item-edit-form','submit');
 const count=f.calls.length;
 f.view.suspendRecovery();
 assert.equal(f.elements.get('item-recovery').hidden,true);
 assert.equal(f.elements.get('item-edit-retry').disabled,true);
 await f.fire('item-edit-retry');assert.equal(f.calls.length,count);
 const checks=f.binding.checks;await f.view.restoreRecovery();
 assert.ok(f.binding.checks>checks);assert.equal(f.elements.get('item-recovery').hidden,false);
 assert.equal(f.calls.length,count);
});

test('a failed suspended restore leaves payload hidden with an accessible error and restore action',async()=>{
 const f=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:undefined);
 await editForm(f);await f.fire('item-edit-form','submit');f.view.suspendRecovery();
 const read=f.store.read;f.store.read=async()=>{throw new Error('storage unavailable');};
 await f.view.restoreRecovery();
 assert.equal(f.elements.get('item-recovery').hidden,true);
 assert.equal(f.elements.get('item-edit-restore').disabled,false);
 assert.equal(f.elements.get('item-recovery-error').hidden,false);
 assert.match(f.text('item-recovery-error'),/storage unavailable/);
 f.store.read=read;await f.fire('item-edit-restore');
 assert.equal(f.elements.get('item-recovery').hidden,false);
 assert.equal(f.elements.get('item-recovery-error').hidden,true);
 assert.equal(f.calls.filter(call=>call.wire).length,1);
});

test('unchanged-owner recovery revalidation preserves unsaved edit and creation input',async()=>{
 const f=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:undefined);
 await editForm(f);const input=editField(f,'task-title','set','  unsaved edit  ');
 f.view.suspendRecovery();await f.view.restoreRecovery();
 assert.equal(f.elements.get('item-edit-form').hidden,false);assert.equal(input.value,'  unsaved edit  ');
 const creation=fixture();creation.view.useProject(formCurrent(),'p');await creation.fire('item-new-request');
 creation.elements.get('item-estimate').value='1.250';
 creation.view.suspendRecovery();await creation.view.restoreRecovery();
 assert.equal(creation.elements.get('item-create-form').hidden,false);
 assert.equal(creation.elements.get('item-estimate').value,'1.250');
});

test('suspension cannot erase knowledge of a pending cleanup failure in this page',async()=>{
 const f=fixture();await f.view.restoreRecovery();let reject;
 f.store.hook(()=>new Promise((resolve,failed)=>{reject=failed;}));
 const signingOut=f.view.signOut(f.binding.value);
 await observed(()=>reject);f.view.suspendRecovery();reject(new Error('delete failed while hidden'));
 await signingOut;f.view.startSession();const count=f.binding.checks;
 await f.view.restoreRecovery();
 assert.equal(f.binding.checks,count);assert.equal(f.elements.get('item-edit-restore').disabled,true);
});

test('a changed owner discovered during resumption clears old unsaved edit content',async()=>{
 const f=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:undefined);
 await editForm(f);editField(f,'task-title','set','private old-owner draft');
 f.view.suspendRecovery();f.binding.value={installation_id:'installation',actor_id:'different'};
 await f.view.restoreRecovery();
 assert.equal(f.elements.get('item-edit-form').hidden,true);
 assert.equal(f.elements.get('item-edit-fields').children.length,0);
 assert.equal(f.calls.some(call=>call.wire),false);
});

test('failed receipt cleanup blocks further sends and retains the same identity',async()=>{
 let f;
 f=fixture((url,body)=>{
  if(url==='/items/read')return {status:200,data:editableRead()};
  if(url==='/items/edit'){f.store.hook(()=>{throw new Error('receipt cleanup aborted');});return editSuccess(body,'edited');}
 });
 await editForm(f);await f.fire('item-edit-form','submit');
 assert.equal(f.elements.get('item-edit-check').disabled,true);assert.equal(f.elements.get('item-edit-retry').disabled,true);
 assert.equal(f.calls.filter(call=>call.wire).length,1);
});

test('failed durable retry closure blocks both recovery actions',async()=>{
 let f;
 f=fixture(url=>{
  if(url==='/items/read')return {status:200,data:editableRead()};
  if(url==='/items/edit'){f.store.hook(()=>{throw new Error('closure aborted');});return {status:410,data:{error:'replay_expired'}};}
 });
 await editForm(f);await f.fire('item-edit-form','submit');
 assert.equal(f.elements.get('item-edit-check').disabled,true);assert.equal(f.elements.get('item-edit-retry').disabled,true);
});

test('post-dispatch expiry still adopts matching success and replay-expiry responses',async()=>{
 for(const status of [200,410]){
  let f;f=fixture((url,body)=>{
   if(url==='/items/read')return {status:200,data:editableRead()};
   if(url==='/items/edit'){
    f.clock.now+=7*86400000+1;
    return status===200?editSuccess(body,'edited'):{status:410,data:{error:'replay_expired'}};
   }
  });
  await editForm(f);await f.fire('item-edit-form','submit');
  assert.equal(f.calls.filter(call=>call.wire).length,1);
  if(status===200){
   assert.match(f.text('item-mutation-receipt'),/Recorded edit/);
   assert.equal(JSON.parse(await f.store.read()).entry,null);
  }else{
   assert.match(f.text('item-recovery-status'),/expired/);
   assert.equal(f.elements.get('item-edit-retry').disabled,true);
   assert.equal(f.elements.get('item-edit-restore').disabled,false);
   await f.fire('item-edit-check');assert.equal(f.calls.filter(call=>call.wire).length,1);
  }
  assert.doesNotMatch(f.text('item-recovery-status'),/cleanup could not be confirmed/);
 }
});

test('cross-tab replacement during a dispatched edit visibly blocks stale actions and preserves replacement',async()=>{
 let f,replacement;
 f=fixture(async(url,body)=>{
  if(url==='/items/read')return {status:200,data:editableRead()};
  if(url==='/items/edit'){
   const original=JSON.parse(await f.store.read()).entry.retained;
   const other=await f.ports.recovery.open(f.binding.value);await other.discard();
   const wire=JSON.parse(original.wire);wire.operation_id='2'.repeat(32);
   await other.retain({...original,wire:JSON.stringify(wire)});replacement=await f.store.read();
   return editSuccess(body,'edited');
  }
 });
 await editForm(f);await f.fire('item-edit-form','submit');
 assert.equal(f.elements.get('item-edit-retry').disabled,true);
 assert.equal(f.elements.get('item-edit-check').disabled,true);
 assert.equal(f.elements.get('item-edit-restore').disabled,false);
 assert.match(f.text('item-recovery-status'),/changed in another tab.*[Rr]estore/);
 assert.match(f.text('item-recovery-status'),/does not cancel|unconfirmed/);
 assert.equal(f.text('item-mutation-receipt'),'');
 assert.equal(await f.store.read(),replacement);
});

test('transport and response-body errors retain explicit uncertainty guidance for edits and creations',async()=>{
 for(const family of ['edit','creation'])for(const reason of ['Failed to fetch','The user aborted a request.','Invalid response body']){
  const f=fixture(url=>{
   if(url==='/items/read')return {status:200,data:editableRead()};
   if(url==='/items/edit'||url==='/items/pending/execute')throw new Error(reason);
  });
  if(family==='edit'){await editForm(f);await f.fire('item-edit-form','submit');}
  else{await selectedCreation(f);await f.fire('pending-execute');}
  assert.match(f.text('item-recovery-status'),/unconfirmed.*may have committed/i);
  assert.match(f.text('item-recovery-status'),/check|retry/i);
  assert.ok(f.text('item-recovery-status').includes(reason));
  assert.equal(f.calls.filter(call=>call.wire).length,1);
 }
});

test('a creation transport failure arriving after suspension cannot overwrite the resumed status',async()=>{
 let reject;
 const f=fixture(url=>url==='/items/pending/execute'?new Promise((resolve,failed)=>{reject=failed;}):undefined);
 await selectedCreation(f);const saving=f.fire('pending-execute');
 await observed(()=>reject);f.view.suspendRecovery();await f.view.restoreRecovery();
 const before=f.text('item-recovery-status');reject(new Error('late transport failure'));await saving;
 assert.equal(f.text('item-recovery-status'),before);
 assert.equal(f.calls.filter(call=>call.wire).length,1);
});

test('unavailable edit storage does not disable server-owned creation recovery',async()=>{
 const store=memoryStore();store.read=async()=>{throw new Error('IndexedDB unavailable');};
 const f=fixture((url,body)=>url==='/items/pending/execute'?Promise.reject(new Error('unknown')):
  url==='/items/pending/reconcile'?creationSuccess(pending(),'replayed'):undefined,store);
 await f.view.restoreRecovery();await selectedCreation(f);await f.fire('pending-execute');
 assert.equal(f.elements.get('item-edit-check').disabled,false);assert.equal(f.elements.get('item-edit-retry').disabled,false);
 await f.fire('item-edit-check');assert.match(f.text('item-mutation-receipt'),/recorded|historical/i);
});
function editField(f,id,action,text) {
 const nodes=descendants(f.elements.get('item-edit-fields'));
 const select=nodes.find(node=>node.tag==='select'&&node.name===id+'_action');assert.ok(select);select.value=action;select.events.change?.();
 const input=nodes.find(node=>node.tag==='textarea'&&node.name===id);assert.ok(input);if(text!==undefined)input.value=text;return input;
}
function editSuccess(body,outcome='replayed') {
 const result=editableRead().result;result.value.project.id=body.project_id;result.value.item.id=body.item_id;result.value.item.project_id=body.project_id;result.value.item.version=(BigInt(body.expected_item_version)+1n).toString();
 result.value.project.configuration_revision=body.expected_configuration_revision;
 if(body.estimate.action==='set')result.value.data.estimate=body.estimate.value;
 if(body.estimate.action==='clear')result.value.data.estimate=null;
 for(const field of body.fields){result.value.data.values=result.value.data.values.filter(value=>value.id!==field.id);if(field.action==='set')result.value.data.values.push({id:field.id,value:{kind:'text',value:field.value}});}
 return {status:200,data:{outcome,result}};
}
test('edit retry and reconciliation retain exact wire and token across changed controls and project views',async()=>{
 let checks=0;const f=fixture((url,body)=>url==='/items/read'?{status:200,data:editableRead()}:url==='/items/edit'?Promise.reject(new Error('lost edit reply')):url==='/items/reconcile'?++checks===1?{status:404,data:{error:'operation_not_found'}}:editSuccess(body):undefined);
 await editForm(f);const input=editField(f,'task-title','set','  saved\r\n\u{e9}  ');f.elements.get('item-edit-estimate-action').value='set';f.elements.get('item-edit-estimate').value='0';
 await f.fire('item-edit-form','submit');const original=f.calls.find(call=>call.url==='/items/edit');assert.ok(original.wire);assert.equal(original.body.operation_id,'11'.repeat(16));assert.equal(original.body.estimate.value,'0');
 input.value='different';const other=formCurrent();other.project.value.id='q';other.configuration.value.project_id='q';other.configuration.value.revision='99';other.project.value.configuration_revision='99';other.project.value.name='Project q';f.view.useProject(other,'q');
 await f.fire('item-edit-check');assert.match(f.text('item-recovery-status'),/in.flight|unconfirmed|lookup/i);await f.fire('item-edit-retry');
 for(const call of f.calls.filter(call=>call.wire))assert.equal(call.wire,original.wire);
 await f.fire('item-edit-check');assert.match(f.text('item-mutation-receipt'),/historical|recorded/i);assert.match(f.text('item-mutation-receipt'),/9007199254740996/);
 assert.equal(f.text('items-project'),'Project q');assert.equal(f.calls.filter(call=>call.url==='/items/edit').length,2);
});
test('one unresolved edit cannot be overwritten and explicit abandonment makes no rollback claim',async()=>{
 const f=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:url==='/items/edit'?Promise.reject(new Error('unknown')):undefined);
 await editForm(f);await f.fire('item-edit-form','submit');const first=f.calls.find(call=>call.wire).wire;
 await f.fire('item-edit-form','submit');assert.equal(f.calls.filter(call=>call.wire).length,1);f.view.clear(false);f.view.useProject(formCurrent(),'p');
 await f.fire('item-edit-retry');assert.equal(f.calls.filter(call=>call.wire).at(-1).wire,first);
 await f.fire('item-edit-forget');assert.match(f.text('item-recovery-status'),/may.*commit|does not.*undo|no.*rollback|already.*complete/i);
});
test('RNG and encoding failures prevent edit transmission and leave no unresolved record',async()=>{
 for(const failure of ['randomFailure','encodeFailure']){const f=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:undefined);await editForm(f);f.binding[failure]=true;await f.fire('item-edit-form','submit');assert.equal(f.calls.some(call=>call.wire),false);assert.equal(f.elements.get('item-edit-retry').disabled,true);}
});
test('binding changes block exact recovery before transmission',async()=>{
 const f=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:url==='/items/edit'?Promise.reject(new Error('unknown')):undefined);
 await editForm(f);await f.fire('item-edit-form','submit');f.binding.value={installation_id:'different',actor_id:'owner'};
 await f.fire('item-edit-retry');assert.equal(f.calls.filter(call=>call.wire).length,1);assert.match(f.text('item-recovery-status'),/owner|installation|discard/i);
});
test('sparse editing preserves unchanged CRLF and omits kept hidden archived and nontext fields',async()=>{
 const f=fixture((url,body)=>url==='/items/read'?{status:200,data:editableRead()}:url==='/items/edit'?editSuccess(body,'edited'):undefined);
 await editForm(f);const input=editField(f,'task-title','set');const original=input.value;assert.equal(original,'  initial\n\u{e9}  ');input.value=original;await f.fire('item-edit-form','submit');
 const body=f.calls.find(call=>call.wire).body;assert.deepEqual(body.fields,[{id:'task-title',action:'set',value:'  initial\r\n\u{e9}  '}]);assert.deepEqual(body.estimate,{action:'keep'});
 assert.equal(body.expected_item_version,'9007199254740995');assert.equal(body.expected_configuration_revision,'9007199254740995');
});
test('malformed or misattributed mutation success leaves the exact retry unresolved',async()=>{
 for(const corrupt of [data=>{data.result.value.item.id='other';},data=>{data.result.value.item.version='1';},data=>{data.result.value.project.configuration_revision='2';},data=>{data.result.value.data.estimate='7';}]){
  const f=fixture((url,body)=>{if(url==='/items/read')return {status:200,data:editableRead()};if(url==='/items/edit'){const response=editSuccess(body);corrupt(response.data);return response;}});
  await editForm(f);f.elements.get('item-edit-estimate-action').value='set';f.elements.get('item-edit-estimate').value='0';await f.fire('item-edit-form','submit');assert.equal(f.elements.get('item-edit-retry').disabled,false);assert.equal(f.text('item-mutation-receipt'),'');
 }
});

test('a deferred old success cannot resolve a separately retained replacement edit',async()=>{
 let release, replacement=false, edits=0;
 const f=fixture((url,body)=>{
  if(url==='/items/list'&&replacement){const row=summary();row.item_id='replacement-item';row.readable_id.item_id=row.item_id;return {status:200,data:{outcome:'listed',entries:[row],before:null}};}
  if(url==='/items/read'){const value=editableRead();value.result.value.item.id=body.item_id;value.readable_id.item_id=body.item_id;return {status:200,data:value};}
  if(url==='/items/edit')return ++edits===1?new Promise(resolve=>{release=resolve;}):Promise.reject(new Error('second save unknown'));
  if(url==='/items/reconcile')return editSuccess(body);
 });
 await editForm(f);const old=f.fire('item-edit-form','submit');await observed(()=>release);const first=f.calls.find(call=>call.wire);
 await f.view.signOut(f.binding.value);f.view.startSession();replacement=true;f.binding.randomSeed=34;await editForm(f);await f.fire('item-edit-form','submit');
 const second=f.calls.filter(call=>call.url==='/items/edit')[1];assert.notEqual(first.body.operation_id,second.body.operation_id);assert.equal(second.body.item_id,'replacement-item');
 release(editSuccess(first.body));await old;assert.equal(f.text('item-mutation-receipt'),'');assert.equal(f.elements.get('item-edit-retry').disabled,false);
 await f.fire('item-edit-check');assert.equal(f.calls.at(-1).wire,second.wire);assert.match(f.text('item-mutation-receipt'),/replacement-item/);
});
test('a binding change during the request prevents adopting its later success',async()=>{
 let release;const f=fixture((url,body)=>url==='/items/read'?{status:200,data:editableRead()}:url==='/items/edit'?new Promise(resolve=>{release=()=>resolve(editSuccess(body));}):undefined);
 await editForm(f);const work=f.fire('item-edit-form','submit');await observed(()=>release);f.binding.value={installation_id:'replacement',actor_id:'owner'};release();await work;
 assert.equal(f.text('item-mutation-receipt'),'');assert.equal(f.elements.get('item-edit-retry').disabled,true);assert.match(f.text('item-recovery-status'),/installation|owner|discard/i);
});
test('optional text and estimate Clear are distinct from Keep and required text cannot be cleared',async()=>{
 const f=fixture((url,body)=>url==='/items/read'?{status:200,data:editableRead()}:url==='/items/edit'?editSuccess(body,'edited'):undefined);
 await editForm(f);editField(f,'task-title','clear');f.elements.get('item-edit-estimate-action').value='clear';await f.fire('item-edit-form','submit');
 const body=f.calls.find(call=>call.wire).body;assert.deepEqual(body.fields,[{id:'task-title',action:'clear'}]);assert.deepEqual(body.estimate,{action:'clear'});assert.match(f.text('item-mutation-receipt'),/Not estimated/);
 const required=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:undefined);const configuration=formCurrent();configuration.configuration.value.fields[0].usage='required';
 required.view.useProject(configuration,'p');await required.fire('items-refresh');await required.row('item-rows');await required.fire('item-edit-start');editField(required,'task-title','clear');await required.fire('item-edit-form','submit');assert.equal(required.calls.some(call=>call.wire),false);
});
test('wrong returned Set or Clear effects leave recovery unresolved',async()=>{
 for(const action of ['set','clear']){
  const f=fixture((url,body)=>{if(url==='/items/read')return {status:200,data:editableRead()};if(url==='/items/edit'){const reply=editSuccess(body);reply.data.result.value.data.values=[{id:'task-title',value:{kind:'text',value:'wrong saved text'}}];return reply;}});
  await editForm(f);editField(f,'task-title',action,'expected');await f.fire('item-edit-form','submit');assert.equal(f.elements.get('item-edit-retry').disabled,false);assert.equal(f.text('item-mutation-receipt'),'');
 }
});
test('expired saved-result identity disables execution retry but retains reconciliation and current-read choices',async()=>{
 const f=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:['/items/edit','/items/reconcile'].includes(url)?{status:410,data:{error:'replay_expired'}}:undefined);
 await editForm(f);await f.fire('item-edit-form','submit');assert.equal(f.elements.get('item-edit-retry').disabled,true);assert.equal(f.elements.get('item-edit-check').disabled,false);assert.equal(f.elements.get('item-edit-read-current').disabled,false);
 await f.fire('item-edit-retry');assert.equal(f.calls.filter(call=>call.url==='/items/edit').length,1);await f.fire('item-edit-check');assert.equal(f.calls.filter(call=>call.url==='/items/reconcile').length,1);
});
test('encoded edit expansion rejects before retention or transmission and preserves the editable values',async()=>{
 const f=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:undefined);const configuration=formCurrent();
 for(let i=0;i<11;i++)configuration.configuration.value.fields.push({id:'escaped-'+i,owner_type:'configured-task',name:'Escaped '+i,kind:'text',usage:'optional',archived:false});
 f.view.useProject(configuration,'p');await f.fire('items-refresh');await f.row('item-rows');await f.fire('item-edit-start');
 for(let i=0;i<11;i++)editField(f,'escaped-'+i,'set','\u0001'.repeat(16384));await f.fire('item-edit-form','submit');
 assert.equal(f.calls.some(call=>call.wire),false);assert.equal(f.elements.get('item-edit-retry').disabled,true);assert.equal(f.elements.get('item-edit-form').hidden,false);
});

function creationSuccess(entry=pending(),outcome='created') {
 const command=opened({...entry,state:'prepared'}).request.value;
 return {status:outcome==='created'?201:200,data:{outcome,result:wrap('item_create_result',{
  project:{id:entry.project_id,name:'Recorded project',prefix:'PR',estimate_unit:'hours',configuration_revision:command.expected_configuration_revision,next_sequence:'2',ever_estimated:true,archived:false},
  readable_id:{project_id:entry.project_id,item_id:entry.target_item_id,prefix:'PR',sequence:'1'},
  item:{id:entry.target_item_id,project_id:entry.project_id,type_id:command.type_id,workflow_id:'flow',status_id:'new',version:'1',archived:false},
  data:{estimate:command.estimate,values:command.values},phase:'new'})}};
}
async function selectedCreation(f){f.view.useProject(current(),'p');await f.fire('pending-refresh');await f.row('pending-rows');}
test('saved creation executes only explicitly after inspection and accepts the actual mutation envelope',async()=>{
 const f=fixture(url=>url==='/items/pending/execute'?creationSuccess():undefined);
 f.view.useProject(current(),'p');await f.fire('pending-refresh');await f.fire('pending-execute');assert.equal(f.calls.some(call=>call.url.includes('/execute')),false);
 await f.row('pending-rows');assert.equal(f.calls.some(call=>call.url.includes('/execute')),false);await f.fire('pending-execute');
 const call=f.calls.find(call=>call.url==='/items/pending/execute');assert.deepEqual(call.body,{project_id:'p',intent_handle:'a'.repeat(32)});
 assert.match(f.text('item-mutation-receipt'),/Recorded creation|PR-1/i);assert.equal(f.elements.get('pending-read-item').disabled,false);
});
test('creation response loss retains the same locator across absence and permits only an explicit same-identity retry',async()=>{
 let executed=0;const f=fixture(url=>url==='/items/pending/execute'?++executed===1?Promise.reject(new Error('lost create reply')):creationSuccess(pending(),'replayed'):url==='/items/pending/reconcile'?{status:404,data:{error:'operation_not_found'}}:undefined);
 await selectedCreation(f);await f.fire('pending-execute');const original=f.calls.find(call=>call.url==='/items/pending/execute').wire;
 await f.fire('item-edit-check');assert.equal(f.elements.get('item-edit-retry').disabled,false);assert.match(f.text('item-recovery-status'),/in.flight|unconfirmed|lookup/i);
 await f.fire('pending-open-selected');await f.fire('pending-cancel');assert.equal(f.calls.some(call=>call.url==='/items/pending/cancel'),false);
 f.view.clear(false);const other=current('q');f.view.useProject(other,'q');await f.fire('item-edit-retry');
 assert.equal(f.calls.filter(call=>call.url==='/items/pending/execute').at(-1).wire,original);assert.match(f.text('item-mutation-receipt'),/Historical recorded creation/);
 assert.equal(f.text('items-project'),'Project q');assert.equal(f.calls.some(call=>call.url==='/items/pending/register'),false);
});
test('a lookup-only absence starts no creation and does not pretend another attempt cannot commit',async()=>{
 const f=fixture(url=>url==='/items/pending/reconcile'?{status:404,data:{error:'operation_not_found'}}:undefined);
 await selectedCreation(f);await f.fire('pending-reconcile');assert.equal(f.calls.some(call=>call.url==='/items/pending/execute'),false);
 assert.equal(f.elements.get('item-edit-retry').disabled,true);assert.equal(f.elements.get('pending-execute').disabled,false);assert.match(f.text('item-recovery-status'),/in.flight|lookup|another.*attempt/i);
});
test('an unresolved edit cannot be overwritten by selecting and executing a different creation',async()=>{
 const f=fixture(url=>url==='/items/read'?{status:200,data:editableRead()}:url==='/items/edit'?Promise.reject(new Error('unknown edit')):undefined);
 await editForm(f);await f.fire('item-edit-form','submit');const original=f.calls.find(call=>call.url==='/items/edit').wire;
 await f.fire('pending-refresh');await f.row('pending-rows');await f.fire('pending-execute');assert.equal(f.calls.some(call=>call.url==='/items/pending/execute'),false);
 await f.fire('item-edit-retry');assert.equal(f.calls.filter(call=>call.url==='/items/edit').at(-1).wire,original);
});
test('malformed creation results retain the locator instead of adopting another target or historical configuration',async()=>{
 for(const corrupt of [value=>{value.item.id='wrong';},value=>{value.readable_id.item_id='wrong';},value=>{value.project.configuration_revision='2';},value=>{value.data.estimate='1';}]){
  const f=fixture(url=>{if(url==='/items/pending/execute'){const response=creationSuccess();corrupt(response.data.result.value);return response;}});
  await selectedCreation(f);await f.fire('pending-execute');assert.equal(f.text('item-mutation-receipt'),'');assert.equal(f.elements.get('item-edit-retry').disabled,false);
 }
});
test('pruned completed creation can reconcile expiry and retain a separate current-read target without execution',async()=>{
 const entry=pending('completed');const f=fixture(url=>url==='/items/pending/list'?{status:200,data:{entries:[entry],before:null}}:url==='/items/pending/open'?{status:200,data:opened(entry)}:url==='/items/pending/reconcile'?{status:410,data:{error:'replay_expired'}}:undefined);
 await selectedCreation(f);await f.fire('pending-reconcile');assert.equal(f.elements.get('item-edit-retry').disabled,true);assert.equal(f.elements.get('pending-read-item').disabled,false);assert.equal(f.elements.get('item-edit-read-current').disabled,false);
 assert.equal(f.calls.some(call=>call.url==='/items/pending/execute'),false);assert.match(f.text('item-recovery-status'),/recorded|expired|current/i);
});
test('unknown cancellation blocks creation execution until tracking is explicitly abandoned',async()=>{
 const f=fixture(url=>url==='/items/pending/cancel'?Promise.reject(new Error('unknown cancel')):url==='/items/pending/execute'?creationSuccess():undefined);
 await selectedCreation(f);await f.fire('pending-cancel');await f.fire('pending-confirm-cancel');assert.equal(f.elements.get('pending-execute').disabled,true);
 await f.fire('pending-check-cancel');assert.equal(f.elements.get('pending-execute').disabled,true);await f.fire('pending-stop-tracking');assert.equal(f.elements.get('pending-execute').disabled,false);
 assert.match(f.text('item-uncertainty'),/may.*commit|may.*complete|rollback/i);
});

test('an old lookup absence cannot clear the same creation after execution promotion',async()=>{
 let release, checks=0;const f=fixture(url=>url==='/items/pending/reconcile'?++checks===1?new Promise(resolve=>{release=resolve;}):{status:404,data:{error:'operation_not_found'}}:url==='/items/pending/execute'?Promise.reject(new Error('execution reply lost')):undefined);
 await selectedCreation(f);const checking=f.fire('pending-reconcile');await new Promise(setImmediate);const initial=f.calls.find(call=>call.url==='/items/pending/reconcile').wire;
 await f.fire('item-edit-retry');assert.equal(f.calls.filter(call=>call.url==='/items/pending/execute').length,1);
 release({status:404,data:{error:'operation_not_found'}});await checking;
 assert.equal(f.elements.get('item-edit-retry').disabled,false);assert.equal(f.elements.get('pending-cancel').disabled,true);
 await f.fire('item-edit-retry');for(const call of f.calls.filter(call=>call.wire))assert.equal(call.wire,initial);
 assert.equal(f.calls.filter(call=>call.url==='/items/pending/execute').length,2);
});
test('missing locator after attempted creation closes retry without offering a disabled action',async()=>{
 const f=fixture(url=>url==='/items/pending/execute'?Promise.reject(new Error('execution unknown')):url==='/items/pending/reconcile'?{status:404,data:{error:'pending_not_found'}}:undefined);
 await selectedCreation(f);await f.fire('pending-execute');await f.fire('item-edit-check');
 assert.equal(f.elements.get('item-edit-retry').disabled,true);assert.equal(f.elements.get('item-edit-check').disabled,false);
 assert.match(f.text('item-recovery-status'),/Retry is unavailable/);assert.doesNotMatch(f.text('item-recovery-status'),/explicitly retry/);
});
test('a verified completed creation resolves matching cancellation uncertainty without claiming undo',async()=>{
 const f=fixture(url=>url==='/items/pending/cancel'?Promise.reject(new Error('cancel unknown')):url==='/items/pending/reconcile'?creationSuccess(pending(),'replayed'):undefined);
 await selectedCreation(f);await f.fire('pending-cancel');await f.fire('pending-confirm-cancel');await f.fire('pending-reconcile');
 assert.equal(f.elements.get('pending-check-cancel').disabled,true);assert.equal(f.elements.get('pending-read-item').disabled,false);
 assert.match(f.text('item-recovery-status'),/cannot undo|recorded/i);
});
