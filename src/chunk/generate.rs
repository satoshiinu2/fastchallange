use fastnoise2::{
    SafeNode,
    generator::{Generator, GeneratorWrapper, simplex::simplex},
};

use crate::chunk::{ChunkManager, ChunkPos, SnappedChunkPos, entry::HEIGHT_MAP_SIZE};

pub struct ChunkMeshData {
    pub position: SnappedChunkPos,
    pub lod_level: usize,
    pub height_map: [f32; HEIGHT_MAP_SIZE],
}

pub struct ChunkGenerator {
    noise: GeneratorWrapper<SafeNode>,
}

impl ChunkGenerator {
    pub fn new() -> Self {
        let noise = simplex()
            .fbm(
                0.5, // gain
                0.0, // weighted strength
                4,   // octaves
                2.0, // lacunarity
            )
            .build();

        Self { noise }
    }

    pub fn height(&self, x: f32, z: f32) -> f32 {
        let continent = self.noise.gen_single_2d(x * 0.0003, z * 0.0003, 1);

        let detail = self.noise.gen_single_2d(x * 0.003, z * 0.003, 2);

        let mountain = self.noise.gen_single_2d(x * 0.01, z * 0.01, 2);

        let mountain = (1.0 - mountain.abs()).powf(4.0);

        (continent * 120.0 - 120.0) + mountain * 80.0 + detail * 20.0
    }

    pub fn generate_chunk(&self, position: SnappedChunkPos, lod_level: usize) -> ChunkMeshData {
        ChunkMeshData {
            position,
            lod_level,
            height_map: self.generate_height_map(position.0, lod_level),
        }
    }

    pub fn generate_height_map(
        &self,
        position: ChunkPos,
        lod_level: usize,
    ) -> [f32; HEIGHT_MAP_SIZE] {
        let mut heights = [0.0; HEIGHT_MAP_SIZE];

        for i in 0..HEIGHT_MAP_SIZE {
            let scale = 1 << lod_level;
            let x = ((i % ChunkManager::MESH_SIZE) * scale) as i64;
            let z = ((i / ChunkManager::MESH_SIZE) * scale) as i64;

            let world_x = position.x * 16 + x;
            let world_z = position.z * 16 + z;

            heights[i] = self.height(world_x as f32, world_z as f32);
        }

        heights
    }
}
