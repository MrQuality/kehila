"use strict";

// A finite test client, not a browser/process owner. The caller owns the browser.
class Protocol {
  constructor(socket) {
    this.socket = socket;
    this.next = 1;
    this.pending = new Map();
    this.closed = false;
    socket.addEventListener("message", event => {
      try {
        if (typeof event.data !== "string" || event.data.length > 1024 * 1024) {
          throw new Error("Browser protocol response exceeded its bound");
        }
        const message = JSON.parse(event.data);
        if (!Number.isSafeInteger(message.id)) return; // Unsolicited event.
        const request = this.pending.get(message.id);
        if (!request) return;
        // Chrome reports a detached flattened session as a root-level routing
        // error, without sessionId. Reject only that request by its unique id;
        // successful replies still require their exact session identity.
        const missingSession = message.sessionId === undefined && request.sessionId !== undefined
          && message.error?.code === -32001;
        if (message.sessionId !== request.sessionId && !missingSession) throw new Error("Browser response session mismatch");
        this.pending.delete(message.id);
        clearTimeout(request.timer);
        if (message.error) request.reject(new Error(JSON.stringify(message.error)));
        else request.resolve(message.result);
      } catch (error) { this.fail(error); }
    });
    socket.addEventListener("close", () => this.fail(new Error("Browser protocol closed")));
    socket.addEventListener("error", () => this.fail(new Error("Browser protocol failed")));
  }

  fail(error) {
    this.closed = true;
    for (const request of this.pending.values()) {
      clearTimeout(request.timer);
      request.reject(error);
    }
    this.pending.clear();
  }

  call(method, params = {}, sessionId, timeout = 5000) {
    if (this.closed) return Promise.reject(new Error("Browser protocol is closed"));
    return new Promise((resolve, reject) => {
      const id = this.next++;
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error("Browser protocol deadline: " + method));
      }, timeout);
      this.pending.set(id, {resolve, reject, timer, sessionId});
      try { this.socket.send(JSON.stringify({id, method, params, ...(sessionId ? {sessionId} : {})})); }
      catch (error) {
        this.pending.delete(id); clearTimeout(timer); reject(error);
      }
    });
  }

  close() {
    this.fail(new Error("Browser protocol released"));
    this.socket.close();
  }
}

async function connect(url) {
  const socket = new WebSocket(url);
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("Browser protocol connection deadline")), 5000);
      socket.addEventListener("open", () => { clearTimeout(timer); resolve(); }, {once:true});
      socket.addEventListener("error", () => { clearTimeout(timer); reject(new Error("Browser connection failed")); }, {once:true});
    });
    return new Protocol(socket);
  } catch (error) { socket.close(); throw error; }
}

module.exports = {Protocol, connect};
