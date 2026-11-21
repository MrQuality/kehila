"use strict";

// Observe native transactions. The optional bounded keepalive creates actual
// write-lock contention; it never replaces IndexedDB state or transaction events.
globalThis.instrumentStorage = () => {
  const transactions = [];
  let nextHold = false, held = null;
  const bound = (target, key) => {
    const value = Reflect.get(target, key, target);
    return typeof value === "function" ? value.bind(target) : value;
  };
  function transaction(database, args) {
    const native = database.transaction(...args);
    let finish;
    const trace = {mode:native.mode, durability:native.durability, reads:0, events:[], ended:false};
    const finished = new Promise(resolve => {finish = resolve;});
    transactions.push({trace, finished});
    for (const kind of ["abort","complete"]) native.addEventListener(kind, () => {
      trace.events.push(kind); trace.ended = true; finish();
    }, {once:true});
    if (nextHold && native.mode === "readwrite") {
      nextHold = false;
      held = {released:false, expired:false};
      const current = held;
      current.timer = setTimeout(() => {current.expired = true; current.released = true;}, 2000);
      function keepalive() {
        if (current.released) return;
        const request = native.objectStore("journal").get("current");
        request.onsuccess = keepalive;
      }
      keepalive();
    }
    return new Proxy(native, {
      get(target,key) {
        if (key !== "objectStore") return bound(target,key);
        return name => {
          const store = target.objectStore(name);
          return new Proxy(store, {get(object, property) {
            if (property !== "get") return bound(object,property);
            return (...values) => {
              const request = object.get(...values);
              request.addEventListener("success", () => {trace.reads++;}, {once:true});
              return request;
            };
          }});
        };
      },
      set:(target,key,value) => Reflect.set(target,key,value,target)
    });
  }
  const factory = {open(...args) {
    const request = indexedDB.open(...args);
    let database;
    return new Proxy(request, {
      get(target,key) {
        if (key !== "result") return bound(target,key);
        if (!database) database = new Proxy(target.result, {
          get:(object,property) => property === "transaction"
            ? (...values) => transaction(object,values) : bound(object,property),
          set:(object,property,value) => Reflect.set(object,property,value,object)
        });
        return database;
      },
      set:(target,key,value) => Reflect.set(target,key,value,target)
    });
  }};
  return {factory, transactions,
    holdNext:() => {nextHold = true;},
    release:() => {
      const before = held ? {released:held.released, expired:held.expired} : null;
      if (held) {held.released = true; clearTimeout(held.timer);}
      return before;
    },
    status:() => ({transactions:transactions.map(value => value.trace),
      held:held ? {released:held.released, expired:held.expired} : null})};
};
