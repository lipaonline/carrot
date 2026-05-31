use serde::{Deserialize, Serialize};

use crate::storage::BunnyStorage;

#[derive(Clone)]
pub struct AppState {
    pub storage: BunnyStorage,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Room {
    pub chunks_infos: Vec<ChunkInfos>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChunkInfos {
    pub file_name: String,
    pub chunks: Vec<String>,
}
