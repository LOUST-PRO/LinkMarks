//! HTTP sync server (yrs over axum).
//!
//! Holds a single shared YDoc. Clients POST encoded yrs update bytes
//! to `/sync`; server applies them and returns its own update since the
//! client's `StateVector` (extracted from the request body — we send
//! state vector + update as a single concatenation).
//!
//! Wire format (intentionally minimal — spike only):
//! - POST /sync body: `body[0..8] LE u64 = state_vector_len`
//!                   followed by `body[8..8+sv_len] = state_vector bytes`
//!                   followed by `body[8+sv_len..] = yrs update bytes`
//! - 200 OK body: server's yrs update bytes (full state relative to
//!                client's state vector)
//! - GET /healthz: returns "ok"
//!
//! In production (post-spike) we would use `yrs::sync::SyncMessage` v1/v2
//! framing directly — but for the spike we keep it explicit and readable.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use yrs::updates::decoder::Decode;
use yrs::{Doc, ReadTxn, StateVector, Transact, WriteTxn};

#[derive(Clone)]
struct AppState {
    doc: Arc<Doc>,
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let doc = Doc::new();
    // Pre-create the maps so the first /sync POST has something to merge into.
    {
        let mut t = doc.transact_mut();
        t.get_or_insert_map("bookmarks");
        t.get_or_insert_map("tags_by_bookmark");
        t.get_or_insert_map("meta");
        t.commit();
    }

    let state = AppState { doc: Arc::new(doc) };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/state", get(get_state))
        .route("/sync", post(post_sync))
        .with_state(state);

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    eprintln!("[server] listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind failed");
    axum::serve(listener, app).await.expect("serve failed");
}

async fn healthz() -> &'static str {
    "ok"
}

async fn get_state(State(state): State<AppState>) -> impl IntoResponse {
    let bytes = {
        let txn = state.doc.transact();
        let sv = StateVector::default();
        txn.encode_state_as_update_v1(&sv)
    };
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        bytes,
    )
}

/// Decode request body:
///   `[8B u64 LE = sv_len][sv_len B = state vector][rest = yrs update]`
fn decode_body(body: &[u8]) -> Option<(StateVector, Vec<u8>)> {
    if body.len() < 8 {
        return None;
    }
    let sv_len = u64::from_le_bytes(body[0..8].try_into().ok()?) as usize;
    if body.len() < 8 + sv_len {
        return None;
    }
    let sv_bytes = &body[8..8 + sv_len];
    let update = body[8 + sv_len..].to_vec();
    StateVector::decode_v1(sv_bytes).ok().map(|sv| (sv, update))
}

/// Encode response: full server state relative to client's state vector.
fn encode_response(doc: &Doc, sv: &StateVector) -> Vec<u8> {
    let txn = doc.transact();
    txn.encode_state_as_update_v1(sv)
}

async fn post_sync(State(state): State<AppState>, body: Bytes) -> impl IntoResponse {
    let (client_sv, client_update) = match decode_body(&body) {
        Some(v) => v,
        None => {
            return (StatusCode::BAD_REQUEST, "bad framing".to_string()).into_response();
        }
    };

    // Apply the client's update.
    {
        let mut t = state.doc.transact_mut();
        match yrs::Update::decode_v1(&client_update) {
            Ok(parsed) => t.apply_update(parsed),
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    format!("decode_v1 failed: {e}"),
                )
                    .into_response();
            }
        }
        t.commit();
    }

    // Return server state relative to client's known SV.
    let response = encode_response(&state.doc, &client_sv);
    (StatusCode::OK, response).into_response()
}
