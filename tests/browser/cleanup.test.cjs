"use strict";
const {test} = require("node:test");
const assert = require("node:assert/strict");
const {releaseDatabases} = require("./cleanup.cjs");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

test("cleanup uses controller identities and stops after successful release", async () => {
  const calls = [], names = ["kehila-component-a-raw", "kehila-component-a-policy"];
  const result = await releaseDatabases(async (...args) => {calls.push(args);}, ["a", "b"], names);
  assert.deepEqual(calls, [["a", `fixture.cleanup(${JSON.stringify(names)})`, 7000]]);
  assert.deepEqual(result, {recoveredFailures:0});
});

test("a failed first context falls back without relying on second initialization", async () => {
  const calls = [], names = ["kehila-component-a-raw", "kehila-component-a-policy"];
  const result = await releaseDatabases(async (session, expression) => {
    calls.push(session);
    if (session === "dead") throw new Error("context lost");
    assert.equal(expression, `fixture.cleanup(${JSON.stringify(names)})`);
  }, ["dead", "uninitialized"], names);
  assert.deepEqual(calls, ["dead", "uninitialized"]);
  assert.deepEqual(result, {recoveredFailures:1});
});

test("exhausted contexts preserve every cleanup failure", async () => {
  const errors = [new Error("first"), new Error("second")];
  await assert.rejects(releaseDatabases(async session => {throw errors[session];}, [0, 1], ["db"]),
    error => error instanceof AggregateError && error.errors[0] === errors[0] && error.errors[1] === errors[1]);
  await assert.rejects(releaseDatabases(async () => {}, [], ["db"]), /every known context/);
  assert.deepEqual(await releaseDatabases(async () => {assert.fail();}, [], []), {recoveredFailures:0});
});

test("the actual uninitialized fixture deletes both supplied databases and reports failures", async () => {
  const html = fs.readFileSync(path.join(__dirname, "journal.html"), "utf8");
  const program = html.match(/<script>\s*([\s\S]*?)<\/script>/)[1];
  const names = ["kehila-component-a-raw", "kehila-component-a-policy"];
  for (const failure of [false, true]) {
    const deleted = [];
    const context = vm.createContext({setTimeout, clearTimeout, indexedDB:{deleteDatabase(name) {
      deleted.push(name);
      const request = {};
      queueMicrotask(() => {
        if (failure && name.endsWith("-raw")) request.onerror();
        else request.onsuccess();
      });
      return request;
    }}});
    vm.runInContext(program, context);
    if (failure) await assert.rejects(context.fixture.cleanup(names), /Database release failed/);
    else await context.fixture.cleanup(names);
    assert.deepEqual(deleted, names);
    await assert.rejects(context.fixture.cleanup(["personal", "personal"]), /Invalid cleanup/);
    assert.deepEqual(deleted, names);
  }
});
