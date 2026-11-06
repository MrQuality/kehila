"use strict";
// Current reads, saved creation intent and cancellation. Execution and replay remain separate.
globalThis.KehilaWorkItems = ({document, request, run, checkBinding}) => {
  const el = id => document.getElementById(id);
  const invalid = () => { throw new Error("Invalid or mismatched WorkItem response."); };
  const require = value => { if (!value) invalid(); };
  const record = value => value !== null && typeof value === "object" && !Array.isArray(value);
  const keys = (value, names) => record(value) && Object.keys(value).length === names.length && names.every(name => Object.hasOwn(value, name));
  const id = value => typeof value === "string" && value.length > 0 && !value.includes("\0") && new TextEncoder().encode(value).length <= 128;
  const uint = (value, positive = true, max = "18446744073709551615") => typeof value === "string" && /^(0|[1-9][0-9]*)$/.test(value) && (!positive || value !== "0") && (value.length < max.length || value.length === max.length && value <= max);
  const less = (a,b) => a.length < b.length || a.length === b.length && a < b;
  const envelope = (value, kind) => { require(keys(value,["codec_version","kind","value"]) && value.codec_version === 1 && value.kind === kind && record(value.value)); return value.value; };
  function estimate(value, unit) {
    if (value === null) return "Not estimated";
    require(uint(value,false,"999999999"));
    const padded = value.padStart(4,"0"), fraction = padded.slice(-3).replace(/0+$/,"");
    return padded.slice(0,-3) + (fraction ? "." + fraction : "") + " " + unit;
  }
  let project = null, itemsBefore = null, pendingBefore = null, opened = null;
  let busy = false, active = true, itemButtons = [], pendingButtons = [];
  let draft = null, formFields = [], confirmation = null;
  let registrationUnknown = false, cancellationUnknown = null, abandonedCancellation = false;
  const message = value => { el("items-status").textContent = value; };
  function controls(nextBusy = busy, nextActive = active) {
    busy = nextBusy; active = nextActive;
    for (const name of ["items-refresh","pending-refresh"]) el(name).disabled = busy || !active || !project;
    el("items-older").disabled = busy || !active || !project || itemsBefore === null;
    el("pending-older").disabled = busy || !active || !project || pendingBefore === null;
    el("pending-read-item").disabled = busy || !active || !project || !opened || opened.state !== "completed";
    for (const button of [...itemButtons,...pendingButtons]) button.disabled = busy || !active || !project;
    creationControls();
  }
  function clear(forget = false) {
    if (forget) { registrationUnknown = false; cancellationUnknown = null; abandonedCancellation = false; }
    draft = null; formFields = []; confirmation = null;
    el("item-create-fields").replaceChildren(); el("item-create-form").hidden = true;
    el("item-kind").replaceChildren(); el("item-kind").value = "";
    el("item-workflow").replaceChildren(); el("item-workflow").value = ""; el("item-estimate").value = "";
    el("pending-cancel-confirmation").hidden = true;
    project = null; itemsBefore = null; pendingBefore = null; opened = null;
    itemButtons = []; pendingButtons = [];
    for (const name of ["item-rows","pending-rows","item-detail","pending-detail"]) el(name).replaceChildren();
    el("items-project").textContent = ""; el("work-items").hidden = true; controls();
  }
  function useProject(reply, expected) {
    clear();
    require(reply.outcome === "read");
    const current = envelope(reply.project,"project_snapshot");
    const configuration = envelope(reply.configuration,"configuration_snapshot");
    require(id(expected) && current.id === expected && typeof current.name === "string" && ["hours","points"].includes(current.estimate_unit));
    require(configuration.project_id === expected && uint(current.configuration_revision) && configuration.revision === current.configuration_revision && Array.isArray(configuration.fields));
    project = {id:expected,name:current.name,unit:current.estimate_unit,archived:current.archived === true || configuration.project_archived === true,configuration};
    el("items-project").textContent = current.name;
    el("work-items").hidden = false;
    message(project.archived ? "This project is archived; creation is unavailable. Current and saved items can still be inspected." : "Refresh current items or prepare a saved creation request. Saving does not create an item."); controls();
  }
  const action = work => run(async () => {
    try { await work(); }
    catch (error) { message(typeof error.message === "string" ? error.message.slice(0,240) : "WorkItem inspection unavailable."); }
  });
  async function checkedContext(context) {
    if (!context || context !== project) return false;
    try { await checkBinding(); }
    catch (error) { if (context !== project) return false; clear(); throw error; }
    return context === project;
  }
  async function transmit(context,path,body) {
    let reply;
    try { reply = await request(path,body); }
    catch (error) { if (context !== project) return null; throw error; }
    return context === project ? reply : null;
  }
  async function send(context,path,body) {
    if (!await checkedContext(context)) return null;
    const reply = await transmit(context,path,body);
    if (!reply) return null;
    if (reply.status !== 200) throw new Error("WorkItem inspection unavailable. Refresh or sign in again; no creation was attempted.");
    return reply.data;
  }
  function readable(value, context, item) {
    require(keys(value,["project_id","item_id","prefix","sequence"]) && value.project_id === context.id && value.item_id === item && id(value.prefix) && uint(value.sequence));
    return value.prefix + "-" + value.sequence;
  }
  function itemSummary(value,context) {
    require(keys(value,["item_id","version","type_id","workflow_id","status_id","archived","estimate","name","readable_id"]) && id(value.item_id) && uint(value.version) && id(value.type_id) && id(value.workflow_id) && id(value.status_id) && typeof value.archived === "boolean" && typeof value.name === "string");
    estimate(value.estimate,context.unit); readable(value.readable_id,context,value.item_id);
    return value;
  }
  function pendingSummary(value,context) {
    require(keys(value,["project_id","request_id","intent_handle","token","target_item_id","registered_ms","expires_ms","state"]) && value.project_id === context.id && uint(value.request_id,true,"9223372036854775807") && typeof value.intent_handle === "string" && /^[0-9a-f]{32}$/.test(value.intent_handle) && typeof value.token === "string" && /^[\x21-\x7e]{1,128}$/.test(value.token) && id(value.target_item_id) && uint(value.registered_ms,false) && uint(value.expires_ms) && less(value.registered_ms,value.expires_ms) && ["prepared","completed","cancelled","expired"].includes(value.state));
    return value;
  }
  function row(label,work,buttons) {
    const li = document.createElement("li"), button = document.createElement("button");
    button.type = "button"; button.textContent = label; button.addEventListener("click",() => action(work));
    buttons.push(button); li.append(button); return li;
  }
  async function list(kind,cursor = null) {
    const context = project, pending = kind === "pending";
    if (pending) { pendingBefore = null; pendingButtons = []; opened = null; hideConfirmation(); el("pending-detail").replaceChildren(); }
    else { itemsBefore = null; itemButtons = []; el("item-detail").replaceChildren(); }
    el(pending ? "pending-rows" : "item-rows").replaceChildren(); controls();
    const data = await send(context,pending ? "/items/pending/list" : "/items/list",{project_id:context?.id,before:cursor});
    if (!data) return;
    require(keys(data,pending ? ["entries","before"] : ["outcome","entries","before"]) && (pending || data.outcome === "listed") && Array.isArray(data.entries) && data.entries.length <= 16);
    let previous = cursor;
    const seen = new Set();
    for (const entry of data.entries) {
      (pending ? pendingSummary : itemSummary)(entry,context);
      const position = pending ? entry.request_id : entry.readable_id.sequence;
      const identity = pending ? entry.intent_handle : entry.item_id;
      require((previous === null || less(position,previous)) && !seen.has(identity));
      seen.add(identity); previous = position;
    }
    require(data.before === null || data.entries.length > 0 && data.before === previous);
    const buttons = [], rows = data.entries.map(entry => pending
      ? row("Request " + entry.request_id + " / " + entry.state,() => inspect(context,entry),buttons)
      : row(readable(entry.readable_id,context,entry.item_id) + " / " + entry.name + " / " + estimate(entry.estimate,context.unit),() => read(context,entry.item_id),buttons));
    if (pending) { pendingBefore = data.before; pendingButtons = buttons; }
    else { itemsBefore = data.before; itemButtons = buttons; }
    el(pending ? "pending-rows" : "item-rows").replaceChildren(...rows);
    message(data.entries.length ? "Select an entry to read it. No creation was attempted." : "No entries on this page."); controls();
  }
  function details(target, pairs) {
    const nodes = pairs.map(([label,value,preserve = false]) => {
      const term = document.createElement("dt"), description = document.createElement("dd");
      term.textContent = label;
      if (preserve) { const text = document.createElement("pre"); text.textContent = value; description.append(text); }
      else description.textContent = value;
      const group = document.createElement("div"); group.append(term,description); return group;
    });
    el(target).replaceChildren(...nodes);
  }
  function fields(values, configuration = null) {
    require(Array.isArray(values) && values.length <= 1024);
    const seen = new Set();
    return values.map(entry => {
      require(keys(entry,["id","value"]) && id(entry.id) && !seen.has(entry.id) && keys(entry.value,["kind","value"])); seen.add(entry.id);
      const {kind,value} = entry.value;
      require(["text","number","boolean","date","single_choice"].includes(kind) && (kind === "boolean" ? typeof value === "boolean" : typeof value === "string"));
      const label = configuration?.fields.find(field => field.id === entry.id)?.name;
      return [typeof label === "string" ? label : entry.id,String(value),true];
    });
  }
  async function read(context,item) {
    el("item-detail").replaceChildren();
    const data = await send(context,"/items/read",{project_id:context.id,item_id:item});
    if (!data) return;
    require(data.outcome === "read" && ["item_create_result","item_edit_result"].includes(data.result?.kind));
    const result = envelope(data.result,data.result.kind), current = result.item;
    require(record(current) && current.id === item && current.project_id === context.id && uint(current.version) && id(current.type_id) && id(current.workflow_id) && id(current.status_id) && typeof current.archived === "boolean" && result.project?.id === context.id && ["hours","points"].includes(result.project.estimate_unit) && record(result.data) && ["new","active","done"].includes(result.phase));
    const name = readable(data.readable_id,context,item);
    const values = fields(result.data.values,context.configuration);
    details("item-detail",[["Item",name],["Version",current.version],["Type",current.type_id],["Workflow",current.workflow_id],["Status",current.status_id],["Phase",result.phase],["Estimate",estimate(result.data.estimate,result.project.estimate_unit)],...values]);
    message("Current item read under fresh authority.");
  }
  async function inspect(context,selected) {
    opened = selected; hideConfirmation(); el("pending-detail").replaceChildren(); controls();
    const data = await send(context,"/items/pending/open",{project_id:context.id,intent_handle:selected.intent_handle});
    if (!data) return;
    require(keys(data,["entry","request"])); const entry = pendingSummary(data.entry,context);
    for (const key of ["project_id","request_id","intent_handle","token","target_item_id","registered_ms","expires_ms"]) require(entry[key] === selected[key]);
    let pairs = [["Saved request",entry.request_id],["State",entry.state],["Item target",entry.target_item_id]];
    if (data.request !== null) {
      require(["prepared","completed"].includes(entry.state));
      const command = envelope(data.request,"item_create_request");
      require(keys(command,["project_id","operation_id","item_id","type_id","workflow_id","expected_configuration_revision","estimate","values"]) && command.project_id === context.id && command.operation_id === entry.token && command.item_id === entry.target_item_id && uint(command.expected_configuration_revision) && id(command.type_id) && (command.workflow_id === null || id(command.workflow_id)));
      pairs = [...pairs,["Type",command.type_id],["Workflow",command.workflow_id === null ? "Configured default at execution" : command.workflow_id],["Configuration revision",command.expected_configuration_revision],["Estimate",estimate(command.estimate,context.unit)],...fields(command.values)];
    } else require(entry.state !== "prepared");
    details("pending-detail",pairs); opened = entry; observeTerminal(entry); controls();
    message(entry.state === "completed" ? "Creation is recorded. Read its current item separately; a retained command is not a saved-result replay." : "Saved request inspected. No WorkItem was created by this action.");
  }
  const sameEntry = (left,right) => left && right && ["project_id","request_id","intent_handle","token","target_item_id","registered_ms","expires_ms"].every(key => left[key] === right[key]);
  function hideConfirmation() { confirmation = null; el("pending-cancel-confirmation").hidden = true; }
  function notices() {
    const lines = [];
    if (registrationUnknown) lines.push("At least one earlier save is unconfirmed. Discover saved requests in the projects where you saved; equal forms do not identify the same intent. A new save is separate.");
    if (cancellationUnknown) lines.push("Cancellation of request " + cancellationUnknown.request_id + " in project " + cancellationUnknown.project_id + " is unconfirmed. Check that same locator; a prepared snapshot may still change.");
    if (abandonedCancellation) lines.push("An earlier cancellation remains unconfirmed after you stopped tracking it. It may already have committed or may still complete; stopping tracking proves no rollback or outcome.");
    el("item-uncertainty").textContent = lines.join(" ");
    el("item-dismiss-save").hidden = !registrationUnknown;
  }
  function creationControls() {
    const unavailable = busy || !active || !project;
    const availableTypes = project && Array.isArray(project.configuration.types) && project.configuration.types.some(kind => !kind.archived);
    el("item-new-request").disabled = unavailable || project.archived || !availableTypes;
    el("item-dismiss-save").disabled = unavailable || !registrationUnknown;
    el("item-save-request").disabled = unavailable || !draft || draft.unsupported;
    for (const name of ["item-kind","item-workflow","item-estimate"]) el(name).disabled = unavailable || !draft;
    for (const field of formFields) {
      field.present.disabled = unavailable || !draft || field.definition.usage === "required";
      field.input.disabled = unavailable || !draft || !field.present.checked;
    }
    el("pending-open-selected").disabled = unavailable || !opened;
    const canCancel = !unavailable && opened && opened.state === "prepared" && (!cancellationUnknown || sameEntry(cancellationUnknown,opened));
    el("pending-cancel").disabled = !canCancel;
    el("pending-confirm-cancel").disabled = !canCancel || !confirmation || confirmation.context !== project || !sameEntry(confirmation.entry,opened);
    el("pending-keep-request").disabled = unavailable || !confirmation;
    el("pending-check-cancel").disabled = unavailable || !cancellationUnknown || cancellationUnknown.project_id !== project.id;
    el("pending-stop-tracking").disabled = unavailable || !cancellationUnknown;
    notices();
  }
  function option(value,label,disabled = false) {
    const node = document.createElement("option"); node.value = value; node.textContent = label; node.disabled = disabled; return node;
  }
  function eligibleWorkflow(context,kind,workflow) {
    if (!workflow || workflow.archived || !kind.permitted_workflows.includes(workflow.id)) return false;
    const initial = context.configuration.statuses.find(status => status.id === workflow.initial_status_id);
    return initial && !initial.archived && initial.phase === "new";
  }
  function renderType() {
    if (!draft || draft.context !== project) throw new Error("Creation form unavailable. Start a separate request under the current project.");
    const configuration = project.configuration, kind = configuration.types.find(value => value.id === el("item-kind").value && !value.archived);
    if (!kind || !id(kind.id) || !Array.isArray(kind.permitted_workflows)) throw new Error("Selected type is unavailable.");
    const definitions = configuration.fields.filter(field => field.owner_type === kind.id && !field.archived && field.usage !== "hidden");
    draft.kind = kind.id;
    draft.unsupported = definitions.some(field => field.usage === "required" && field.kind !== "text");
    const workflows = configuration.workflows.filter(workflow => eligibleWorkflow(project,kind,workflow));
    const defaultAvailable = workflows.some(workflow => workflow.id === kind.default_workflow_id);
    el("item-workflow").replaceChildren(option("","Configured default (" + kind.default_workflow_id + ")",!defaultAvailable),...workflows.map(workflow => option(workflow.id,workflow.id)));
    el("item-workflow").value = "";
    formFields = definitions.filter(field => field.kind === "text").map(definition => {
      require(id(definition.id) && typeof definition.name === "string" && ["optional","required"].includes(definition.usage));
      const group = document.createElement("div"), includeLabel = document.createElement("label"), valueLabel = document.createElement("label"), present = document.createElement("input"), input = document.createElement("textarea"), name = document.createElement("span");
      present.type = "checkbox"; present.name = definition.id + "_included"; present.checked = definition.usage === "required";
      includeLabel.append(present); const inclusion = document.createElement("span"); inclusion.textContent = definition.usage === "required" ? "Required" : "Include this field"; includeLabel.append(inclusion);
      name.textContent = definition.name; input.name = definition.id; input.value = ""; input.required = definition.usage === "required"; valueLabel.append(name,input); group.append(includeLabel,valueLabel);
      present.addEventListener("change",() => controls());
      return {definition,present,input,group};
    });
    el("item-create-fields").replaceChildren(...formFields.map(field => field.group));
    message(draft.unsupported ? "A required field is not supported by this text creation form. Choose another type or wait for a supported form." : "Enter configured text and an optional estimate. Saving records intent; execution later validates the required configuration."); controls();
  }
  async function beginCreation() {
    const context = project;
    if (!await checkedContext(context)) return;
    if (context.archived) throw new Error("Creation is unavailable in an archived project.");
    const configuration = context.configuration;
    require(Array.isArray(configuration.types) && configuration.types.length <= 1024 && Array.isArray(configuration.workflows) && Array.isArray(configuration.statuses));
    const kinds = configuration.types.filter(kind => !kind.archived);
    if (!kinds.length) throw new Error("No configured type is available for creation.");
    for (const kind of kinds) require(id(kind.id));
    draft = {context,unsupported:false}; formFields = [];
    el("item-kind").replaceChildren(...kinds.map(kind => option(kind.id,kind.id))); el("item-kind").value = kinds[0].id; el("item-estimate").value = "";
    el("item-create-form").hidden = false; renderType();
  }
  function parseEstimate(text) {
    if (text === "") return null;
    if (typeof text !== "string" || !/^(0|[1-9][0-9]{0,5})(?:\.[0-9]{1,3})?$/.test(text)) throw new Error("Estimate must be an exact decimal from 0 to 999999.999 with at most three decimal places.");
    const [whole,fraction = ""] = text.split(".");
    return (whole + fraction.padEnd(3,"0")).replace(/^0+(?=[0-9])/,"");
  }
  function creationForm(context) {
    if (!draft || draft.context !== context || el("item-create-form").hidden || context.archived) throw new Error("Creation form unavailable. Start a separate request; an earlier unconfirmed save is not retried here.");
    if (draft.unsupported) throw new Error("A required nontext field is unsupported by this form.");
    const configuration = context.configuration, kind = configuration.types.find(value => value.id === el("item-kind").value && !value.archived);
    if (!kind || !id(kind.id) || draft.kind !== kind.id) throw new Error("The selected type changed. Refresh its configured fields before saving.");
    const workflow = el("item-workflow").value;
    const selectedWorkflow = configuration.workflows.find(value => value.id === (workflow || kind.default_workflow_id));
    if (!eligibleWorkflow(context,kind,selectedWorkflow)) throw new Error("Choose an available configured workflow.");
    const values = []; let textBytes = 0;
    for (const field of formFields) {
      if (!field.present.checked) continue;
      const text = field.input.value;
      if (typeof text !== "string" || text.includes("\0")) throw new Error("Text must be valid UTF-8 without NUL.");
      for (const symbol of text) { const code = symbol.charCodeAt(0); if (symbol.length === 1 && code >= 0xd800 && code <= 0xdfff) throw new Error("Text must be valid UTF-8."); }
      if (field.definition.usage === "required" && /^\p{White_Space}*$/u.test(text)) throw new Error("Required text must contain a non-whitespace value.");
      const bytes = new TextEncoder().encode(text).byteLength; textBytes += bytes;
      if (bytes > 16384 || textBytes > 1048576 || values.length === 128) throw new Error("Text exceeds the field or total payload byte limit.");
      values.push({id:field.definition.id,value:text});
    }
    const body = {project_id:context.id,type_id:kind.id,workflow_id:workflow || null,expected_configuration_revision:configuration.revision,estimate:parseEstimate(el("item-estimate").value),values};
    if (new TextEncoder().encode(JSON.stringify(body)).byteLength > 1048576) throw new Error("Encoded request exceeds the body size limit. No save was sent.");
    return body;
  }
  function showEntry(entry) {
    opened = entry; hideConfirmation();
    details("pending-detail",[["Saved request",entry.request_id],["State",entry.state],["Item target",entry.target_item_id]]); controls();
  }
  async function registerCreation() {
    const context = project, body = creationForm(context);
    if (!await checkedContext(context)) return;
    const previousUnknown = registrationUnknown;
    registrationUnknown = true; el("item-create-form").hidden = true; controls();
    message("Save acknowledgement pending. Discovery is recovery; no automatic repeat will be sent.");
    const reply = await transmit(context,"/items/pending/register",body);
    if (!reply) return;
    if ([400,429].includes(reply.status)) {
      registrationUnknown = previousUnknown; el("item-create-form").hidden = false; controls();
      message(reply.status === 400 ? "The request was rejected. Correct the exact editable values before saving again." : "Save was rejected by capacity or quota. Resolve saved requests or wait before trying again."); return;
    }
    require(reply.status === 201 && keys(reply.data,["entry"]));
    const entry = pendingSummary(reply.data.entry,context); require(entry.state === "prepared");
    registrationUnknown = previousUnknown; draft = null; formFields = []; el("item-create-fields").replaceChildren(); showEntry(entry);
    message("Creation request saved. Inspect it separately; no WorkItem has been created by saving.");
  }
  function observeTerminal(entry) {
    if (cancellationUnknown && sameEntry(cancellationUnknown,entry) && entry.state !== "prepared") cancellationUnknown = null;
  }
  async function confirmCancellation() {
    const selected = confirmation;
    if (!selected || selected.context !== project || !sameEntry(selected.entry,opened) || opened.state !== "prepared") return;
    if (cancellationUnknown && !sameEntry(cancellationUnknown,selected.entry)) throw new Error("Check the earlier unconfirmed cancellation before cancelling another request.");
    if (!await checkedContext(selected.context)) return;
    cancellationUnknown = Object.freeze({...selected.entry}); hideConfirmation(); controls();
    const reply = await transmit(selected.context,"/items/pending/cancel",{project_id:selected.context.id,intent_handle:selected.entry.intent_handle});
    if (!reply) return;
    require(reply.status === 200 && keys(reply.data,["entry"]));
    const entry = pendingSummary(reply.data.entry,selected.context);
    require(sameEntry(entry,selected.entry) && entry.state !== "prepared"); observeTerminal(entry); showEntry(entry);
    message(entry.state === "completed" ? "Creation is already recorded. Cancellation did not undo it; read its current item separately." : "Saved request is " + entry.state + ". No replacement was registered.");
  }
  el("item-new-request").addEventListener("click",() => action(beginCreation));
  el("item-kind").addEventListener("change",() => action(async () => renderType()));
  el("item-create-form").addEventListener("submit",event => { event.preventDefault(); action(registerCreation); });
  el("item-dismiss-save").addEventListener("click",() => action(async () => { if (!await checkedContext(project)) return; registrationUnknown = false; controls(); message("Earlier save warning dismissed at your request. No request was proved rejected and no identities were merged."); }));
  el("pending-open-selected").addEventListener("click",() => action(() => opened && inspect(project,opened)));
  el("pending-cancel").addEventListener("click",() => {
    if (!project || !opened || opened.state !== "prepared" || cancellationUnknown && !sameEntry(cancellationUnknown,opened)) return;
    confirmation = {context:project,entry:Object.freeze({...opened})}; el("pending-cancel-confirmation").hidden = false; controls();
  });
  el("pending-keep-request").addEventListener("click",() => { hideConfirmation(); controls(); });
  el("pending-confirm-cancel").addEventListener("click",() => action(confirmCancellation));
  el("pending-stop-tracking").addEventListener("click",() => action(async () => {
    if (!await checkedContext(project) || !cancellationUnknown) return;
    cancellationUnknown = null; abandonedCancellation = true; hideConfirmation(); controls();
    message("Stopped tracking the earlier cancellation at your request. This does not prove a confirmed outcome or rollback; it may already have committed or may still complete.");
  }));
  el("pending-check-cancel").addEventListener("click",() => action(() => cancellationUnknown && cancellationUnknown.project_id === project?.id && inspect(project,cancellationUnknown)));
  el("items-refresh").addEventListener("click",() => action(() => list("items")));
  el("items-older").addEventListener("click",() => action(() => list("items",itemsBefore)));
  el("pending-refresh").addEventListener("click",() => action(() => list("pending")));
  el("pending-older").addEventListener("click",() => action(() => list("pending",pendingBefore)));
  el("pending-read-item").addEventListener("click",() => action(() => opened?.state === "completed" && read(project,opened.target_item_id)));
  clear(true);
  return {useProject,clear,controls};
};
