use std::time::{SystemTime, UNIX_EPOCH};

use fastnoise2::{
    SafeNode,
    generator::{Generator, GeneratorWrapper, simplex::simplex},
};

use crate::chunk::{ChunkManager, ChunkPos, SnappedChunkPos, entry::HEIGHT_MAP_SIZE};

pub struct ChunkMeshData {
    pub position: SnappedChunkPos,
    pub lod_level: usize,
    pub height_map: [f32; HEIGHT_MAP_SIZE],
    pub shadow_map: [u8; HEIGHT_MAP_SIZE],
}

pub struct ChunkGenerator {
    noise: GeneratorWrapper<SafeNode>,
}

struct MapGenResult {
    height_map: [f32; HEIGHT_MAP_SIZE],
    shadow_map: [u8; HEIGHT_MAP_SIZE],
}

impl ChunkGenerator {
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i32;

        let noise = simplex()
            .fbm(
                0.5, // gain
                0.0, // weighted strength
                4,   // octaves
                2.0, // lacunarity
            )
            .seed_offset(seed)
            .build();

        Self { noise }
    }

    pub fn height(&self, x: f32, z: f32) -> f32 {
        // 1. 大陸のベース（広大な低地や高地のうねり）
        let continent = self.noise.gen_single_2d(x * 0.0003, z * 0.0003, 1);

        // 2. 地面の細かい凹凸
        let detail = self.noise.gen_single_2d(x * 0.003, z * 0.003, 2);

        // 3. 💥 新設：たまに巨大な山が出現する「エリア」を決めるマスク
        // 周波数を非常に低く（0.0001〜0.0002程度）して、広大なマップの中に「たまに」山脈がくるようにします
        let mountain_mask = self.noise.gen_single_2d(x * 0.15, z * 0.15, 3);
        // ノイズの範囲（通常-1.0〜1.0）を調整し、さらに累乗(powf)することで
        // 「基本は0（平地）で、たまに1.0近く（超高山エリア）になる」ように尖らせます
        let mountain_mask = ((mountain_mask + 1.0) * 0.5).powf(6.0);

        // 4. 山の険しい形状（リッジドノイズ）
        let mountain_raw = self.noise.gen_single_2d(x * 0.01, z * 0.01, 2);
        let mountain_ridge = (1.0 - mountain_raw.abs()).powf(4.0);

        // 5. マスクと山の形状を掛け合わせることで、「特定のエリアだけ、めちゃくちゃ高い山」にする
        // ここでは最大250ブロック級の巨峰がたまに出るように倍率を上げています
        let big_mountain = mountain_ridge * mountain_mask * 400.0;

        // 最終的な高さの合成
        (continent * 120.0 - 120.0) + big_mountain + (detail * 20.0)
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
        let mut shadow_map = [0; HEIGHT_MAP_SIZE];

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
            let shadow_f = (shadow_value * (1.0 - ambient) + ambient).clamp(0.0, 1.0);
            shadow_map[i] = (shadow_f * 255.0) as u8;
        }

        MapGenResult {
            height_map,
            shadow_map,
        }
    }
}
