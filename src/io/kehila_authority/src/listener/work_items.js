"use strict";
// Current reads and explicit item saves; historical replay retains original identity.
globalThis.KehilaWorkItems = ({document, request, requestWire, run, checkBinding,
  randomBytes = count => globalThis.crypto.getRandomValues(new Uint8Array(count)), encode = JSON.stringify}) => {
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
  let currentRead = null, editor = null, editFields = [], retainedSave = null, retryClosed = false;
  let abandonedSave = false, receiptTarget = null, lookupNotice = false;
  let openedFacts = null, openedInspected = false;
  // Retained UTF-8 records: <=1MiB exact wire plus <=2KiB identity/lifecycle
  // metadata. Typed view/form/response temporaries and JS heap are separate.
  const EDIT_WIRE_BYTES = 1048576, EDIT_METADATA_BYTES = 2048;
  const message = value => { el("items-status").textContent = value; };
  function controls(nextBusy = busy, nextActive = active) {
    busy = nextBusy; active = nextActive;
    for (const name of ["items-refresh","pending-refresh"]) el(name).disabled = busy || !active || !project;
    el("items-older").disabled = busy || !active || !project || itemsBefore === null;
    el("pending-older").disabled = busy || !active || !project || pendingBefore === null;
    el("pending-read-item").disabled = busy || !active || !project || !opened || opened.state !== "completed";
    for (const button of [...itemButtons,...pendingButtons]) button.disabled = busy || !active || !project;
    creationControls(); mutationControls();
  }
  function clear(forget = false) {
    if (forget) {
      registrationUnknown = false; cancellationUnknown = null; abandonedCancellation = false;
      if (retainedSave) el("item-recovery-status").textContent = "The save recovery record was discarded. This does not cancel a save or undo work that may already have completed.";
      abandonedSave = abandonedSave || retainedSave !== null; retainedSave = null; retryClosed = false;
      receiptTarget = null; el("item-mutation-receipt").replaceChildren();
    }
    openedFacts = null; openedInspected = false; lookupNotice = false;
    currentRead = null; editor = null; editFields = [];
    el("item-edit-form").hidden = true; el("item-edit-fields").replaceChildren();
    el("item-edit-estimate").value = ""; el("item-edit-estimate-action").value = "keep";
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
    catch (error) {
      const text = typeof error.message === "string" ? error.message.slice(0,240) : "WorkItem operation unavailable.";
      message(text); if (retainedSave || abandonedSave) el("item-recovery-status").textContent = text;
    }
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
    if (pending) { pendingBefore = null; pendingButtons = []; opened = null; openedFacts = null; openedInspected = false; hideConfirmation(); el("pending-detail").replaceChildren(); }
    else { itemsBefore = null; itemButtons = []; currentRead = null; editor = null; editFields = []; el("item-edit-form").hidden = true; el("item-detail").replaceChildren(); }
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
    currentRead = null; editor = null; editFields = []; el("item-edit-form").hidden = true; controls();
    el("item-detail").replaceChildren();
    const data = await send(context,"/items/read",{project_id:context.id,item_id:item});
    if (!data) return;
    require(data.outcome === "read" && ["item_create_result","item_edit_result"].includes(data.result?.kind));
    const result = envelope(data.result,data.result.kind), current = result.item;
    require(record(current) && current.id === item && current.project_id === context.id && uint(current.version) && id(current.type_id) && id(current.workflow_id) && id(current.status_id) && typeof current.archived === "boolean" && result.project?.id === context.id && ["hours","points"].includes(result.project.estimate_unit) && record(result.data) && ["new","active","done"].includes(result.phase));
    const name = readable(data.readable_id,context,item);
    const values = fields(result.data.values,context.configuration);
    details("item-detail",[["Item",name],["Version",current.version],["Type",current.type_id],["Workflow",current.workflow_id],["Status",current.status_id],["Phase",result.phase],["Estimate",estimate(result.data.estimate,result.project.estimate_unit)],...values]);
    currentRead = {context,result}; controls();
    message("Current item read under fresh authority.");
  }
  async function inspect(context,selected) {
    opened = selected; openedFacts = null; openedInspected = false; hideConfirmation(); el("pending-detail").replaceChildren(); controls();
    const data = await send(context,"/items/pending/open",{project_id:context.id,intent_handle:selected.intent_handle});
    if (!data) return;
    require(keys(data,["entry","request"])); const entry = pendingSummary(data.entry,context);
    for (const key of ["project_id","request_id","intent_handle","token","target_item_id","registered_ms","expires_ms"]) require(entry[key] === selected[key]);
    let facts = null, pairs = [["Saved request",entry.request_id],["State",entry.state],["Item target",entry.target_item_id]];
    if (data.request !== null) {
      require(["prepared","completed"].includes(entry.state));
      const command = envelope(data.request,"item_create_request");
      require(keys(command,["project_id","operation_id","item_id","type_id","workflow_id","expected_configuration_revision","estimate","values"]) && command.project_id === context.id && command.operation_id === entry.token && command.item_id === entry.target_item_id && uint(command.expected_configuration_revision) && id(command.type_id) && (command.workflow_id === null || id(command.workflow_id)));
      facts = Object.freeze({type:command.type_id,workflow:command.workflow_id,configuration:command.expected_configuration_revision,estimate:command.estimate});
      pairs = [...pairs,["Type",command.type_id],["Workflow",command.workflow_id === null ? "Configured default at execution" : command.workflow_id],["Configuration revision",command.expected_configuration_revision],["Estimate",estimate(command.estimate,context.unit)],...fields(command.values)];
    } else require(entry.state !== "prepared");
    details("pending-detail",pairs); opened = entry; openedFacts = facts; openedInspected = true; observeTerminal(entry); controls();
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
    const executionUnknown = retainedSave?.family === "item_create" && retainedSave.attempted;
    const sameCreation = retainedSave?.family === "item_create" && sameEntry(retainedSave.entry,opened);
    el("pending-execute").disabled = unavailable || !openedInspected || !opened || opened.state !== "prepared" || cancellationUnknown !== null || (retainedSave !== null && (!sameCreation || retryClosed));
    el("pending-reconcile").disabled = unavailable || !opened || (retainedSave !== null && !sameCreation);
    const canCancel = !unavailable && !executionUnknown && opened && opened.state === "prepared" && (!cancellationUnknown || sameEntry(cancellationUnknown,opened));
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
  function mutationControls() {
    const unavailable = busy || !active;
    el("item-edit-start").disabled = unavailable || !project || !currentRead || retainedSave !== null || project.archived || currentRead.result.item.archived;
    el("item-edit-save").disabled = unavailable || !editor || retainedSave !== null;
    el("item-edit-estimate-action").disabled = unavailable || !editor;
    el("item-edit-estimate").disabled = unavailable || !editor || el("item-edit-estimate-action").value !== "set";
    for (const field of editFields) { field.action.disabled = unavailable || !editor; field.input.disabled = unavailable || !editor || field.action.value !== "set"; }
    el("item-edit-check").disabled = unavailable || !retainedSave;
    el("item-edit-retry").disabled = unavailable || !retainedSave || retryClosed;
    el("item-edit-forget").disabled = unavailable || !retainedSave;
    const target = retainedSave || receiptTarget;
    el("item-edit-read-current").disabled = unavailable || !target || !project || project.id !== target.project;
    const scope = retainedSave || receiptTarget;
    el("item-save-scope").textContent = scope ? (scope.family === "item_create" ? "Creation" : "Edit") + " in project " + scope.project + "; item " + scope.item + "." : "";
    el("item-recovery").hidden = !retainedSave && !abandonedSave && !receiptTarget && !lookupNotice;
  }
  function textValue(value, required = false) {
    if (typeof value !== "string" || value.includes("\0")) throw new Error("Text must be valid UTF-8 without NUL.");
    for (const symbol of value) { const code = symbol.charCodeAt(0); if (symbol.length === 1 && code >= 0xd800 && code <= 0xdfff) throw new Error("Text must be valid UTF-8."); }
    if (required && /^\p{White_Space}*$/u.test(value)) throw new Error("Required text must contain a non-whitespace value.");
    if (new TextEncoder().encode(value).length > 16384) throw new Error("Text exceeds the field byte limit.");
    return value;
  }
  const sameOwner = (a,b) => a.installation_id === b.installation_id && a.actor_id === b.actor_id;
  async function saveBinding(retained = null) {
    const binding = await checkBinding();
    if (retained && retainedSave !== retained) return null;
    require(keys(binding,["installation_id","actor_id"]) && id(binding.installation_id) && id(binding.actor_id));
    if (retained && !sameOwner(binding,retained.binding)) {
      clear(true); el("item-recovery-status").textContent = "The installation or owner changed. The old save record was discarded; this does not undo a save that may have completed.";
      throw new Error("The installation or owner changed. No old save was resent.");
    }
    return Object.freeze({...binding});
  }
  function nextVersion(value) {
    require(uint(value)); const next = (BigInt(value) + 1n).toString();
    if (!uint(next)) throw new Error("Item version is exhausted. No edit can be prepared.");
    return next;
  }
  async function beginEdit() {
    const selected = currentRead, context = project;
    if (!selected || selected.context !== context || retainedSave || context.archived || selected.result.item.archived) throw new Error("Read the current item before preparing a separate edit.");
    await saveBinding(); if (selected !== currentRead || context !== project) return;
    const result = selected.result, item = result.item;
    require(result.project.configuration_revision === context.configuration.revision && uint(context.configuration.revision)); nextVersion(item.version);
    const kind = context.configuration.types.find(value => value.id === item.type_id); require(kind);
    const values = new Map(result.data.values.map(entry => [entry.id,entry.value]));
    editFields = context.configuration.fields.filter(field => field.owner_type === item.type_id && !field.archived && field.usage !== "hidden" && field.kind === "text").map(definition => {
      require(id(definition.id) && typeof definition.name === "string" && ["required","optional"].includes(definition.usage));
      const existing = values.get(definition.id); require(!existing || existing.kind === "text");
      const group = document.createElement("div"), label = document.createElement("label"), action = document.createElement("select"), input = document.createElement("textarea"), name = document.createElement("span");
      action.name = definition.id + "_action"; action.replaceChildren(option("keep","Keep"),option("set","Set"),option("clear","Clear",definition.usage === "required")); action.value = "keep";
      input.name = definition.id; input.value = existing ? existing.value : ""; name.textContent = definition.name;
      label.append(name,action,input); group.append(label); action.addEventListener("change",() => controls());
      return {definition,action,input,group,original:existing ? existing.value : null,rendered:input.value};
    });
    editor = {context,item:{...item},configuration:context.configuration.revision,phase:result.phase,estimate:result.data.estimate};
    el("item-edit-estimate-action").value = "keep"; el("item-edit-estimate").value = "";
    el("item-edit-fields").replaceChildren(...editFields.map(field => field.group)); el("item-edit-form").hidden = false;
    message("Prepare explicit text and estimate changes. Keep preserves the existing exact value."); controls();
  }
  function retainedEdit(selected,binding) {
    if (!selected || selected !== editor || selected.context !== project || el("item-edit-form").hidden || retainedSave) throw new Error("An unresolved save cannot be overwritten. Check it or explicitly stop tracking it.");
    const fields = []; let bytes = 0;
    for (const field of editFields) {
      const action = field.action.value;
      if (action === "keep") continue;
      if (action === "clear") { if (field.definition.usage === "required") throw new Error("Required text cannot be cleared."); fields.push({id:field.definition.id,action}); }
      else if (action === "set") {
        // Browsers normalize textarea line endings. An unchanged rendered value
        // retains the original stored text rather than silently rewriting CRLF.
        const value = textValue(field.original !== null && field.input.value === field.rendered ? field.original : field.input.value,field.definition.usage === "required");
        bytes += new TextEncoder().encode(value).length; fields.push({id:field.definition.id,action,value});
      } else invalid();
    }
    if (fields.length > 128 || bytes > 1048576) throw new Error("Edit exceeds the field or payload limit.");
    const action = el("item-edit-estimate-action").value;
    let estimate;
    if (action === "keep" || action === "clear") estimate = {action};
    else if (action === "set") { const value = parseEstimate(el("item-edit-estimate").value); if (value === null) throw new Error("Set requires an exact estimate, including zero."); estimate = {action,value}; }
    else invalid();
    const raw = Array.from(randomBytes(16)); require(raw.length === 16 && raw.every(value => Number.isInteger(value) && value >= 0 && value <= 255));
    const operation_id = raw.map(value => value.toString(16).padStart(2,"0")).join("");
    const body = {project_id:selected.context.id,item_id:selected.item.id,operation_id,expected_item_version:selected.item.version,expected_configuration_revision:selected.configuration,estimate,fields};
    const wire = encode(body); require(typeof wire === "string");
    if (new TextEncoder().encode(wire).length > EDIT_WIRE_BYTES) throw new Error("Encoded edit exceeds the request limit. No save was sent.");
    require(JSON.stringify(JSON.parse(wire)) === JSON.stringify(body));
    const metadata = {binding,project:body.project_id,item:body.item_id,family:"item_edit",version:body.expected_item_version,configuration:body.expected_configuration_revision,
      lifecycle:{type:selected.item.type_id,workflow:selected.item.workflow_id,status:selected.item.status_id,archived:selected.item.archived,phase:selected.phase},estimate:selected.estimate};
    if (new TextEncoder().encode(JSON.stringify(metadata)).length > EDIT_METADATA_BYTES) throw new Error("Edit identity exceeds the retention limit. No save was sent.");
    Object.freeze(metadata.lifecycle); return Object.freeze({...metadata,wire});
  }
  function savedResult(data,retained,family) {
    require(retained.family === family && ["item_create","item_edit"].includes(family));
    const create = family === "item_create";
    require(keys(data,["outcome","result"]) && [create ? "created" : "edited","replayed"].includes(data.outcome));
    const value = envelope(data.result,family + "_result");
    require(keys(value,create ? ["project","item","data","phase","readable_id"] : ["project","item","data","phase"]));
    const item = value.item;
    require(keys(item,["id","project_id","type_id","workflow_id","status_id","version","archived"]) && item.id === retained.item && item.project_id === retained.project && id(item.type_id) && id(item.workflow_id) && id(item.status_id) && uint(item.version) && typeof item.archived === "boolean" && ["new","active","done"].includes(value.phase));
    require(keys(value.project,["id","name","prefix","estimate_unit","configuration_revision","archived","next_sequence","ever_estimated"]) && value.project.id === retained.project && uint(value.project.configuration_revision) && ["hours","points"].includes(value.project.estimate_unit));
    require(typeof value.project.name === "string" && id(value.project.prefix) && typeof value.project.archived === "boolean" && uint(value.project.next_sequence) && typeof value.project.ever_estimated === "boolean");
    require(keys(value.data,["estimate","values"])); estimate(value.data.estimate,value.project.estimate_unit); fields(value.data.values);
    return value;
  }
  function editResult(data,retained) {
    const value = savedResult(data,retained,"item_edit"), item = value.item, state = retained.lifecycle, original = JSON.parse(retained.wire);
    require(item.version === nextVersion(retained.version) && value.project.configuration_revision === retained.configuration);
    require(item.type_id === state.type && item.workflow_id === state.workflow && item.status_id === state.status && item.archived === state.archived && value.phase === state.phase);
    const expected = original.estimate.action === "set" ? original.estimate.value : original.estimate.action === "clear" ? null : retained.estimate;
    require(value.data.estimate === expected);
    for (const change of original.fields) {
      const value = valueFor(change.id);
      if (change.action === "set") require(value && value.kind === "text" && value.value === change.value);
      else if (change.action === "clear") require(value === undefined);
      else require(change.action === "keep");
    }
    function valueFor(id) { return value.data.values.find(entry => entry.id === id)?.value; }
    return value;
  }
  async function operateEdit(kind,retained = retainedSave) {
    if (!retained || retained.family !== "item_edit" || retained !== retainedSave || kind === "retry" && retryClosed) return;
    if (!await saveBinding(retained)) return;
    const reply = await requestWire(kind === "check" ? "/items/reconcile" : "/items/edit",retained.wire);
    if (retained !== retainedSave || !await saveBinding(retained)) return;
    if (reply.status !== 200) {
      if (reply.status === 410 && reply.data?.error === "replay_expired") retryClosed = true;
      el("item-recovery-status").textContent = retryClosed ? "The saved result expired. Read the current item separately; this identity will never become a new save." : "Save remains unconfirmed. A lookup or rejected attempt does not prove an earlier in-flight save cannot commit. Check or retry this exact request.";
      controls(); return;
    }
    const result = editResult(reply.data,retained);
    details("item-mutation-receipt",[["Result",reply.data.outcome === "replayed" ? "Historical recorded edit" : "Recorded edit"],["Project",retained.project],["Item",retained.item],["Version",result.item.version],["Estimate",estimate(result.data.estimate,result.project.estimate_unit)],...fields(result.data.values)]);
    receiptTarget = {project:retained.project,item:retained.item,family:retained.family}; retainedSave = null; retryClosed = false; lookupNotice = false;
    if (currentRead?.context.id === retained.project && currentRead.result.item.id === retained.item) {
      currentRead = null; editor = null; editFields = []; el("item-edit-form").hidden = true;
    }
    el("item-recovery-status").textContent = "Recorded save confirmed. This result is separate from current item data; read current state before another edit."; controls();
  }
  async function saveEdit() {
    const selected = editor; const binding = await saveBinding(); if (selected !== editor) return;
    const retained = retainedEdit(selected,binding);
    retainedSave = retained; retryClosed = false; editor = null; editFields = []; el("item-edit-form").hidden = true;
    el("item-mutation-receipt").replaceChildren(); receiptTarget = null;
    el("item-recovery-status").textContent = "Save acknowledgement pending. The exact request is retained; no automatic repeat will be sent."; controls();
    await operateEdit("retry",retained);
  }
  function creationRecord(entry,binding) {
    pendingSummary(entry,{id:entry.project_id});
    const wire = JSON.stringify({project_id:entry.project_id,intent_handle:entry.intent_handle});
    const metadata = {family:"item_create",binding,project:entry.project_id,item:entry.target_item_id,entry:Object.freeze({...entry}),facts:openedFacts,attempted:false};
    if (new TextEncoder().encode(wire).length > 2048 || new TextEncoder().encode(JSON.stringify(metadata)).length > 2048) throw new Error("Creation identity exceeds its retention limit. No execution was sent.");
    return Object.freeze({...metadata,wire});
  }
  function creationResult(data,retained) {
    const value = savedResult(data,retained,"item_create");
    require(value.item.version === "1" && value.item.archived === false);
    readable(value.readable_id,{id:retained.project},retained.item);
    require(value.readable_id.prefix === value.project.prefix && value.project.next_sequence === nextVersion(value.readable_id.sequence));
    const facts = retained.facts;
    if (facts) require(value.item.type_id === facts.type && value.project.configuration_revision === facts.configuration && (facts.workflow === null || value.item.workflow_id === facts.workflow) && value.data.estimate === facts.estimate);
    // The client retains bounded identity/facts, not the full server-owned
    // command. Exact field agreement is checked by the correlated backend path.
    return value;
  }
  async function operateCreation(kind,retained = retainedSave) {
    if (!retained || retained.family !== "item_create" || retained !== retainedSave || kind === "retry" && retryClosed) return;
    if (kind === "retry" && cancellationUnknown) throw new Error("Check the unconfirmed cancellation before executing a saved creation.");
    if (!await saveBinding(retained)) return;
    if (kind === "retry" && cancellationUnknown) throw new Error("Check the unconfirmed cancellation before executing a saved creation.");
    if (kind === "retry" && !retained.attempted) {
      retained = Object.freeze({...retained,attempted:true}); retainedSave = retained;
    }
    hideConfirmation(); controls();
    const reply = await requestWire(kind === "check" ? "/items/pending/reconcile" : "/items/pending/execute",retained.wire);
    if (retained !== retainedSave || !await saveBinding(retained)) return;
    if (![200,201].includes(reply.status)) {
      const error = reply.data?.error;
      if (reply.status === 410 || reply.status === 404 && error === "pending_not_found") retryClosed = true;
      if (reply.status === 410 && error === "replay_expired") {
        receiptTarget = {project:retained.project,item:retained.item,family:retained.family};
        if (sameEntry(opened,retained.entry)) showEntry({...retained.entry,state:"completed"});
        el("item-recovery-status").textContent = "Creation is recorded, but its saved result expired. Read the current item separately; this identity will never become a new creation.";
      } else {
        if (sameEntry(opened,retained.entry) && reply.status === 410 && ["pending_expired","pending_cancelled"].includes(error)) showEntry({...retained.entry,state:error === "pending_expired" ? "expired" : "cancelled"});
        if (sameEntry(opened,retained.entry) && error === "pending_not_found") { opened = null; openedFacts = null; openedInspected = false; el("pending-detail").replaceChildren(); }
        el("item-recovery-status").textContent = retryClosed
          ? "Retry is unavailable for this expired identity or missing locator. An earlier in-flight execution may still complete; check the recorded outcome. No replacement request was registered."
          : "Creation was not confirmed at this lookup. Another session or an earlier in-flight attempt may still complete. No new identity was registered; check or explicitly retry the same saved request.";
      }
      // Only a lookup record for which this page never began execution can be
      // released by absence. An earlier attempted execution remains unknown.
      if (!retained.attempted && reply.status === 404 && ["operation_not_found","pending_not_found"].includes(error)) { retainedSave = null; retryClosed = false; lookupNotice = true; }
      controls(); return;
    }
    require(reply.status === (reply.data?.outcome === "created" ? 201 : 200));
    const value = creationResult(reply.data,retained);
    details("item-mutation-receipt",[["Result",reply.data.outcome === "replayed" ? "Historical recorded creation" : "Recorded creation"],["Project",retained.project],["Item",readable(value.readable_id,{id:retained.project},retained.item)],["Version",value.item.version],["Estimate",estimate(value.data.estimate,value.project.estimate_unit)],...fields(value.data.values)]);
    receiptTarget = {project:retained.project,item:retained.item,family:retained.family}; retainedSave = null; retryClosed = false; lookupNotice = false;
    if (sameEntry(opened,retained.entry)) showEntry({...retained.entry,state:"completed"});
    el("item-recovery-status").textContent = "Recorded creation confirmed. Its historical result is separate from current item data; cancellation cannot undo it."; controls();
  }
  async function selectedCreation(kind) {
    const selected = opened, context = project;
    if (!selected || !context) return;
    if (kind === "retry" && (!openedInspected || selected.state !== "prepared" || cancellationUnknown)) return;
    const binding = await saveBinding(); if (selected !== opened || context !== project) return;
    if (retainedSave && (retainedSave.family !== "item_create" || !sameEntry(retainedSave.entry,selected))) throw new Error("Check or explicitly stop tracking the retained save before tracking another request.");
    if (!retainedSave) { retainedSave = creationRecord(selected,binding); retryClosed = false; lookupNotice = false; receiptTarget = null; el("item-mutation-receipt").replaceChildren(); }
    el("item-recovery-status").textContent = kind === "check" ? "Checking recorded creation only. This check never executes the request." : "Creation acknowledgement pending. An interrupted response does not cancel the owned work.";
    controls(); await operateCreation(kind,retainedSave);
  }
  async function operateRetained(kind) {
    const retained = retainedSave; if (!retained) return;
    if (retained.family === "item_edit") return operateEdit(kind,retained);
    if (retained.family === "item_create") return operateCreation(kind,retained);
    invalid();
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
    opened = entry; openedFacts = null; openedInspected = false; hideConfirmation(); observeTerminal(entry);
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
    if (retainedSave?.family === "item_create" && retainedSave.attempted) throw new Error("Check the unconfirmed creation before cancelling its request.");
    if (cancellationUnknown && !sameEntry(cancellationUnknown,selected.entry)) throw new Error("Check the earlier unconfirmed cancellation before cancelling another request.");
    if (!await checkedContext(selected.context)) return;
    if (confirmation !== selected || !sameEntry(selected.entry,opened) || retainedSave?.family === "item_create" && retainedSave.attempted) return;
    cancellationUnknown = Object.freeze({...selected.entry}); hideConfirmation(); controls();
    const reply = await transmit(selected.context,"/items/pending/cancel",{project_id:selected.context.id,intent_handle:selected.entry.intent_handle});
    if (!reply) return;
    require(reply.status === 200 && keys(reply.data,["entry"]));
    const entry = pendingSummary(reply.data.entry,selected.context);
    require(sameEntry(entry,selected.entry) && entry.state !== "prepared"); showEntry(entry);
    message(entry.state === "completed" ? "Creation is already recorded. Cancellation did not undo it; read its current item separately." : "Saved request is " + entry.state + ". No replacement was registered.");
  }
  el("item-edit-start").addEventListener("click",() => action(beginEdit));
  el("item-edit-form").addEventListener("submit",event => { event.preventDefault(); action(saveEdit); });
  el("item-edit-estimate-action").addEventListener("change",() => controls());
  el("item-edit-check").addEventListener("click",() => action(() => operateRetained("check")));
  el("item-edit-retry").addEventListener("click",() => action(() => operateRetained("retry")));
  el("item-edit-read-current").addEventListener("click",() => action(() => { const target = retainedSave || receiptTarget; return target && project?.id === target.project && read(project,target.item); }));
  el("item-edit-forget").addEventListener("click",() => action(async () => {
    const retained = retainedSave; if (!retained || !await saveBinding(retained)) return;
    retainedSave = null; retryClosed = false; abandonedSave = true;
    el("item-recovery-status").textContent = "Stopped tracking this save. It may already have completed or may still commit; this does not cancel it or prove rollback."; controls();
  }));
  el("item-new-request").addEventListener("click",() => action(beginCreation));
  el("item-kind").addEventListener("change",() => action(async () => renderType()));
  el("item-create-form").addEventListener("submit",event => { event.preventDefault(); action(registerCreation); });
  el("item-dismiss-save").addEventListener("click",() => action(async () => { if (!await checkedContext(project)) return; registrationUnknown = false; controls(); message("Earlier save warning dismissed at your request. No request was proved rejected and no identities were merged."); }));
  el("pending-execute").addEventListener("click",() => action(() => selectedCreation("retry")));
  el("pending-reconcile").addEventListener("click",() => action(() => selectedCreation("check")));
  el("pending-open-selected").addEventListener("click",() => action(() => opened && inspect(project,opened)));
  el("pending-cancel").addEventListener("click",() => {
    if (!project || !opened || opened.state !== "prepared" || retainedSave?.family === "item_create" && retainedSave.attempted || cancellationUnknown && !sameEntry(cancellationUnknown,opened)) return;
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
