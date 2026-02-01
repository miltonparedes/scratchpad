mod db;
mod handlers;
mod models;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use db::Database;

pub struct AppState {
    pub db: Database,
    pub tx: broadcast::Sender<String>,
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

    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "scratchpad-server.db".to_string());
    let db = Database::open(&db_path)?;
    db.init()?;

    let (tx, _rx) = broadcast::channel::<String>(100);

    let state = Arc::new(AppState { db, tx });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/api/ops", post(handlers::push_ops))
        .route("/api/ops/{workspace_id}", get(handlers::get_ops))
        .route("/api/snapshot/{workspace_id}", get(handlers::get_snapshot))
        .route("/api/snapshot/{workspace_id}", post(handlers::save_snapshot))
        .route("/ws", get(handlers::websocket_handler))
        .layer(cors)
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
