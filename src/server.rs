//! The axum web server: routes, token auth, and TLS (R9–R18).
//!
//! Routes (R13–R16), all guarded by the token in the path:
//!
//!   ```text
//!   GET  /p/<token>          → the keyboard UI
//!   POST /p/<token>/key      → { key, hold_ms } → emit
//!   GET  /p/<token>/ping     → { ok: true }
//!   ```
//!
//! Anything else → a generic 404 (R13: don't leak that the service is up).

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use rcgen::{CertificateParams, KeyPair};

use crate::{
    sink::{Key, HOLD_CAP_MS},
    ui,
};

/// The server + the sink it drives.
pub struct AppState {
    pub token: String,
    pub sink: Arc<tokio::sync::Mutex<Box<dyn crate::sink::Sink>>>,
}

impl AppState {
    fn token_valid(&self, provided: &str) -> bool {
        crate::auth::token_is_valid(provided, &self.token)
    }
}

#[derive(serde::Deserialize)]
struct KeyBody {
    key: String,
    #[serde(default)]
    hold_ms: u64,
}

/// `GET /p/<token>` — the UI.
async fn ui_handler(
    State(st): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<axum::response::Response, StatusCode> {
    if !st.token_valid(&token) {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(axum::response::Response::builder()
        .status(200)
        .header("content-type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(ui::PAGE.to_string()))
        .unwrap())
}

/// `POST /p/<token>/key` — emit a key.
async fn key_handler(
    State(st): State<Arc<AppState>>,
    Path(token): Path<String>,
    body: Result<axum::Json<KeyBody>, axum::extract::rejection::JsonRejection>,
) -> Result<StatusCode, StatusCode> {
    if !st.token_valid(&token) {
        return Err(StatusCode::NOT_FOUND);
    }
    let body = body.map_err(|_| StatusCode::BAD_REQUEST)?;
    let key = Key::from_wire(&body.key).ok_or(StatusCode::BAD_REQUEST)?;
    let hold_ms = body.hold_ms.min(HOLD_CAP_MS as u64) as u32;
    st.sink
        .lock()
        .await
        .emit(key, hold_ms)
        .map(|_| StatusCode::OK)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// `GET /p/<token>/ping` — health.
async fn ping_handler(
    State(st): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if !st.token_valid(&token) {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::OK)
}

async fn not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

/// Build the axum router. Every path is token-guarded; unknown paths and bad
/// tokens all collapse to the same 404 (R13).
pub fn build_router(st: Arc<AppState>) -> Router {
    Router::new()
        .route("/p/{token}", get(ui_handler))
        .route("/p/{token}/key", post(key_handler))
        .route("/p/{token}/ping", get(ping_handler))
        .fallback(not_found)
        .with_state(st)
}

/// Build the in-memory self-signed TLS config (R9). Nothing is written to
/// disk; the cert + key are generated in memory and handed to rustls via PEM.
pub async fn self_signed_tls() -> Result<RustlsConfig, Box<dyn std::error::Error + Send + Sync>> {
    let mut params = CertificateParams::new(vec![]).map_err(|e| format!("params: {e}"))?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "web-remote");
    let pair = KeyPair::generate().map_err(|e| format!("keygen: {e}"))?;
    let cert = params
        .self_signed(&pair)
        .map_err(|e| format!("cert: {e}"))?;
    let cert_pem = cert.pem();
    let key_pem = pair.serialize_pem();
    RustlsConfig::from_pem(cert_pem.into_bytes(), key_pem.into_bytes())
        .await
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            format!("rustls config: {e}").into()
        })
}

/// Bind + serve. `scheme` is `"https"` (default) or `"http"` (`--http`).
pub async fn serve(
    st: Arc<AppState>,
    bind: SocketAddr,
    scheme: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = build_router(st);
    match scheme {
        "http" => {
            let listener = tokio::net::TcpListener::bind(bind).await?;
            axum::serve(listener, app.into_make_service()).await?;
            Ok(())
        }
        "https" => {
            let cfg = self_signed_tls().await?;
            axum_server::bind_rustls(bind, cfg)
                .serve(app.into_make_service())
                .await?;
            Ok(())
        }
        _ => Err("unknown scheme".into()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // `oneshot`

    use crate::{
        server::AppState,
        sink::{Key, Sink, SinkError},
    };

    /// A sink that records every key it's asked to emit, so tests can
    /// assert the HTTP layer actually drives the sink with the right key.
    /// The log is shared with a `SharedLog` handle so the test can read it
    /// after the request (the `Sink` is boxed and owned by the state).
    type Log = Arc<std::sync::Mutex<Vec<(Key, u32)>>>;

    struct RecordingSink {
        log: Log,
    }

    impl Sink for RecordingSink {
        fn emit(&self, key: Key, hold_ms: u32) -> Result<(), SinkError> {
            self.log.lock().expect("poisoned").push((key, hold_ms));
            Ok(())
        }
    }

    const TOKEN: &str = "testtoken000000000000000000000000000000";

    /// Build a router + a shared handle to the recording log.
    fn app() -> (axum::Router, Log) {
        let log: Log = Arc::new(std::sync::Mutex::new(Vec::new()));
        let st = Arc::new(AppState {
            token: TOKEN.to_string(),
            sink: Arc::new(tokio::sync::Mutex::new(Box::new(RecordingSink {
                log: log.clone(),
            }))),
        });
        (crate::server::build_router(st), log)
    }

    /// Drive a single request through the router (no port binding).
    /// Takes the builder `Result` because `Request::builder().uri(...)` yields
    /// one; we unwrap since a malformed test request is a test bug.
    async fn fire(
        app: axum::Router,
        req: Result<Request<Body>, axum::http::Error>,
    ) -> axum::response::Response {
        app.oneshot(req.expect("bad test request"))
            .await
            .expect("oneshot failed")
    }

    #[tokio::test]
    async fn ping_ok_with_valid_token() {
        let (app, _) = app();
        let res = fire(
            app,
            Request::builder()
                .uri(format!("/p/{TOKEN}/ping"))
                .body(Body::empty()),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn bad_token_is_404() {
        let (app, _) = app();
        let res = fire(
            app,
            Request::builder()
                .uri("/p/wrongtoken000000000000000000000000000000/ping")
                .body(Body::empty()),
        )
        .await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unknown_path_is_404() {
        let (app, _) = app();
        let res = fire(app, Request::builder().uri("/nope").body(Body::empty())).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn ui_served_with_all_keys() {
        let (app, _) = app();
        let res = fire(
            app,
            Request::builder()
                .uri(format!("/p/{TOKEN}"))
                .body(Body::empty()),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), 1_000_000)
            .await
            .expect("body");
        let text = String::from_utf8_lossy(&body);
        // The 11 visible keys must each appear as a data-key button.
        for k in [
            "play",
            "next",
            "prev",
            "fullscreen",
            "voldn",
            "volup",
            "mute",
            "up",
            "down",
            "left",
            "right",
        ] {
            assert!(
                text.contains(&format!("data-key=\"{k}\"")),
                "UI missing key {k}"
            );
        }
        // Stop is not a visible key (long-press on play/pause) — R21.
        assert!(
            !text.contains("data-key=\"stop\""),
            "stop must not be a visible key"
        );
    }

    /// Each of the 12 wire names must be accepted and reach the sink.
    #[tokio::test]
    async fn all_twelve_keys_accepted_and_routed() {
        for (wire, key) in [
            ("play", Key::PlayPause),
            ("stop", Key::Stop),
            ("next", Key::Next),
            ("prev", Key::Prev),
            ("volup", Key::VolUp),
            ("voldn", Key::VolDown),
            ("mute", Key::Mute),
            ("up", Key::Up),
            ("down", Key::Down),
            ("left", Key::Left),
            ("right", Key::Right),
            ("fullscreen", Key::Fullscreen),
        ] {
            let (app, log) = app();
            let res = fire(
                app,
                Request::builder()
                    .method("POST")
                    .uri(format!("/p/{TOKEN}/key"))
                    .header("content-type", "application/json")
                    .body(Body::from(format!("{{\"key\":\"{wire}\"}}"))),
            )
            .await;
            assert_eq!(res.status(), StatusCode::OK, "key {wire} should be 200");
            let logged = log.lock().expect("poisoned").clone();
            assert_eq!(logged, vec![(key, 0)], "sink should have received {key:?}");
        }
    }

    #[tokio::test]
    async fn hold_ms_is_capped_at_5000() {
        let (app, log) = app();
        fire(
            app,
            Request::builder()
                .method("POST")
                .uri(format!("/p/{TOKEN}/key"))
                .header("content-type", "application/json")
                .body(Body::from("{\"key\":\"volup\",\"hold_ms\":999999}")),
        )
        .await;
        let logged = log.lock().expect("poisoned").clone();
        assert_eq!(logged, vec![(Key::VolUp, 5000)], "hold must be capped");
    }

    #[tokio::test]
    async fn unknown_key_is_400() {
        let (app, _) = app();
        let res = fire(
            app,
            Request::builder()
                .method("POST")
                .uri(format!("/p/{TOKEN}/key"))
                .header("content-type", "application/json")
                .body(Body::from("{\"key\":\"notakey\"}")),
        )
        .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn malformed_body_is_400() {
        let (app, _) = app();
        let res = fire(
            app,
            Request::builder()
                .method("POST")
                .uri(format!("/p/{TOKEN}/key"))
                .header("content-type", "application/json")
                .body(Body::from("not json at all")),
        )
        .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
