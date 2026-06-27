use fastnoise2::{
    SafeNode,
    generator::{Generator, GeneratorWrapper, simplex::simplex},
};

use crate::chunk::{ChunkManager, ChunkPos, SnappedChunkPos, entry::HEIGHT_MAP_SIZE};

pub struct ChunkMeshData {
    pub position: SnappedChunkPos,
    pub lod_level: usize,
    pub height_map: [f32; HEIGHT_MAP_SIZE],
    pub shadow_map: [f32; HEIGHT_MAP_SIZE],
}

pub struct ChunkGenerator {
    noise: GeneratorWrapper<SafeNode>,
}

struct MapGenResult {
    height_map: [f32; HEIGHT_MAP_SIZE],
    shadow_map: [f32; HEIGHT_MAP_SIZE],
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
        let MapGenResult {
            height_map,
            shadow_map,
        } = self.generate_height_and_shadow_map(position.0, lod_level);

        ChunkMeshData {
            position,
            lod_level,
            height_map,
            shadow_map,
        }
    }

    fn generate_height_and_shadow_map(&self, position: ChunkPos, lod_level: usize) -> MapGenResult {
        let mut height_map = [0.0; HEIGHT_MAP_SIZE];
        let mut shadow_map = [0.0; HEIGHT_MAP_SIZE];

        // 太陽の光が届く方向ベクトル
        let light_dir = glam::Vec3::new(0.5, 1.0, 0.3).normalize();

        let scale = 1 << lod_level;
        let step = scale as f32; // 隣のマスへの距離

        for i in 0..HEIGHT_MAP_SIZE {
            let local_x = ((i % ChunkManager::MESH_SIZE) * scale) as i64;
            let local_z = ((i / ChunkManager::MESH_SIZE) * scale) as i64;

            let world_x = (position.x * 16 + local_x) as f32;
            let world_z = (position.z * 16 + local_z) as f32;

            // 1. まず中心の通常の高さを取得
            let h_center = self.height(world_x, world_z);
            height_map[i] = h_center;

            // 2. 💥 境界バグを防ぐ肝：直接ノイズ関数から「1マス隣の本物の高さ」を引く
            let h_left = self.height(world_x - step, world_z);
            let h_right = self.height(world_x + step, world_z);
            let h_back = self.height(world_x, world_z - step);
            let h_forward = self.height(world_x, world_z + step);

            // 3. 中心差分を用いて、その地点の法線 (地面の向きベクトル) を計算
            // 左右、前後の傾きから法線ベクトルを作る
            let nx = h_left - h_right;
            let nz = h_back - h_forward;
            let ny = step * 2.0; // X, Zの差分が 2マス分 (step * 2) なので合わせる
            let normal = glam::Vec3::new(nx, ny, nz).normalize();

            // 4. 法線とライト方向の内積 (Dot Product) を計算して、光の強さを 0.0 ~ 1.0 に収める
            // 地面が太陽を向いているほど 1.0 に近づき、逆を向くほど 0.0 (影) になる
            let dot = normal.dot(light_dir);
            let shadow_value = dot.max(0.0); // 0以下（裏側）は真っ黒

            // ほんの少し環境光（ベースの明るさ）を足してあげると、影が真っ黒になりすぎずリアルになります
            let ambient = 0.2;
            shadow_map[i] = (shadow_value * (1.0 - ambient) + ambient).clamp(0.0, 1.0);
        }

        MapGenResult {
            height_map,
            shadow_map,
        }
    }
}
