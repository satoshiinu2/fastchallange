use crate::chunk::{ChunkManager, SnappedChunkPos};

const HEIGHT_MAP_SIZE: usize = ChunkManager::SIZE * ChunkManager::SIZE;

pub struct ChunkEntry {
    pub position: SnappedChunkPos,
    pub lod_level: usize,
    pub height_map: [f32; HEIGHT_MAP_SIZE],
}
impl ChunkEntry {
    pub(crate) fn new(position: SnappedChunkPos, lod_level: usize) -> Self {
        Self {
            position,
            lod_level,
            height_map: [0.0; HEIGHT_MAP_SIZE],
        }
    }
}
