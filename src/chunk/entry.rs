use wgpu::util::DeviceExt;

use crate::{
    chunk::{ChunkManager, SnappedChunkPos},
    render::GpuState,
};

pub const HEIGHT_MAP_SIZE: usize = ChunkManager::MESH_SIZE * ChunkManager::MESH_SIZE;

pub struct ChunkEntry {
    pub position: SnappedChunkPos,
    pub lod_level: usize,
    pub height_map: [f32; HEIGHT_MAP_SIZE],
}
impl ChunkEntry {
    pub fn new(
        position: SnappedChunkPos,
        lod_level: usize,
        height_map: [f32; HEIGHT_MAP_SIZE],
    ) -> Self {
        Self {
            position,
            lod_level,
            height_map,
        }
    }
}
