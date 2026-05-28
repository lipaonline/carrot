use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    repositories,
    state::{AppState, ChunkInfos, Room},
};

pub async fn save_chunk(
    State(state): State<AppState>,
    Path(id): Path<String>,
    bytes: Bytes,
) -> Result<(), StatusCode> {
    let size = bytes.len();
    println!("Received chunk with id: {} and size: {}", id, size);

    if let Err(e) = state.storage.put(&id, bytes.to_vec()).await {
        eprintln!("Failed to upload chunk {} to storage: {}", id, e);
        return Err(StatusCode::BAD_GATEWAY);
    }

    repositories::register_chunk(&state.db, id).await;
    Ok(())
}

pub async fn get_chunk(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Vec<u8>, StatusCode> {
    match state.storage.get(&id).await {
        Ok(Some(data)) => {
            println!("Retrieved chunk with id: {} and size: {}", id, data.len());
            Ok(data)
        }
        Ok(None) => {
            eprintln!("Chunk with id: {} not found", id);
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            eprintln!("Failed to fetch chunk {} from storage: {}", id, e);
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

pub async fn create_room(State(state): State<AppState>, Path(id): Path<String>) {
    repositories::create_room(&state.db, id).await;
}

pub async fn get_room_content(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Room>, StatusCode> {
    println!("Fetching content for room with id: {}", id);
    Ok(Json(repositories::get_room_content(&state.db, &id).await))
}

pub async fn add_chunk_to_room(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(chunk_info): Json<ChunkInfos>,
) {
    println!(
        "Adding chunk with file name: {} to room with id: {}",
        chunk_info.file_name, room_id
    );
    repositories::add_chunk_to_room(&state.db, room_id, chunk_info).await;
}
