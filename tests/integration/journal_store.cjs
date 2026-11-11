// Transactional journal port for policy/page tests; no browser persistence claim.
module.exports = function memoryStore() {
  let value = null, tail = Promise.resolve(), nextHook = null;
  return {
    async read() { await tail; return value; },
    compare(expected, replacement, admit = () => {}) {
      const hook = nextHook; nextHook = null;
      const work = tail.then(async () => {
        if (value !== expected) return false;
        if (hook) await hook(replacement);
        admit(); value = replacement; return true;
      });
      tail = work.catch(() => {}); return work;
    },
    hook(next) { nextHook = next; },
    async corrupt(change) { await tail; value = change(value); }
  };
};
