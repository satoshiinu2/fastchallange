use log::info;

use crate::chunk::{SnappedChunkPos, entry::ChunkEntry};

pub fn generate_height_map(position: SnappedChunkPos, lod_level: usize) -> ChunkEntry {
    info!("generated {:?}", position);
    ChunkEntry::new(position, lod_level)
}
