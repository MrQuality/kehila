use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::any,
    Json, Router,
};
use serde_json::{json, Value};
use std::{env, net::SocketAddr, sync::Arc, time::Duration};
use task_worker::{MutationOutcome, MutationRequest, OperationPolicy, Store};
use tokio::{net::TcpListener, sync::Semaphore, time::timeout};

#[derive(Clone)]
struct App {
    store: Arc<Store>,
    authority: String,
    slots: Arc<Semaphore>,
}

fn reply(status: u16, body: Value) -> Response {
    (
        StatusCode::from_u16(status).expect("static status"),
        Json(body),
    )
        .into_response()
}

fn task_id(path: &str) -> Option<&str> {
    let id = path.strip_prefix("/tasks/")?;
    if id.is_empty()
        || id.len() > 80
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return None;
    }
    Some(id)
}

async fn bounded(State(app): State<App>, request: Request, next: Next) -> Response {
    // This listener is an internal Go-to-worker boundary, not a browser API.
    // Go constructs a fresh request and never forwards browser context headers.
    if request.headers().get("host").and_then(|h| h.to_str().ok()) != Some(app.authority.as_str())
        || request.headers().contains_key("origin")
        || request.headers().contains_key("sec-fetch-site")
    {
        return reply(403, json!({"error": "origin_forbidden"}));
    }
    let Ok(_permit) = app.slots.try_acquire() else {
        return reply(503, json!({"error": "worker_busy"}));
    };
    timeout(Duration::from_secs(5), next.run(request))
        .await
        .unwrap_or_else(|_| reply(503, json!({"error": "dependency_unavailable"})))
}

async fn handle(State(app): State<App>, request: Request) -> Response {
    if request.method() == Method::GET && request.uri().path() == "/health" {
        return match app.store.check_indexes().await {
            Ok(true) => reply(200, json!({"ready": true})),
            _ => reply(503, json!({"error": "indexes_unavailable"})),
        };
    }
    let Some(id) = task_id(request.uri().path()).map(str::to_owned) else {
        return reply(404, json!({"error": "unknown_path"}));
    };
    match *request.method() {
        Method::GET => match app.store.read(&id).await {
            Ok(Some(record)) => reply(
                200,
                json!({"id": id, "version": record.version, "title": record.title, "status": record.status}),
            ),
            Ok(None) => reply(404, json!({"error": "not_found"})),
            Err(_) => reply(503, json!({"error": "dependency_unavailable"})),
        },
        Method::POST => {
            if request
                .headers()
                .get("content-type")
                .and_then(|h| h.to_str().ok())
                != Some("application/json")
            {
                return reply(415, json!({"error": "json_required"}));
            }
            let body =
                match timeout(Duration::from_secs(3), to_bytes(request.into_body(), 4096)).await {
                    Err(_) => return reply(408, json!({"error": "body_timeout"})),
                    Ok(Err(_)) => return reply(400, json!({"error": "invalid_body_size"})),
                    Ok(Ok(body)) => body,
                };
            let mutation: MutationRequest = match serde_json::from_slice(&body) {
                Ok(value) => value,
                Err(_) => return reply(400, json!({"error": "invalid_mutation"})),
            };
            match app.store.mutate(&id, &mutation).await {
                MutationOutcome::Saved(version) | MutationOutcome::Replay(version) => reply(
                    200,
                    json!({"id": id, "version": version, "sync_token": null}),
                ),
                MutationOutcome::OperationIdReused => {
                    reply(409, json!({"error": "operation_id_reused"}))
                }
                MutationOutcome::VersionConflict(current_version) => reply(
                    409,
                    json!({"error": "version_conflict", "current_version": current_version}),
                ),
                MutationOutcome::Invalid("operation_id_expired") => {
                    reply(409, json!({"error": "operation_id_expired"}))
                }
                MutationOutcome::Invalid(reason) => reply(400, json!({"error": reason})),
                MutationOutcome::Unavailable => {
                    reply(503, json!({"error": "dependency_unavailable"}))
                }
            }
        }
        _ => reply(405, json!({"error": "method_not_allowed"})),
    }
}

#[tokio::main(worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let uri = env::var("KEHILA_MONGO_URL")?;
    let database = env::var("KEHILA_TASK_DB").unwrap_or_else(|_| "yaja".into());
    let collection =
        env::var("KEHILA_TASK_COLLECTION").unwrap_or_else(|_| "task_operations".into());
    let policy = OperationPolicy {
        admission_days: env::var("KEHILA_OPERATION_ADMISSION_DAYS")
            .unwrap_or_else(|_| "90".into())
            .parse()?,
        replay_days: env::var("KEHILA_OPERATION_REPLAY_DAYS")
            .unwrap_or_else(|_| "90".into())
            .parse()?,
    };
    if policy.admission_days == 0 || policy.replay_days == 0 {
        return Err("operation admission and replay periods must be positive".into());
    }
    let store = timeout(
        Duration::from_secs(5),
        Store::connect(&uri, &database, &collection, policy),
    )
    .await??;
    match env::args().nth(1).as_deref() {
        Some("install-indexes") => {
            timeout(Duration::from_secs(5), store.install_indexes()).await??;
            return Ok(());
        }
        Some("drop-test-collection") => {
            timeout(Duration::from_secs(5), store.drop_test_collection()).await??;
            return Ok(());
        }
        Some(_) => return Err("unknown command".into()),
        None => {}
    }
    if !timeout(Duration::from_secs(5), store.check_indexes()).await?? {
        return Err("required task operation indexes are missing".into());
    }
    let listen: SocketAddr = env::var("KEHILA_WORKER_LISTEN_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8082".into())
        .parse()?;
    if !listen.ip().is_loopback() {
        return Err("worker must bind to loopback".into());
    }
    let listener = TcpListener::bind(listen).await?;
    let app = App {
        store: Arc::new(store),
        authority: listener.local_addr()?.to_string(),
        slots: Arc::new(Semaphore::new(16)),
    };
    let router = Router::new()
        .fallback(any(handle))
        .layer(middleware::from_fn_with_state(app.clone(), bounded))
        .with_state(app);
    axum::serve(listener, router).await?;
    Ok(())
}
