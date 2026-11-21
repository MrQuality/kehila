"use strict";

// Database identities belong to the controller, including when a target never
// completed initialization. The caller still owns target/profile shutdown.
async function releaseDatabases(evaluate, sessions, databaseNames) {
  if (!databaseNames.length) return {recoveredFailures:0};
  const failures = [];
  for (const session of sessions) {
    try {
      await evaluate(session, `fixture.cleanup(${JSON.stringify(databaseNames)})`, 7000);
      return {recoveredFailures:failures.length};
    } catch (error) {
      failures.push(error);
    }
  }
  throw new AggregateError(failures, "Database release failed in every known context");
}

module.exports = {releaseDatabases};
