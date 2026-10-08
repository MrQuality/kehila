-- Deferred invoker triggers execute at COMMIT after SECURITY DEFINER returns.
-- Compactor needs read access; this grants no write or actor capability privileges.
GRANT SELECT ON kehila.operations,kehila.operation_payloads TO qa_compactor;
