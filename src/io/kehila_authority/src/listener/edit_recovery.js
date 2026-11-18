"use strict";
// Edit policy remains separate from the shared durable journal mechanism.
globalThis.KehilaEditRecoveryStore = options => globalThis.KehilaRecoveryStore({...options,
  databaseName:"kehila-edit-recovery",
  unavailableMessage:"Edit recovery storage could not be confirmed. Reload before saving; no replacement was sent."});
globalThis.KehilaEditRecovery = ({store = globalThis.KehilaEditRecoveryStore(), now = Date.now,
  crypto = globalThis.crypto} = {}) => globalThis.KehilaRecoveryJournal({store,now,crypto,
  wireLimit:1048576,metadataLimit:2048,
  validateRetained(value, {require,keys,object,text,id,uint,binding}) {
    const wireLimit = 1048576;
    const amount = value => value === null || uint(value, true, "999999999");
    require(keys(value, ["binding","project","item","family","version","configuration","lifecycle","estimate","wire"]));
    binding(value.binding);
    require(value.family === "item_edit" && id(value.project) && id(value.item) && uint(value.version) && uint(value.configuration) && amount(value.estimate));
    const state = value.lifecycle;
    require(keys(state, ["type","workflow","status","archived","phase"]) && id(state.type) && id(state.workflow) && id(state.status)
      && typeof state.archived === "boolean" && ["new","active","done"].includes(state.phase));
    text(value.wire, wireLimit);
    let body;
    try { body = JSON.parse(value.wire); } catch (_) { require(false); }
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
});
