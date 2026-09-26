use serde_json::json;
use std::{env, io::Read};
use task_worker::{MutationOutcome, MutationRequest, Store};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

fn reply(request: Request, status: u16, body: serde_json::Value) {
    let response = Response::from_string(body.to_string())
        .with_status_code(StatusCode(status))
        .with_header(
            Header::from_bytes("Content-Type", "application/json").expect("static header"),
        );
    let _ = request.respond(response);
}

fn task_id(path: &str) -> Option<&str> {
    let id = path.strip_prefix("/tasks/")?;
    if id.is_empty()
        || id.len() > 80
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return None;
    }
    Some(id)
}

fn handle(store: &Store, mut request: Request) {
    if request.method() == &Method::Get && request.url() == "/health" {
        match store.check_indexes() {
            Ok(true) => reply(request, 200, json!({"ready": true})),
            _ => reply(request, 503, json!({"error": "indexes_unavailable"})),
        }
        return;
    }
    let Some(id) = task_id(request.url()).map(str::to_owned) else {
        reply(request, 404, json!({"error": "unknown_path"}));
        return;
    };
    match request.method() {
        Method::Get => match store.read(&id) {
            Ok(Some(record)) => reply(
                request,
                200,
                json!({
                    "id": id, "version": record.version,
                    "title": record.title, "status": record.status,
                }),
            ),
            Ok(None) => reply(request, 404, json!({"error": "not_found"})),
            Err(_) => reply(request, 503, json!({"error": "dependency_unavailable"})),
        },
        Method::Post => {
            let mut body = Vec::new();
            if request
                .as_reader()
                .take(4097)
                .read_to_end(&mut body)
                .is_err()
                || body.is_empty()
                || body.len() > 4096
            {
                reply(request, 400, json!({"error": "invalid_body_size"}));
                return;
            }
            let mutation: MutationRequest = match serde_json::from_slice(&body) {
                Ok(value) => value,
                Err(_) => {
                    reply(request, 400, json!({"error": "invalid_mutation"}));
                    return;
                }
            };
            match store.mutate(&id, &mutation) {
                MutationOutcome::Saved(version) | MutationOutcome::Replay(version) => reply(
                    request,
                    200,
                    json!({"id": id, "version": version, "sync_token": null}),
                ),
                MutationOutcome::OperationIdReused => {
                    reply(request, 409, json!({"error": "operation_id_reused"}))
                }
                MutationOutcome::VersionConflict(current_version) => reply(
                    request,
                    409,
                    json!({"error": "version_conflict", "current_version": current_version}),
                ),
                MutationOutcome::Invalid("operation_id_expired") => {
                    reply(request, 409, json!({"error": "operation_id_expired"}))
                }
                MutationOutcome::Invalid(reason) => reply(request, 400, json!({"error": reason})),
                MutationOutcome::Unavailable => {
                    reply(request, 503, json!({"error": "dependency_unavailable"}))
                }
            }
        }
        _ => reply(request, 405, json!({"error": "method_not_allowed"})),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let uri = env::var("YAJA_MONGO_URL")?;
    let database = env::var("YAJA_TASK_DB").unwrap_or_else(|_| "yaja".into());
    let collection = env::var("YAJA_TASK_COLLECTION").unwrap_or_else(|_| "task_operations".into());
    let store = Store::connect(&uri, &database, &collection)?;
    if env::args().nth(1).as_deref() == Some("install-indexes") {
        store.install_indexes()?;
        return Ok(());
    }
    if !store.check_indexes()? {
        return Err("required task operation indexes are missing".into());
    }
    let listen = env::var("YAJA_WORKER_LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:8082".into());
    let server = Server::http(&listen)?;
    for request in server.incoming_requests() {
        handle(&store, request);
    }
    Ok(())
}
