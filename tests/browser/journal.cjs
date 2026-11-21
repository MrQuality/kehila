"use strict";
// Connect only to a caller-owned, disposable browser/profile. No process/trust
// lifecycle is hidden here. The caller must stop it and verify profile cleanup.
const assert = require("node:assert/strict");
const path = require("node:path");
const {pathToFileURL} = require("node:url");
const {randomUUID} = require("node:crypto");
const {connect} = require("./protocol.cjs");

async function run(endpoint) {
  const base = new URL(endpoint);
  assert.equal(base.protocol, "http:");
  assert.equal(base.username + base.password, "");
  assert.ok(["127.0.0.1", "[::1]"].includes(base.hostname), "Explicit loopback browser endpoint required");
  const version = await fetch(new URL("/json/version", base), {signal:AbortSignal.timeout(5000)});
  assert.equal(version.status, 200);
  const description = await version.json();
  const websocket = new URL(description.webSocketDebuggerUrl);
  assert.equal(websocket.protocol, "ws:");
  assert.equal(websocket.host, base.host);
  const client = await connect(websocket.href), targets = [], sessions = [];
  const fixture = pathToFileURL(path.join(__dirname, "journal.html")).href;
  let primary;
  async function evaluate(sessionId, expression, timeout = 5000) {
    const reply = await client.call("Runtime.evaluate", {expression, awaitPromise:true, returnByValue:true}, sessionId, timeout);
    if (reply.exceptionDetails) throw new Error(JSON.stringify(reply.exceptionDetails));
    return reply.result.value;
  }
  try {
    for (let i = 0; i < 2; ++i) {
      const {targetId} = await client.call("Target.createTarget", {url:fixture});
      targets.push(targetId);
      const {sessionId} = await client.call("Target.attachToTarget", {targetId, flatten:true});
      sessions.push(sessionId);
      const deadline = Date.now() + 5000;
      while (!await evaluate(sessionId, "typeof fixture === 'object'")) {
        if (Date.now() >= deadline) throw new Error("Fixture load deadline");
        await new Promise(resolve => setTimeout(resolve, 25));
      }
    }
    const name = "kehila-component-" + randomUUID();
    for (const sessionId of sessions) {
      const capability = await evaluate(sessionId, `fixture.initialize(${JSON.stringify(name)})`);
      assert.equal(capability.indexedDB, true);
      assert.equal(capability.secureContext, true);
    }
    await evaluate(sessions[0], "fixture.holdNext()");
    const a = evaluate(sessions[0], "fixture.compare(null, 'a')");
    // Attach rejection handlers immediately, including if a later witness fails.
    let settledA = false, settledB = false;
    const observedA = a.then(value => {settledA = true; return {value};}, error => {settledA = true; return {error};});
    async function status(id, successfulReads = 0) {
      const deadline = Date.now() + 1500;
      while (true) {
        const current = await evaluate(id, "fixture.status()");
        if (current.transactions.length && current.transactions.at(-1).reads >= successfulReads) return current;
        if (Date.now() >= deadline) throw new Error("Transaction creation witness deadline");
        await new Promise(resolve => setTimeout(resolve, 10));
      }
    }
    const first = await status(sessions[0], 1);
    assert.equal(first.transactions.at(-1).ended, false);
    const b = evaluate(sessions[1], "fixture.compare(null, 'b')");
    const observedB = b.then(value => {settledB = true; return {value};}, error => {settledB = true; return {error};});
    const queued = await status(sessions[1]);
    const active = await evaluate(sessions[0], "fixture.status()");
    assert.deepEqual(active.held, {released:false, expired:false});
    assert.equal(active.transactions.at(-1).ended, false);
    assert.equal(active.transactions.at(-1).reads, 1);
    assert.equal(queued.transactions.at(-1).mode, "readwrite");
    assert.equal(queued.transactions.at(-1).durability, "strict");
    assert.equal(queued.transactions.at(-1).reads, 0);
    assert.equal(queued.transactions.at(-1).ended, false);
    assert.equal(settledA, false, "The first compare must await actual transaction completion");
    assert.equal(settledB, false);
    assert.deepEqual(await evaluate(sessions[0], "fixture.release()"), {released:false, expired:false},
      "The controller, not the safety deadline, must release the transaction");
    const completed = await Promise.all([observedA,observedB]);
    for (const result of completed) if (result.error) throw result.error;
    const outcomes = completed.map(result => result.value);
    assert.deepEqual(outcomes, [true,false], "The held first transaction commits before the queued competitor");
    const winner = "a";
    for (const id of sessions) assert.equal(await evaluate(id, "fixture.read()"), winner);
    const aborted = await evaluate(sessions[0], `fixture.abort(${JSON.stringify(winner)})`);
    assert.deepEqual(aborted, {observed:"QuotaExceededError", events:["abort"], value:winner});
    assert.equal(await evaluate(sessions[1], "fixture.read()"), winner);

    const wire = await evaluate(sessions[0], `fixture.retain(${JSON.stringify("1".repeat(32))})`);
    assert.deepEqual(await evaluate(sessions[0], "fixture.admit()"), {wire});
    assert.equal(await evaluate(sessions[1], "fixture.restore()"), wire);
    await evaluate(sessions[1], "fixture.clear()");
    assert.match((await evaluate(sessions[0], "fixture.admit()")).rejected, /changed|cleared|stale/i);
    assert.equal(await evaluate(sessions[1], "fixture.freshRestore()"), null);
  } catch (error) { primary = error; }
  finally {
    if (sessions.length) {
      try { await evaluate(sessions[0], "fixture.cleanup()", 7000); }
      catch (error) { primary = primary ? new AggregateError([primary,error], "Test and database release failed") : error; }
    }
    for (const targetId of targets) {
      try { assert.equal((await client.call("Target.closeTarget", {targetId})).success, true); }
      catch (error) { primary = primary ? new AggregateError([primary,error], "Test and target release failed") : error; }
    }
    client.close();
  }
  if (primary) throw primary;
  console.log(JSON.stringify({browser:description.Browser, scope:"file-origin IndexedDB component",
    crossTabCAS:true, injectedQuotaAbort:true, crossTabGenerationInvalidation:true,
    queuedWriteWitness:true, actualAbortEvent:true, validAdmitControl:true,
    databaseRelease:true, targetRelease:true}));
}

if (require.main === module) {
  if (process.argv.length !== 3) throw new Error("Usage: node tests/browser/journal.cjs http://127.0.0.1:<owned-port>");
  run(process.argv[2]).catch(error => {console.error(error); process.exitCode = 1;});
}
module.exports = {run};
