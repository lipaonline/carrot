mod handlers;
mod repositories;
mod state;
mod storage;

use std::time::Duration;

use axum::{
    Router,
    routing::{get, post},
};
use logs::Logs;
use state::AppState;
use storage::BunnyStorage;
use tokio::time::interval;
use tower_http::cors::{Any, CorsLayer};

fn setup_purge_task(storage: BunnyStorage) {
    tokio::spawn(async move {
        run_purge_task(storage).await;
    });
}

async fn run_purge_task(storage: BunnyStorage) {
    let mut ticker = interval(Duration::from_secs(30 * 60));

    loop {
        ticker.tick().await;

        match repositories::purge_expired_rooms(&storage).await {
            Ok(count) if count > 0 => {
                logs::info!("Purged {} expired rooms", count);
            }
            Err(e) => {
                logs::error!("Cleanup job failed: {}", e);
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() {
    Logs::new().init();

    let storage = BunnyStorage::from_env();

    setup_purge_task(storage.clone());

    let state = AppState { storage };

    // The web client is served from a different origin than the API, so the
    // browser requires CORS headers. The API is stateless (no cookies/auth),
    // so a permissive policy is safe; tighten allow_origin to the web app's
    // domain if you ever want to lock it down.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/chunks/{id}", post(handlers::save_chunk))
        .route("/chunks/{id}", get(handlers::get_chunk))
        .route("/rooms/{id}", post(handlers::create_room))
        .route("/rooms/{id}", get(handlers::get_room_content))
        .route("/rooms/{id}/chunks", post(handlers::add_chunk_to_room))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
