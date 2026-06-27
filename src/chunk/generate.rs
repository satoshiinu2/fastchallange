use crate::chunk::{SnappedChunkPos, entry::HEIGHT_MAP_SIZE};

pub struct ChunkMeshData {
    pub position: SnappedChunkPos,
    pub lod_level: usize,
    pub height_map: [f32; HEIGHT_MAP_SIZE],
}

pub fn generate_height_map(position: SnappedChunkPos, lod_level: usize) -> ChunkMeshData {
    ChunkMeshData {
        position,
        lod_level,
        height_map: [0.0; HEIGHT_MAP_SIZE], // TODO: noise gen
    }
}
