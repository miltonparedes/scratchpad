mod db;
mod handlers;
mod models;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context as _, Result};
use axum::{
    Router,
    http::{HeaderValue, Method, header},
    routing::{get, post},
};
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use db::Database;

pub struct AppState {
    pub db: Database,
    pub tx: broadcast::Sender<String>,
    pub server_token: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "scratchpad_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_path =
        std::env::var("DATABASE_PATH").unwrap_or_else(|_| "scratchpad-server.db".to_string());
    let db = Database::open(&db_path)?;
    db.init()?;

    let (tx, _rx) = broadcast::channel::<String>(100);

    let server_token = std::env::var("SERVER_TOKEN").ok().filter(|s| !s.is_empty());

    let state = Arc::new(AppState {
        db,
        tx,
        server_token,
    });

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/api/ops", post(handlers::push_ops))
        .route("/api/ops/{workspace_id}", get(handlers::get_ops))
        .route("/api/snapshot/{workspace_id}", get(handlers::get_snapshot))
        .route(
            "/api/snapshot/{workspace_id}",
            post(handlers::save_snapshot),
        )
        .route("/ws", get(handlers::websocket_handler))
        .layer(cors_layer()?)
        .with_state(state);

    let host: IpAddr = std::env::var("HOST")
        .unwrap_or_else(|_| "127.0.0.1".to_string())
        .parse()
        .context("Invalid HOST")?;

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .context("Invalid PORT")?;

    let addr = SocketAddr::from((host, port));
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn cors_layer() -> Result<CorsLayer> {
    let layer = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    if std::env::var("CORS_ALLOW_ANY").as_deref() == Ok("true") {
        return Ok(layer.allow_origin(Any));
    }

    match std::env::var("CORS_ORIGIN") {
        Ok(origin) if !origin.is_empty() => {
            let origin = HeaderValue::from_str(&origin).context("Invalid CORS_ORIGIN")?;
            Ok(layer.allow_origin(origin))
        }
        _ => Ok(layer),
    }
}
