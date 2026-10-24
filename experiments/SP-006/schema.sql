-- Disposable protocol fixture. No application schema/capability adoption.
CREATE SCHEMA sp006;
CREATE TABLE sp006.documents (
  doc_id text PRIMARY KEY CHECK (doc_id ~ '^[a-z]{1,8}$'),
  revision bigint NOT NULL CHECK (revision > 0),
  value text NOT NULL CHECK (octet_length(value) <= 128)
);
CREATE TABLE sp006.targets (
  name text PRIMARY KEY,
  phase text NOT NULL CHECK (phase IN ('active','building','retained','failed'))
);
CREATE UNIQUE INDEX one_build ON sp006.targets ((phase)) WHERE phase='building';
CREATE UNIQUE INDEX one_active ON sp006.targets ((phase)) WHERE phase='active';
CREATE TABLE sp006.control (
  singleton boolean PRIMARY KEY CHECK (singleton), active text REFERENCES sp006.targets,
  next_target text REFERENCES sp006.targets, phase text NOT NULL
    CHECK (phase IN ('live','building','switching')), snapshot_complete boolean NOT NULL
);
CREATE TABLE sp006.events (
  event_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  doc_id text NOT NULL, revision bigint NOT NULL, value text NOT NULL,
  UNIQUE (doc_id,revision)
);
CREATE TABLE sp006.deliveries (
  event_id bigint REFERENCES sp006.events, target text REFERENCES sp006.targets,
  acknowledged boolean NOT NULL DEFAULT false, PRIMARY KEY (event_id,target)
);
CREATE FUNCTION sp006.immutable_event() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'immutable outbox'; END $$;
CREATE TRIGGER immutable_event BEFORE UPDATE OR DELETE ON sp006.events
FOR EACH ROW EXECUTE FUNCTION sp006.immutable_event();
-- Every projecting writer must use this gate before any domain/actor locks.
-- The fixture has no authentication: this is not a production grant design.
CREATE FUNCTION sp006.put(id text, contents text) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE rev bigint; event bigint;
BEGIN
  PERFORM pg_advisory_xact_lock_shared(6006001);
  INSERT INTO sp006.documents VALUES (id,1,contents)
    ON CONFLICT (doc_id) DO UPDATE SET revision=sp006.documents.revision+1,value=EXCLUDED.value
    RETURNING revision INTO rev;
  INSERT INTO sp006.events(doc_id,revision,value) VALUES(id,rev,contents) RETURNING event_id INTO event;
  INSERT INTO sp006.deliveries(event_id,target)
    SELECT event,name FROM sp006.targets WHERE phase IN ('active','building');
  RETURN rev;
END $$;
