mod entities;
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
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use state::AppState;
use storage::BunnyStorage;
use tokio::time::interval;

async fn setup_schema(db: &DatabaseConnection) {
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        "CREATE TABLE IF NOT EXISTS rooms (id TEXT PRIMARY KEY, expires_at TIMESTAMPTZ NOT NULL)"
            .to_owned(),
    ))
    .await
    .expect("Failed to create rooms table");

    db.execute(Statement::from_string(
        DbBackend::Postgres,
        "CREATE TABLE IF NOT EXISTS chunks (
            id          TEXT    PRIMARY KEY,
            room_id     TEXT    REFERENCES rooms(id) ON DELETE CASCADE,
            file_name   TEXT,
            chunk_order INTEGER
        )"
        .to_owned(),
    ))
    .await
    .expect("Failed to create chunks table");
}

fn setup_purge_task(db: DatabaseConnection, storage: BunnyStorage) {
    tokio::spawn(async move {
        run_purge_task(db, storage).await;
    });
}

async fn run_purge_task(db: DatabaseConnection, storage: BunnyStorage) {
    let mut ticker = interval(Duration::from_mins(30));

    loop {
        ticker.tick().await;

        match purge_expired_rooms(&db, &storage).await {
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

async fn purge_expired_rooms(
    db: &DatabaseConnection,
    storage: &BunnyStorage,
) -> Result<u64, sea_orm::DbErr> {
    // Snapshot the chunk IDs about to be deleted so we can also remove their
    // blobs from Bunny Storage. Storage deletes are best-effort: a failure
    // logs a warning but does not abort the room purge.
    let chunk_ids = repositories::expired_chunk_ids(db).await?;

    let output = db
        .execute(Statement::from_string(
            DbBackend::Postgres,
            "DELETE FROM rooms WHERE expires_at < NOW()".to_owned(),
        ))
        .await?;

    for chunk_id in &chunk_ids {
        if let Err(e) = storage.delete(chunk_id).await {
            logs::warn!("Failed to delete chunk {} from storage: {}", chunk_id, e);
        }
    }

    Ok(output.rows_affected())
}

#[tokio::main]
async fn main() {
    Logs::new().init();

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/postgres".to_string());

    let db = Database::connect(&db_url)
        .await
        .expect("Failed to connect to database");

    setup_schema(&db).await;

    let storage = BunnyStorage::from_env();

    setup_purge_task(db.clone(), storage.clone());

    let state = AppState { db, storage };

    let app = Router::new()
        .route("/chunks/{id}", post(handlers::save_chunk))
        .route("/chunks/{id}", get(handlers::get_chunk))
        .route("/rooms/{id}", post(handlers::create_room))
        .route("/rooms/{id}", get(handlers::get_room_content))
        .route("/rooms/{id}/chunks", post(handlers::add_chunk_to_room))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
