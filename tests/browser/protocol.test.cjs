"use strict";
const {test} = require("node:test");
const assert = require("node:assert/strict");
const {Protocol} = require("./protocol.cjs");

class Socket extends EventTarget {
  sent = [];
  send(text) { this.sent.push(JSON.parse(text)); }
  close() { this.dispatchEvent(new Event("close")); }
  response(message) { this.dispatchEvent(new MessageEvent("message", {data:JSON.stringify(message)})); }
}

test("responses are matched by identity across sessions and out-of-order delivery", async () => {
  const socket = new Socket(), client = new Protocol(socket);
  const a = client.call("Runtime.evaluate", {expression:"1"}, "a");
  const b = client.call("Runtime.evaluate", {expression:"2"}, "b");
  assert.equal(socket.sent[0].sessionId, "a");
  socket.response({method:"unsolicited"});
  socket.response({id:socket.sent[1].id, sessionId:"b", result:{value:2}});
  socket.response({id:socket.sent[0].id, sessionId:"a", result:{value:1}});
  assert.deepEqual(await Promise.all([a,b]), [{value:1},{value:2}]);
  assert.equal(client.pending.size, 0);
  client.close();
});

test("a response from another session cannot settle a request", async () => {
  const socket = new Socket(), client = new Protocol(socket);
  const pending = assert.rejects(client.call("Runtime.evaluate", {}, "expected"), /session mismatch/);
  socket.response({id:socket.sent[0].id, sessionId:"foreign", result:{}});
  await pending;
  client.close();
});

test("close rejects all outstanding work and refuses later calls", async () => {
  const socket = new Socket(), client = new Protocol(socket);
  const a = assert.rejects(client.call("first"), /closed/);
  const b = assert.rejects(client.call("second"), /closed/);
  socket.close();
  await Promise.all([a,b]);
  assert.equal(client.pending.size, 0);
  await assert.rejects(client.call("late"), /closed/);
});

test("deadline and malformed response fail rather than hanging", async () => {
  const socket = new Socket(), client = new Protocol(socket);
  await assert.rejects(client.call("never", {}, undefined, 5), /deadline/);
  assert.equal(client.pending.size, 0);
  const pending = assert.rejects(client.call("bad-response"), /JSON/);
  socket.dispatchEvent(new MessageEvent("message", {data:"invalid JSON"}));
  await pending;
  client.close();
});
