#!/bin/sh
# Runs only inside the admitted disposable PostgreSQL container. All waits and
# commands are bounded again by the caller's timeout and owned-container cleanup.
set -eu
scratch=$(mktemp -d /tmp/kehila-core-wait.XXXXXX)
owner=$(cat /proc/sys/kernel/random/uuid)
case "$owner" in ''|*[!a-f0-9-]*) exit 1;; esac
block_app="kehila-core-$owner-b"
write_app="kehila-core-$owner-w"
observe_app="kehila-core-$owner-o"
block_child=''
write_child=''
psql_cmd() {
  PGAPPNAME="$observe_app" PGOPTIONS='-c statement_timeout=1000' LC_ALL=C \
    psql -X -qAt -U postgres -d postgres -v ON_ERROR_STOP=1 -v VERBOSITY=verbose "$@"
}
cleanup() {
  original=$?
  trap - EXIT INT TERM
  # Discover only this unpredictable namespace, then bind cancellation to the
  # exact PID/application/backend-start triple rather than reusing a bare PID.
  if cancel_sql=$(psql_cmd -c "SELECT format('SELECT pg_cancel_backend(pid) FROM pg_stat_activity WHERE pid=%s AND application_name=%L AND backend_start=%L::timestamptz;',pid,application_name,backend_start) FROM pg_stat_activity WHERE application_name IN ('$block_app','$write_app','$observe_app') AND usename=current_user AND pid<>pg_backend_pid();" 2>"$scratch/cleanup.err"); then
    if [ -n "$cancel_sql" ]; then
      if ! printf '%s\n' "$cancel_sql" | psql_cmd >"$scratch/cleanup.out" 2>>"$scratch/cleanup.err"; then
        [ "$original" -ne 0 ] || original=1
      fi
    fi
  else
    [ "$original" -ne 0 ] || original=1
  fi
  if [ -n "$block_child" ]; then wait "$block_child" || :; fi
  if [ -n "$write_child" ]; then wait "$write_child" || :; fi
  if remaining=$(psql_cmd -c "SELECT count(*) FROM pg_stat_activity WHERE application_name IN ('$block_app','$write_app','$observe_app') AND pid<>pg_backend_pid();" 2>>"$scratch/cleanup.err"); then
    if [ "$remaining" != 0 ]; then [ "$original" -ne 0 ] || original=1; fi
  else
    [ "$original" -ne 0 ] || original=1
  fi
  if [ "$original" -ne 0 ]; then
    for diagnostic in "$scratch/block.err" "$scratch/write.err" "$scratch/cleanup.err"; do
      if [ -f "$diagnostic" ]; then cat "$diagnostic" >&2; fi
    done
  fi
  exit "$original"
}
trap cleanup EXIT
trap 'exit 1' INT TERM

psql_cmd -c "DO \$\$ BEGIN
  IF (SELECT count(*) FROM kehila_auth.test_wait_packet)<>1
     OR EXISTS(SELECT FROM kehila.operations WHERE actor_id='owner' AND command_family='item_edit'
       AND target_project_id='p' AND target_item_id='saved-item' AND token='wait-core')
     OR NOT EXISTS(SELECT FROM kehila_auth.test_wait_packet p
       JOIN kehila.items i ON i.project_id='p' AND i.item_id='saved-item' AND i.current_version=(p.prepared).item_version
       JOIN kehila.project_operational_heads h ON h.project_id='p' AND h.state_version=(p.prepared).state_version
         AND h.operation_row_id=(p.prepared).state_operation
       JOIN kehila.projects j ON j.project_id='p' AND j.configuration_revision=(p.prepared).configuration_revision
       WHERE convert_from(p.request_bytes,'UTF8')::jsonb #>>'{value,operation_id}'='wait-core'
         AND convert_from(p.request_bytes,'UTF8')::jsonb #>>'{value,expected_item_version}'=i.current_version::numeric(20,0)::text
         AND p.history #>>'{value,previous_project_operation}'=h.operation_row_id::text
         AND (p.history #>>'{value,project_state_version}')::numeric=h.state_version+1) THEN
    RAISE EXCEPTION 'fresh wait packet or unseen identity required';
  END IF;
END; \$\$;"
psql_cmd >"$scratch/block.out" 2>"$scratch/block.err" <<SQL &
SET application_name='$block_app';
SET statement_timeout='15s';
SET idle_in_transaction_session_timeout='15s';
SET ROLE kehila_owner;
BEGIN;
DO \$\$ BEGIN
  PERFORM FROM kehila.operations o JOIN kehila.project_operational_heads h ON h.operation_row_id=o.operation_row_id
    CROSS JOIN kehila_auth.test_wait_packet p
    WHERE h.project_id='p' AND h.operation_row_id=(p.prepared).state_operation
    FOR NO KEY UPDATE OF o;
  IF NOT FOUND THEN RAISE EXCEPTION 'actual predecessor core required'; END IF;
END; \$\$;
SELECT pg_sleep(12);
ROLLBACK;
SQL
block_child=$!
attempt=0
block_identity=''
while [ "$attempt" -lt 60 ]; do
  block_identity=$(psql_cmd -c "SELECT pid||'|'||extract(epoch FROM backend_start)::text FROM pg_stat_activity WHERE application_name='$block_app' AND usename=current_user AND state='active' AND wait_event='PgSleep' AND xact_start IS NOT NULL;")
  [ -z "$block_identity" ] || break
  kill -0 "$block_child" 2>/dev/null || { echo 'blocker exited before positive readiness' >&2; exit 1; }
  attempt=$((attempt+1))
  sleep 0.05
done
[ -n "$block_identity" ] || { echo 'blocker readiness not observed' >&2; exit 1; }
block_pid=${block_identity%%|*}
block_start=${block_identity#*|}
case "$block_pid" in ''|*[!0-9]*) exit 1;; esac
case "$block_start" in ''|*[!0-9.]*) exit 1;; esac

psql_cmd >"$scratch/write.out" 2>"$scratch/write.err" <<SQL &
SET application_name='$write_app';
SET statement_timeout='15s';
SET ROLE kehila_owner;
BEGIN;
SELECT r.outcome FROM kehila_auth.test_wait_packet p,LATERAL kehila_auth.save_item_edit(
  p.authority,'p','saved-item','wait-core',p.request_bytes,p.prepared,p.history) r;
SET CONSTRAINTS ALL IMMEDIATE;
ROLLBACK;
SELECT 'writer-rolled-back';
SQL
write_child=$!
attempt=0
waiting=0
while [ "$attempt" -lt 60 ]; do
  waiting=$(psql_cmd -c "SELECT count(*) FROM pg_stat_activity w JOIN pg_stat_activity b ON b.pid=$block_pid WHERE w.application_name='$write_app' AND w.usename=current_user AND w.wait_event_type='Lock' AND b.application_name='$block_app' AND extract(epoch FROM b.backend_start)=$block_start AND b.wait_event='PgSleep' AND b.pid=ANY(pg_blocking_pids(w.pid));")
  [ "$waiting" != 1 ] || break
  kill -0 "$write_child" 2>/dev/null || break
  attempt=$((attempt+1))
  sleep 0.05
done
if [ "$waiting" = 1 ]; then
  item_probe=$(psql_cmd -c "BEGIN; SET ROLE kehila_owner; SELECT 1 FROM kehila.items WHERE project_id='p' AND item_id='saved-item' FOR NO KEY UPDATE NOWAIT; ROLLBACK;")
  [ "$item_probe" = 1 ] || { echo 'exact item probe missing' >&2; exit 1; }
else
  # Timeout/absence alone is not the expected defect. Prove the fresh writer
  # already accepted and rolled back while the positively ready blocker remains.
  [ "$(cat "$scratch/write.out")" = 'accepted
writer-rolled-back' ] || { echo 'wait not observed and writer not yet completed; ambiguous control' >&2; exit 1; }
  write_remaining=$(psql_cmd -c "SELECT count(*) FROM pg_stat_activity WHERE application_name='$write_app';")
  [ "$write_remaining" = 0 ] || { echo 'nonblocking writer completion not established' >&2; exit 1; }
fi
cancelled=$(psql_cmd -c "SELECT pg_cancel_backend(pid) FROM pg_stat_activity WHERE pid=$block_pid AND application_name='$block_app' AND extract(epoch FROM backend_start)=$block_start AND wait_event='PgSleep';")
[ "$cancelled" = t ] || { echo 'intentional blocker cancellation not established' >&2; exit 1; }
if wait "$block_child"; then block_status=0; else block_status=$?; fi
block_child=''
[ "$block_status" = 3 ] || { echo 'unexpected blocker exit' >&2; exit 1; }
case "$(cat "$scratch/block.err")" in *"57014:"*"canceling statement due to user request"*) :;;
  *) echo 'intentional blocker SQL cancellation not observed' >&2; exit 1;; esac
if wait "$write_child"; then write_status=0; else write_status=$?; fi
write_child=''
[ "$write_status" = 0 ] || { echo 'writer failed before accepted rollback' >&2; exit 1; }
[ "$(cat "$scratch/write.out")" = 'accepted
writer-rolled-back' ] || { echo 'fresh writer did not accept then roll back' >&2; exit 1; }
unchanged=$(psql_cmd -c "SELECT (SELECT count(*)=11 FROM kehila.operations) AND (SELECT count(*)=9 FROM kehila.item_revisions) AND (SELECT count(*)=7 FROM kehila.operation_payloads) AND (SELECT count(*)=3 FROM kehila.items) AND (SELECT current_version=2 FROM kehila.items WHERE item_id='saved-item') AND (SELECT state_version=9 FROM kehila.project_operational_heads WHERE project_id='p');")
[ "$unchanged" = t ] || { echo 'writer rollback state oracle failed' >&2; exit 1; }
psql_cmd -c 'SET ROLE kehila_owner; DROP TABLE kehila_auth.test_wait_packet;'
if [ "$waiting" != 1 ]; then
  echo 'EXPECTED_MISSING_PREDECESSOR_FENCE: fresh edit accepted and rolled back without predecessor wait' >&2
  exit 41
fi
printf '%s\n' 'PASS: predecessor core wait precedes item acquisition and leaves no accepted work'
