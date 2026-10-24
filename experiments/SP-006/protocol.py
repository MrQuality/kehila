"""Bounded experimental rebuild coordinator, deliberately outside production."""
import hashlib
import json
import re
import urllib.error
import urllib.parse
import urllib.request

from verification_budget import command_timeout

GATE = 6006001
COORDINATOR = 6006002
LIMIT = 32


def require(condition, message):
    if not condition:
        raise ValueError(message)


def literal(value):
    require(re.fullmatch(r"[a-z0-9_-]{1,80}", value) is not None, 'invalid fixture identifier')
    return "'" + value + "'"


def document(doc_id, revision, value):
    return {'doc_id': doc_id, 'projection_revision': revision, 'value': value}


def digest(body):
    return hashlib.sha256(json.dumps(body, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


class Search:
    """Only the owned disposable loopback OpenSearch service; no credentials."""
    def __init__(self, prefix):
        literal(prefix)
        self.prefix = prefix
        self.alias = prefix + '_read'
        self.client = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

    def request(self, method, path, body=None):
        require(path.startswith('/') and '..' not in path, 'invalid fixture path')
        data = None if body is None else json.dumps(body).encode()
        request = urllib.request.Request('http://127.0.0.1:9200' + path, data=data,
                                         method=method, headers={'Content-Type': 'application/json'})
        try:
            response = self.client.open(request, timeout=command_timeout(5))
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read(65537)
            require(len(raw) <= 65536, 'oversized search response')
            return response.status, json.loads(raw)

    def index(self, name):
        require(name.startswith(self.prefix + '_'), 'unowned index')
        literal(name)
        status, _ = self.request('PUT', '/' + name, {'settings': {'number_of_shards': 1,
                                                               'number_of_replicas': 0,
                                                               'refresh_interval': '-1'}})
        require(status == 200, 'index creation failed')

    def read(self, name, doc_id):
        literal(name); literal(doc_id)
        status, result = self.request('GET', '/' + name + '/_doc/' + doc_id)
        require(status == 200, 'document unavailable')
        return result

    def publish(self, target, body):
        require(target.startswith(self.prefix + '_'), 'unowned index')
        literal(target); literal(body['doc_id'])
        hashed = digest(body)
        payload = dict(body, payload_sha256=hashed)
        path = '/' + target + '/_doc/' + body['doc_id'] + '?version_type=external&version=' + str(body['projection_revision'])
        status, _ = self.request('PUT', path, payload)
        if status in (200, 201):
            return 'applied'
        require(status == 409, 'index delivery failed; leave pending')
        current = self.read(target, body['doc_id'])
        if current['_version'] > body['projection_revision']:
            return 'superseded'
        require(current['_version'] == body['projection_revision'], 'unexpected version conflict')
        source = current['_source']
        require(source == payload, 'same-version content mismatch; leave pending')
        return 'duplicate'

    def refresh(self, target):
        status, result = self.request('POST', '/' + target + '/_refresh')
        require(status == 200 and result['_shards']['failed'] == 0, 'refresh failed')

    def alias_target(self):
        status, result = self.request('GET', '/_alias/' + self.alias)
        require(status == 200 and len(result) == 1, 'ambiguous alias')
        return next(iter(result))

    def switch(self, old, new):
        require(self.alias_target() == old, 'alias no longer matches intent')
        status, result = self.request('POST', '/_aliases', {'actions': [
            {'remove': {'index': old, 'alias': self.alias, 'must_exist': True}},
            {'add': {'index': new, 'alias': self.alias}}]})
        require(status == 200 and result.get('acknowledged') is True, 'alias result ambiguous')


class Rebuild:
    """Borrow owned SQL participants; coordinator ownership outlives writer fences.

    No writer transaction or actor lock spans external search I/O. The bounded
    read-only source snapshot stays open during bulk-copy HTTP and pins MVCC.
    A session maintenance gate blocks writers during setup/cutover, by design.
    Only tiny synthetic fixtures are supported; all epochs/indexes are test-owned.
    """
    def __init__(self, database, coordinator, search):
        self.db, self.session, self.search = database, coordinator, search
        self.fenced = False
        self.session.execute('SELECT pg_advisory_lock(' + str(COORDINATOR) + ');')

    def begin_fence(self):
        require(not self.fenced, 'already fenced')
        self.session.send('SELECT pg_advisory_lock(' + str(GATE) + ');')

    def finish_fence(self):
        self.session.wait()
        self.fenced = True

    def fence(self):
        self.begin_fence(); self.finish_fence()

    def unfence(self):
        require(self.fenced, 'not fenced')
        require(self.session.execute('SELECT pg_advisory_unlock(' + str(GATE) + ');') == 't', 'gate ownership lost')
        self.fenced = False

    def register(self, target, snapshot):
        require(self.fenced, 'registration requires fence')
        self.search.index(target)
        self.session.execute("BEGIN; INSERT INTO sp006.targets VALUES (" + literal(target) + ", 'building'); "
                    "UPDATE sp006.control SET next_target=" + literal(target) + ", phase='building', snapshot_complete=false; COMMIT;")
        # A statement actually establishes the snapshot before releasing the gate.
        snapshot.execute('BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY; SELECT count(*) FROM sp006.documents;')
        self.unfence()

    def snapshot(self, target, source):
        cursor = ''
        for _ in range(LIMIT):
            raw = source.execute("SELECT row_to_json(d) FROM (SELECT doc_id,revision,value FROM sp006.documents "
                                 "WHERE doc_id > '" + cursor + "' ORDER BY doc_id LIMIT 1) d;")
            if not raw:
                source.execute('COMMIT;')
                self.session.execute('UPDATE sp006.control SET snapshot_complete=true WHERE next_target=' + literal(target) + ';')
                return
            row = json.loads(raw); cursor = row['doc_id']; literal(cursor)
            self.search.publish(target, document(cursor, row['revision'], row['value']))
        raise ValueError('snapshot fixture bound exceeded')

    def delivery(self, event_id, target, *, acknowledge=True):
        require(type(event_id) is int and event_id > 0, 'invalid event ID')
        raw = self.db.sql('SELECT row_to_json(e) FROM sp006.events e JOIN sp006.deliveries d USING(event_id) '
                          'WHERE event_id=' + str(event_id) + ' AND target=' + literal(target) + ';')
        require(bool(raw), 'missing delivery')
        row = json.loads(raw)
        outcome = self.search.publish(target, document(row['doc_id'], row['revision'], row['value']))
        if acknowledge:
            self.db.sql('UPDATE sp006.deliveries SET acknowledged=true WHERE event_id=' + str(event_id)
                        + ' AND target=' + literal(target) + ';')
        return outcome

    def drain(self, target):
        for _ in range(LIMIT):
            pending = self.db.sql('SELECT event_id FROM sp006.deliveries WHERE NOT acknowledged AND target='
                                  + literal(target) + ' ORDER BY event_id LIMIT 1;')
            if not pending:
                return
            self.delivery(int(pending), target)
        raise ValueError('delivery fixture bound exceeded')

    def verify_documents(self, target):
        rows = json.loads(self.db.sql("SELECT coalesce(json_agg(d),'[]'::json) FROM sp006.documents d;"))
        require(len(rows) < LIMIT, 'fixture document bound exceeded')
        for row in rows:
            expected = document(row['doc_id'], row['revision'], row['value'])
            require(self.search.read(target, row['doc_id'])['_source'] == dict(expected, payload_sha256=digest(expected)),
                    'incomplete target')

    def cutover(self, target, *, crash_after_alias=False):
        require(self.fenced, 'cutover requires fence')
        require(self.session.execute('SELECT snapshot_complete FROM sp006.control WHERE next_target=' + literal(target) + ';') == 't',
                'incomplete or lost snapshot')
        self.drain(target)
        self.verify_documents(target)
        self.search.refresh(target)
        old = self.session.execute('SELECT active FROM sp006.control;')
        self.session.execute("UPDATE sp006.control SET phase='switching';")
        actual = self.search.alias_target()
        require(actual in (old, target), 'alias inconsistent with intent')
        if actual == old:
            self.search.switch(old, target)
        if crash_after_alias:
            raise InterruptedError('injected coordinator interruption after alias write')
        require(self.search.alias_target() == target, 'alias switch not confirmed')
        self.session.execute("BEGIN; UPDATE sp006.targets SET phase='retained' WHERE name=" + literal(old) + '; '
                    "UPDATE sp006.targets SET phase='active' WHERE name=" + literal(target) + '; '
                    'UPDATE sp006.control SET active=' + literal(target) + ",next_target=NULL,phase='live'; COMMIT;")
        self.unfence()

    def abandon_snapshot(self, target):
        require(self.fenced, 'abandon requires fence')
        self.session.execute("BEGIN; UPDATE sp006.targets SET phase='failed' WHERE name=" + literal(target)
                    + "; UPDATE sp006.control SET next_target=NULL,phase='live',snapshot_complete=false; COMMIT;")
        self.unfence()
