use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::{ChunkInfos, Room};
use crate::storage::BunnyStorage;

/// How long a room lives after it is first created.
const ROOM_TTL_SECS: u64 = 3600;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Path-safe, deterministic encoding of a file name so it can be used as an
/// object key. The authoritative file name lives inside the manifest body, so
/// this only needs to be unique per file and free of path separators.
fn hex(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for byte in input.bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn marker_key(room_id: &str) -> String {
    format!("rooms/{room_id}/.created")
}

fn manifest_key(room_id: &str, file_name: &str) -> String {
    format!("rooms/{room_id}/{}.json", hex(file_name))
}

/// Create a room by writing an expiry marker. Idempotent: an existing marker is
/// left untouched so the original expiry (and thus the TTL) is preserved.
pub async fn create_room(storage: &BunnyStorage, room_id: &str) {
    let key = marker_key(room_id);

    match storage.get(&key).await {
        Ok(Some(_)) => {} // already created — keep the original expiry
        Ok(None) => {
            let expires_at = now_secs() + ROOM_TTL_SECS;
            if let Err(e) = storage.put(&key, expires_at.to_string().into_bytes()).await {
                logs::warn!("Failed to create room marker {}: {}", key, e);
            }
        }
        Err(e) => logs::warn!("Failed to read room marker {}: {}", key, e),
    }
}

/// Attach a file (a set of ordered chunk ids) to a room by storing its manifest.
pub async fn add_chunk_to_room(storage: &BunnyStorage, room_id: &str, chunk_info: ChunkInfos) {
    create_room(storage, room_id).await;

    let key = manifest_key(room_id, &chunk_info.file_name);
    match serde_json::to_vec(&chunk_info) {
        Ok(body) => {
            if let Err(e) = storage.put(&key, body).await {
                logs::warn!("Failed to write manifest {}: {}", key, e);
            }
        }
        Err(e) => logs::error!("Failed to serialize manifest for {}: {}", key, e),
    }
}

/// Read a room's content by listing its directory and parsing every manifest.
/// Files are returned in a stable (file-name) order.
pub async fn get_room_content(storage: &BunnyStorage, room_id: &str) -> Room {
    let entries = storage
        .list(&format!("rooms/{room_id}"))
        .await
        .unwrap_or_default();

    let mut file_map: BTreeMap<String, ChunkInfos> = BTreeMap::new();
    for entry in entries {
        if entry.is_directory || !entry.object_name.ends_with(".json") {
            continue;
        }

        let key = format!("rooms/{room_id}/{}", entry.object_name);
        match storage.get(&key).await {
            Ok(Some(body)) => match serde_json::from_slice::<ChunkInfos>(&body) {
                Ok(info) => {
                    file_map.insert(info.file_name.clone(), info);
                }
                Err(e) => logs::warn!("Failed to parse manifest {}: {}", key, e),
            },
            Ok(None) => {}
            Err(e) => logs::warn!("Failed to read manifest {}: {}", key, e),
        }
    }

    Room {
        chunks_infos: file_map.into_values().collect(),
    }
}

/// Delete every room whose expiry marker is in the past, along with its chunk
/// blobs and manifests. Returns the number of rooms purged.
pub async fn purge_expired_rooms(storage: &BunnyStorage) -> Result<u64, reqwest::Error> {
    let now = now_secs();
    let rooms = storage.list("rooms").await?;
    let mut purged = 0u64;

    for room in rooms {
        if !room.is_directory {
            continue;
        }
        let room_id = &room.object_name;

        let expired = match storage.get(&marker_key(room_id)).await? {
            Some(body) => String::from_utf8_lossy(&body)
                .trim()
                .parse::<u64>()
                .map(|expires_at| expires_at < now)
                .unwrap_or(false),
            None => false,
        };
        if !expired {
            continue;
        }

        purge_room(storage, room_id).await;
        purged += 1;
    }

    Ok(purged)
}

/// Delete all objects belonging to a single room: the chunk blobs referenced by
/// its manifests, then the manifests and the expiry marker. Deletes are
/// best-effort — a failure logs a warning but does not abort the purge.
async fn purge_room(storage: &BunnyStorage, room_id: &str) {
    let entries = storage
        .list(&format!("rooms/{room_id}"))
        .await
        .unwrap_or_default();

    for entry in &entries {
        if entry.is_directory {
            continue;
        }

        let key = format!("rooms/{room_id}/{}", entry.object_name);

        // For manifests, also delete the chunk blobs they reference.
        if entry.object_name.ends_with(".json") {
            if let Ok(Some(body)) = storage.get(&key).await {
                if let Ok(info) = serde_json::from_slice::<ChunkInfos>(&body) {
                    for chunk_id in &info.chunks {
                        if let Err(e) = storage.delete(chunk_id).await {
                            logs::warn!("Failed to delete chunk {} from storage: {}", chunk_id, e);
                        }
                    }
                }
            }
        }

        if let Err(e) = storage.delete(&key).await {
            logs::warn!("Failed to delete object {} from storage: {}", key, e);
        }
    }

    // Remove the now-empty room directory itself; Bunny keeps empty directories
    // in listings otherwise, so they would pile up and be re-scanned forever.
    if let Err(e) = storage.delete(&format!("rooms/{room_id}/")).await {
        logs::warn!("Failed to delete room directory {}: {}", room_id, e);
    }
}
