use std::time::{SystemTime, UNIX_EPOCH};

use fastnoise2::{
    SafeNode,
    generator::{Generator, GeneratorWrapper, simplex::simplex},
};
use glam::Vec3;

use crate::chunk::{ChunkManager, ChunkPos, SnappedChunkPos, entry::HEIGHT_MAP_SIZE};

pub struct ChunkMeshData {
    pub position: SnappedChunkPos,
    pub lod_level: usize,
    pub height_map: [f32; HEIGHT_MAP_SIZE],
    pub shadow_map: [u8; HEIGHT_MAP_SIZE],
}

pub struct ChunkGenerator {
    noise_continent: GeneratorWrapper<SafeNode>,
    noise_detail: GeneratorWrapper<SafeNode>,
    noise_ridge: GeneratorWrapper<SafeNode>,
    noise_mountain: GeneratorWrapper<SafeNode>,
    light_dir: glam::Vec3,
}

struct MapGenResult {
    height_map: [f32; HEIGHT_MAP_SIZE],
    shadow_map: [u8; HEIGHT_MAP_SIZE],
}

impl ChunkGenerator {
    const CONTINENT_FREQ: f32 = 0.0003;
    const DETAIL_FREQ: f32 = 0.003;
    const MASK_FREQ: f32 = 0.15;
    const RIDGE_FREQ: f32 = 0.01;
    const CONTIENT_SCALE: f32 = 120.0;
    const SEA_OFFSET: f32 = -120.0;
    const MOUNT_MAX: f32 = 100.0;
    const DETAIL_POWER: f32 = 20.0;
    const MASK_POW: f32 = 6.0;
    const RIDGE_POW: f32 = 4.0;

    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i32;

        let make_noise = |offset: i32| {
            simplex()
                .fbm(0.5, 0.0, 3, 2.0)
                .seed_offset(seed + offset)
                .build()
        };

        Self {
            noise_continent: make_noise(1),
            noise_detail: make_noise(2),
            noise_ridge: make_noise(2),
            noise_mountain: make_noise(4),
            light_dir: Vec3::new(0.5, 1.0, 0.3).normalize(),
        }
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
        let scale = 1 << lod_level;
        let step = scale as f32;
        let mesh_size = ChunkManager::MESH_SIZE;

        // 座標配列を事前構築（center + 隣4方向 × HEIGHT_MAP_SIZE 点）
        // shadow計算に隣接点が必要なので、5セット分の座標を用意する
        // レイアウト: [center×N, left×N, right×N, back×N, forward×N]
        const N: usize = HEIGHT_MAP_SIZE;
        const TOTAL: usize = N * 5;

        let mut xs = [0.0f32; TOTAL];
        let mut zs = [0.0f32; TOTAL];

        for i in 0..N {
            let local_x = ((i % mesh_size) * scale) as i64;
            let local_z = ((i / mesh_size) * scale) as i64;
            let wx = (position.x * 16 + local_x) as f32;
            let wz = (position.z * 16 + local_z) as f32;

            xs[i] = wx;
            zs[i] = wz; // center
            xs[N + i] = wx - step;
            zs[N + i] = wz; // left
            xs[2 * N + i] = wx + step;
            zs[2 * N + i] = wz; // right
            xs[3 * N + i] = wx;
            zs[3 * N + i] = wz - step; // back
            xs[4 * N + i] = wx;
            zs[4 * N + i] = wz + step; // forward
        }

        // 各レイヤーをバッチ生成
        let scale_xs = |factor: f32| xs.iter().map(|x| x * factor).collect::<Vec<_>>();
        let scale_zs = |factor: f32| zs.iter().map(|z| z * factor).collect::<Vec<_>>();

        let mut continent_out = [0.0f32; TOTAL];
        let mut detail_out = [0.0f32; TOTAL];
        let mut mountain_out = [0.0f32; TOTAL];
        let mut ridge_out = [0.0f32; TOTAL];

        self.noise_continent.gen_position_array_2d(
            &mut continent_out,
            &scale_xs(Self::CONTINENT_FREQ),
            &scale_zs(Self::CONTINENT_FREQ),
            0.0,
            0.0,
            1,
        );
        self.noise_detail.gen_position_array_2d(
            &mut detail_out,
            &scale_xs(Self::DETAIL_FREQ),
            &scale_zs(Self::DETAIL_FREQ),
            0.0,
            0.0,
            2,
        );
        self.noise_mountain.gen_position_array_2d(
            &mut mountain_out,
            &scale_xs(Self::MASK_FREQ),
            &scale_zs(Self::MASK_FREQ),
            0.0,
            0.0,
            3,
        );
        self.noise_ridge.gen_position_array_2d(
            &mut ridge_out,
            &scale_xs(Self::RIDGE_FREQ),
            &scale_zs(Self::RIDGE_FREQ),
            0.0,
            0.0,
            2,
        );

        // --- 3. height合成 & shadow計算 ---
        let mut height_map = [0.0f32; HEIGHT_MAP_SIZE];
        let mut shadow_map = [0u8; HEIGHT_MAP_SIZE];

        for i in 0..N {
            let h = |c: f32, d: f32, mm: f32, mr: f32| -> f32 {
                let mountain_mask = ((mm + 1.0) * 0.5).powf(Self::MASK_POW);
                let mountain_ridge = (1.0 - mr.abs()).powf(Self::RIDGE_POW);
                (c * Self::CONTIENT_SCALE + Self::SEA_OFFSET)
                    + mountain_ridge * mountain_mask * Self::MOUNT_MAX
                    + d * Self::DETAIL_POWER
            };

            let hc = h(
                continent_out[i],
                detail_out[i],
                mountain_out[i],
                ridge_out[i],
            );
            let hl = h(
                continent_out[N + i],
                detail_out[N + i],
                mountain_out[N + i],
                ridge_out[N + i],
            );
            let hr = h(
                continent_out[2 * N + i],
                detail_out[2 * N + i],
                mountain_out[2 * N + i],
                ridge_out[2 * N + i],
            );
            let hb = h(
                continent_out[3 * N + i],
                detail_out[3 * N + i],
                mountain_out[3 * N + i],
                ridge_out[3 * N + i],
            );
            let hf = h(
                continent_out[4 * N + i],
                detail_out[4 * N + i],
                mountain_out[4 * N + i],
                ridge_out[4 * N + i],
            );

            height_map[i] = hc;

            let nx = hl - hr;
            let nz = hb - hf;
            let ny = step * 2.0;

            // dot / |normal| = dot * rsqrt(nx²+ny²+nz²)
            let dot_unnorm = nx * self.light_dir.x + ny * self.light_dir.y + nz * self.light_dir.z;
            let len_sq = nx * nx + ny * ny + nz * nz;
            let dot = dot_unnorm * len_sq.sqrt().recip();
            let shadow_f = (dot.max(0.0) * 0.8 + 0.2).clamp(0.0, 1.0);

            shadow_map[i] = (shadow_f * 255.0) as u8;
        }

        MapGenResult {
            height_map,
            shadow_map,
        }
    }
}
