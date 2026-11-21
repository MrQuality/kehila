"use strict";
const {test} = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

function fixture() {
  const timers = new Map(), requests = [], native = [];
  let nextTimer = 0;
  function createTransaction(name, mode) {
    let ended = false;
    const transaction = new EventTarget();
    transaction.mode = mode;
    transaction.durability = "strict";
    const terminate = kind => {ended = true; transaction.dispatchEvent(new Event(kind));};
    transaction.objectStore = () => ({get() {
      assert.equal(ended, false, "keepalive must not access a terminated transaction");
      const request = new EventTarget();
      request.emitError = () => {
        const event = new Event("error", {cancelable:true});
        if (request.onerror?.(event) === false) event.preventDefault();
        request.dispatchEvent(event);
        if (!event.defaultPrevented) terminate("abort");
        return event.defaultPrevented;
      };
      requests.push(request);
      return request;
    }});
    native.push({transaction, terminate});
    return transaction;
  }
  const context = vm.createContext({
    indexedDB:{open:() => ({result:{transaction:createTransaction}})},
    setTimeout:(callback, delay) => {const id = ++nextTimer; timers.set(id, {callback, delay}); return id;},
    clearTimeout:id => timers.delete(id)
  });
  vm.runInContext(fs.readFileSync(path.join(__dirname, "instrument.js"), "utf8"), context);
  const observer = context.instrumentStorage();
  const database = observer.factory.open("synthetic").result;
  observer.holdNext();
  database.transaction("journal", "readwrite");
  const expire = () => {
    assert.equal(timers.size, 1);
    const [id, timer] = [...timers][0];
    assert.equal(timer.delay, 2000);
    timers.delete(id);
    timer.callback();
  };
  return {observer, database, timers, requests,
    terminate:(kind, index = 0) => native[index].terminate(kind), expire};
}

for (const kind of ["abort", "complete"]) {
  test("a held transaction releases its timer on " + kind, async () => {
    const {observer, timers, requests, terminate} = fixture();
    assert.equal(timers.size, 1);
    terminate(kind);
    await observer.transactions[0].finished;
    assert.equal(timers.size, 0);
    assert.equal(observer.status().held.released, true);
    assert.equal(observer.status().held.expired, false);
    assert.deepEqual(Array.from(observer.transactions[0].trace.events), [kind]);
    assert.equal(observer.transactions[0].trace.ended, true);
    requests[0].onsuccess();
    assert.equal(requests.length, 1);
  });
}

test("request failure is not suppressed and still terminates the transaction", async () => {
  const {observer, timers, requests} = fixture();
  assert.equal(requests[0].emitError(), false);
  await observer.transactions[0].finished;
  assert.deepEqual(Array.from(observer.transactions[0].trace.events), ["abort"]);
  assert.equal(timers.size, 0);
  assert.equal(observer.status().held.released, true);
});

test("another transaction's completion cannot release the held transaction", () => {
  const {observer, database, timers, terminate} = fixture();
  database.transaction("journal", "readonly");
  terminate("complete", 1);
  assert.equal(timers.size, 1);
  assert.equal(observer.status().held.released, false);
  assert.equal(observer.transactions[0].trace.ended, false);
  terminate("complete", 0);
  assert.equal(timers.size, 0);
  assert.equal(observer.status().held.released, true);
});

test("explicit release and a genuine safety expiry remain distinguishable", () => {
  const released = fixture();
  const before = released.observer.release();
  assert.equal(before.released, false);
  assert.equal(before.expired, false);
  assert.equal(released.timers.size, 0);
  released.terminate("complete");
  assert.equal(released.observer.status().held.expired, false);

  const expired = fixture();
  expired.expire();
  expired.terminate("complete");
  assert.equal(expired.timers.size, 0);
  assert.equal(expired.observer.status().held.released, true);
  assert.equal(expired.observer.status().held.expired, true);
});
