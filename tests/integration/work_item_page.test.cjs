// Deterministic controller checks; not browser or native qualification.
const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = path.resolve(__dirname, '../../src/io/kehila_authority/src/listener/work_items.js');
const wrap = (kind,value) => ({codec_version:1,kind,value});
const current = (id='p') => ({outcome:'read',project:wrap('project_snapshot',{id,name:'Project '+id,estimate_unit:'hours',configuration_revision:'1'}),configuration:wrap('configuration_snapshot',{project_id:id,revision:'1',fields:[],types:[],statuses:[]})});
const summary = (sequence='9007199254740993', project='p') => ({item_id:'item-'+sequence,version:'9007199254740995',type_id:'task',workflow_id:'flow',status_id:'new',archived:false,estimate:'1',name:'  <script>exact</script>  ',readable_id:{project_id:project,item_id:'item-'+sequence,prefix:'PR',sequence}});
const pending = (state='prepared',project='p') => ({project_id:project,request_id:'9007199254740997',intent_handle:'a'.repeat(32),token:'b'.repeat(32),target_item_id:'target',registered_ms:'0',expires_ms:'86400000',state});
const opened = (entry=pending()) => ({entry,request:entry.state==='completed'?null:wrap('item_create_request',{project_id:entry.project_id,operation_id:entry.token,item_id:entry.target_item_id,type_id:'task',workflow_id:null,expected_configuration_revision:'1',estimate:'0',values:[{id:'title',value:{kind:'text',value:'  frozen title  '}}]})});
const itemRead = (row=summary()) => ({outcome:'read',readable_id:row.readable_id,result:wrap('item_edit_result',{project:{id:row.readable_id.project_id,estimate_unit:'hours'},item:{id:row.item_id,project_id:row.readable_id.project_id,type_id:'task',workflow_id:'flow',status_id:'new',version:row.version,archived:false},data:{estimate:row.estimate,values:[{id:'title',value:{kind:'text',value:'  exact text  '}}]},phase:'new'})});
class Element {
 constructor(tag='div'){this.tag=tag;this.hidden=false;this.disabled=false;this.textContent='';this.children=[];this.events={};}
 addEventListener(type,fn){this.events[type]=fn;}
 append(...children){this.children.push(...children);}
 replaceChildren(...children){this.children=children;}
}
function fixture(handler=()=>undefined){
 const elements=new Map(); const calls=[]; const jobs=[]; const binding={failure:false,checks:0};
 const document={getElementById(id){if(!elements.has(id))elements.set(id,new Element());return elements.get(id);},createElement(tag){return new Element(tag);}};
 const ports={document,async checkBinding(){binding.checks++;if(binding.failure)throw new Error('Owner binding unavailable');},run(work){const job=work();jobs.push(job);return job;},async request(url,body){calls.push({url,body});return await handler(url,body) || {status:200,data:url==='/items/list'?{outcome:'listed',entries:[summary()],before:null}:url==='/items/read'?itemRead():url==='/items/pending/list'?{entries:[pending()],before:null}:opened()};}};
 const context={TextEncoder};vm.createContext(context);vm.runInContext(fs.readFileSync(source,'utf8'),context);
 const view=context.KehilaWorkItems(ports);
 const fire=async(id,type='click')=>{const el=document.getElementById(id);if(!el.disabled){assert.equal(typeof el.events[type],'function','enabled handler '+id);el.events[type]({preventDefault(){}});}await Promise.all(jobs.splice(0));};
 const row=async(id,index=0)=>{document.getElementById(id).children[index].children[0].events.click();await Promise.all(jobs.splice(0));};
 const text=id=>{const collect=el=>[el.textContent,...el.children.map(collect)].join('\n');return collect(document.getElementById(id));};
 return {view,ports,calls,elements,fire,row,text,binding};
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
