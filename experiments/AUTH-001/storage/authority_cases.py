"""Native runtime-start and post-wait authority oracles for disposable A1."""
from sessions import Session, SqlFailure, blocked
from enrollment_cases import denied, require

RUNTIME = 'auth001_test_runtime'
KEY = '1262831689,1'


def acquire(session):
    require(session.execute('SELECT pg_try_advisory_lock(' + KEY + ');') == 't',
            'dedicated connection did not acquire the session lock')


def reject_waiter(session):
    try:
        session.wait()
    except SqlFailure as error:
        require(error.state == '55000', 'stale authority returned wrong SQLSTATE')
    else:
        raise ValueError('post-wait stale authority was accepted')


def run(db):
    denied(db, "SELECT * FROM auth001.fence('installation',0,1,1);", '55000', user=RUNTIME)
    denied(db, "SELECT * FROM auth001.start_runtime('installation','without-lock');", '55000',
           user=RUNTIME)
    denied(db, "SELECT * FROM auth001.start_runtime('installation','admin');", '42501')
    with Session(db, RUNTIME) as serving, Session(db, 'postgres') as observer:
        with Session(db, RUNTIME) as first:
            acquire(first)
            first.execute('BEGIN;')
            first.execute("SELECT * FROM auth001.start_runtime('installation','rolled-back-start');")
            first.execute('ROLLBACK;')
            require(db.sql('SELECT runtime_generation FROM auth001.installation;') == '0',
                    'rolled-back startup changed durable generation')
            require(db.sql("SELECT count(*) FROM auth001.management_events WHERE event_kind='runtime_start';") == '0',
                    'rolled-back startup retained an audit event')
            # Session acquisition must survive rollback as well as startup commit.
            require(serving.execute('SELECT pg_try_advisory_lock(' + KEY + ');') == 'f',
                    'startup rollback lost session-duration ownership')
            require(first.execute("SELECT * FROM auth001.start_runtime('installation','first-start');")
                    == 'installation|owner|1|1|1', 'first startup metadata mismatch')
            require(serving.execute('SELECT pg_try_advisory_lock(' + KEY + ');') == 'f',
                    'startup commit lost session-duration ownership')
            denied(db, "SELECT * FROM auth001.start_runtime('installation','wrong-backend');", '55000',
                   user=RUNTIME)
            # This is a separate serving transaction, not the lock connection.
            serving.execute('BEGIN;')
            require(serving.execute("SELECT * FROM auth001.fence('installation',1,1,1);")
                    == 'installation|owner|1|1|1', 'initial serving fence metadata mismatch')
        # Closing the first advisory connection must not bypass its in-flight work.
        with Session(db, RUNTIME) as replacement:
            acquire(replacement)
            replacement.send("SELECT * FROM auth001.start_runtime('installation','second-start');")
            blocked(observer, replacement, serving)
            serving.execute('COMMIT;')
            require(replacement.wait() == 'installation|owner|2|1|1',
                    'takeover did not advance after fenced work committed')
            denied(db, "SELECT * FROM auth001.fence('installation',1,1,1);", '55000', user=RUNTIME)
            # A waiter that began before takeover commit must use a fresh snapshot.
            replacement.execute('BEGIN;')
            replacement.execute("SELECT * FROM auth001.start_runtime('installation','third-start');")
            serving.send("SELECT * FROM auth001.fence('installation',2,1,1);")
            blocked(observer, serving, replacement)
            replacement.execute('COMMIT;')
            reject_waiter(serving)
    # Deliberately ignore the third committed result: reacquire on a fresh owned
    # connection with a fresh attempt, never reconstruct or reuse the old runtime.
    with Session(db, RUNTIME) as fresh:
        acquire(fresh)
        require(fresh.execute("SELECT * FROM auth001.start_runtime('installation','fresh-after-loss');")
                == 'installation|owner|4|1|1', 'unknown-response restart reused an old generation')
        denied(db, "SELECT * FROM auth001.fence('installation',4,1,99);", '55000', user=RUNTIME)
        db.sql("UPDATE kehila.actors SET active=false WHERE actor_id='owner';")
        denied(db, "SELECT * FROM auth001.fence('installation',4,1,1);", '55000', user=RUNTIME)
        db.sql("UPDATE kehila.actors SET active=true WHERE actor_id='owner';")
        db.sql('UPDATE auth001.owner_credentials SET verifier=NULL;')
        denied(db, "SELECT * FROM auth001.fence('installation',4,1,1);", '55000', user=RUNTIME)
        db.sql("UPDATE auth001.owner_credentials SET verifier='synthetic-verifier';")
        # Put this expected SQL error last: psql's ON_ERROR_STOP ends its owned
        # connection, so it must not release the earlier test's lock prematurely.
        try:
            fresh.execute("SELECT * FROM auth001.start_runtime('installation','first-start');")
        except SqlFailure as error:
            require(error.state == '23505', 'startup attempt reuse returned wrong SQLSTATE')
        else:
            raise ValueError('completed startup attempt reused')
    denied(db, "BEGIN ISOLATION LEVEL REPEATABLE READ; "
               "SELECT * FROM auth001.fence('installation',4,1,1);", '25000', user=RUNTIME)
    with Session(db, 'postgres') as overflow:
        acquire(overflow)
        overflow.execute('BEGIN; UPDATE auth001.installation '
                         'SET runtime_generation=18446744073709551615; '
                         'SET LOCAL ROLE auth001_test_runtime;')
        try:
            overflow.execute("SELECT * FROM auth001.start_runtime('installation','overflow');")
        except SqlFailure as error:
            require(error.state == '23514', 'runtime overflow returned wrong SQLSTATE')
        else:
            raise ValueError('runtime generation overflow accepted')
    require(db.sql('SELECT runtime_generation FROM auth001.installation;') == '4',
            'failed startup did not roll back its generation')
    require(db.sql("SELECT count(*) FROM auth001.management_events WHERE event_kind='runtime_start';") == '4',
            'startup history contains rollback/retry residue')
    return ['dedicated_session_lock_and_startup', 'post_wait_takeover_fencing']
