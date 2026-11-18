"use strict";
// Selection policy has a separate database and recovery identity from edits.
globalThis.KehilaSelectionRecoveryStore = options => globalThis.KehilaRecoveryStore({...options,
  databaseName:"kehila-selection-recovery",
  unavailableMessage:"Current-work recovery storage could not be confirmed. No replacement request was sent."});
globalThis.KehilaSelectionRecovery = ({store = globalThis.KehilaSelectionRecoveryStore(), now = Date.now,
  crypto = globalThis.crypto} = {}) => globalThis.KehilaRecoveryJournal({store,now,crypto,
  wireLimit:4096,metadataLimit:2048,
  validateRetained(value, {require,keys,text,id,uint,binding}) {
    require(keys(value,["binding","family","wire"]) && value.family === "selection");
    binding(value.binding); text(value.wire,4096);
    let body;
    try { body = JSON.parse(value.wire); } catch (_) { require(false); }
    require(keys(body,["operation_token","expected_version","requested"])
      && typeof body.operation_token === "string" && /^[0-9a-f]{32}$/.test(body.operation_token)
      && uint(body.expected_version,true));
    if (body.requested !== null) {
      require(keys(body.requested,["project_id","item_id"])
        && id(body.requested.project_id) && id(body.requested.item_id));
    }
    // This journal retains only the interface's JSON.stringify form. Requiring
    // exact reserialization also rejects duplicate/escaped-equivalent keys;
    // the server independently parses and admits the original request bytes.
    require(JSON.stringify(body) === value.wire);
  }
});
