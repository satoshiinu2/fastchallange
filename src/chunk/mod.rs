use std::ops::{Add, Sub};

use glam::{DVec3, I64Vec3};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk::{entry::ChunkEntry, generate::ChunkGenerator};

mod entry;
mod generate;
mod queue;

pub struct ChunkManager {
    generator: ChunkGenerator,

    pub(crate) entries: FxHashMap<SnappedChunkPos, ChunkEntry>,
    last_updated_pos: Option<ChunkPos>,
    pub radius: i64,
    recreate_queue: FxHashMap<SnappedChunkPos, usize>, // (pos, lod_level)
}

impl ChunkManager {
    pub const SIZE: usize = 16;
    pub const MESH_SIZE: usize = Self::SIZE + 1;

    pub fn new() -> Self {
        Self {
            generator: ChunkGenerator::new(),
            entries: FxHashMap::default(),
            last_updated_pos: None,
            radius: 100,
            recreate_queue: FxHashMap::default(),
        }
    }

    pub fn get_snapped_xzpos(&self, pos: ChunkPos) -> (ChunkPos, usize, bool) {
        let mut lod_level = 0;

        let idk_pos = self.last_updated_pos.unwrap_or(ChunkPos::ZERO);

        const BIT_MASK2: i64 = !0b0001;
        let dist2: ChunkPos = (pos.bit_and(BIT_MASK2)) - (idk_pos.bit_and(BIT_MASK2));
        if dist2.len_sq() >= 8 * 8 {
            lod_level += 1;
        }

        const BIT_MASK4: i64 = !0b0011;
        let dist4: ChunkPos = (pos.bit_and(BIT_MASK4)) - (idk_pos.bit_and(BIT_MASK4));
        if dist4.len_sq() >= 16 * 16 {
            lod_level += 1;
        }

        const BIT_MASK8: i64 = !0b0111;
        let dist8: ChunkPos = (pos.bit_and(BIT_MASK8)) - (idk_pos.bit_and(BIT_MASK8));
        if dist8.len_sq() >= 32 * 32 {
            lod_level += 1;
        }

        const BIT_MASK16: i64 = !0b1111;
        let dist16: ChunkPos = (pos.bit_and(BIT_MASK16)) - (idk_pos.bit_and(BIT_MASK16));
        if dist16.len_sq() >= 64 * 64 {
            lod_level += 1;
        }

        let mask: i64 = !((1i64 << lod_level) - 1);
        let final_snapped: ChunkPos = pos.bit_and(mask);

        let should_gen = pos == final_snapped;

        return (final_snapped, lod_level, should_gen);
    }

    pub fn update_position(&mut self, pos: DVec3) {
        let c = (pos / Self::SIZE as f64).as_i64vec3();
        let new_pos = ChunkPos::new(c.x, c.z);

        if let Some(last) = self.last_updated_pos {
            let delta = new_pos - last;
            if delta.x == 0 && delta.z == 0 {
                return;
            }

            #[cfg(feature = "log_chunk_update")]
            info!("Chunk position updated: {:?} -> {:?}", last, new_pos);
        }

        self.last_updated_pos = Some(new_pos);
        self.rebuild_full();
    }

    pub fn sample_terrain_height(&self, world_x: f64, world_z: f64) -> Option<f64> {
        let chunk_x = (world_x / Self::SIZE as f64).floor() as i64;
        let chunk_z = (world_z / Self::SIZE as f64).floor() as i64;
        let point = ChunkPos::new(chunk_x, chunk_z);

        for lod_level in 0..=5 {
            let mask = !((1i64 << lod_level) - 1);
            let snapped = point.bit_and(mask);
            let key = SnappedChunkPos(snapped);
            if let Some(entry) = self.entries.get(&key) {
                if let Some(height) = self.sample_entry_height(entry, world_x, world_z) {
                    return Some(height);
                }
            }
        }

        None
    }

    fn sample_entry_height(&self, entry: &ChunkEntry, world_x: f64, world_z: f64) -> Option<f64> {
        let scale = 1u32 << entry.lod_level;
        let origin_x = entry.position.0.x as f64 * Self::SIZE as f64;
        let origin_z = entry.position.0.z as f64 * Self::SIZE as f64;
        let local_x = world_x - origin_x;
        let local_z = world_z - origin_z;
        let total_size = Self::SIZE as f64 * scale as f64;

        if local_x < 0.0 || local_x > total_size || local_z < 0.0 || local_z > total_size {
            return None;
        }

        let mesh_size = Self::MESH_SIZE;
        let frac_x = (local_x / scale as f64).clamp(0.0, Self::SIZE as f64);
        let frac_z = (local_z / scale as f64).clamp(0.0, Self::SIZE as f64);
        let x0 = frac_x.floor() as usize;
        let z0 = frac_z.floor() as usize;
        let x0 = x0.min(mesh_size - 2);
        let z0 = z0.min(mesh_size - 2);
        let tx = (frac_x - x0 as f64) as f32;
        let tz = (frac_z - z0 as f64) as f32;

        let i00 = z0 * mesh_size + x0;
        let i10 = i00 + 1;
        let i01 = (z0 + 1) * mesh_size + x0;
        let i11 = i01 + 1;

        let h00 = entry.height_map[i00] as f64;
        let h10 = entry.height_map[i10] as f64;
        let h01 = entry.height_map[i01] as f64;
        let h11 = entry.height_map[i11] as f64;

        let h0 = h00 * (1.0 - tx as f64) + h10 * tx as f64;
        let h1 = h01 * (1.0 - tx as f64) + h11 * tx as f64;

        Some(h0 * (1.0 - tz as f64) + h1 * tz as f64)
    }

    pub fn sample_height_clamped(&self, world_x: f64, world_z: f64) -> f64 {
        self.sample_terrain_height(world_x, world_z)
            .unwrap_or(-1.0e6)
    }

    pub fn rebuild_full(&mut self) {
        let Some(center) = self.last_updated_pos else {
            return;
        };

        let mut needed: FxHashSet<SnappedChunkPos> = FxHashSet::default();

        for dx in -self.radius..=self.radius {
            for dz in -self.radius..=self.radius {
                let candidate = center + ChunkPos::new(dx, dz);
                let (snapped, lod_level, should_gen) = self.get_snapped_xzpos(candidate);
                if !should_gen {
                    continue;
                }
                let key = SnappedChunkPos(snapped);
                if needed.insert(key) {
                    match self.entries.get(&key) {
                        None => {
                            self.recreate_queue.insert(key, lod_level);
                        }
                        Some(e) if e.lod_level != lod_level => {
                            self.recreate_queue.insert(key, lod_level);
                        }
                        _ => {}
                    }
                }
            }
        }

        self.entries.retain(|k, _| needed.contains(k));
    }

    /// 指定した最大半径（距離）までに生成されるチャンク数の概算を返す O(1) の関数
    pub fn estimate_generated_chunks(&self) -> usize {
        let area = |inner: i64, outer: i64| -> i64 {
            let o = (2 * outer + 1).pow(2);
            let i = if inner > 0 { (2 * inner + 1).pow(2) } else { 0 };
            o - i
        };

        let r = self.radius;
        let lod0 = area(0, 7.min(r)) / 1;
        let lod1 = area(8, 15.min(r).max(7)) / 4;
        let lod2 = area(16, 31.min(r).max(15)) / 16;
        let lod3 = area(32, 63.min(r).max(31)) / 64;
        let lod4 = area(64, r.max(63)) / 256;

        (lod0 + lod1 + lod2 + lod3 + lod4).max(0) as usize
    }
}

impl Default for ChunkManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkPos {
    pub x: i64,
    pub z: i64,
}
impl ChunkPos {
    pub const ZERO: ChunkPos = ChunkPos { x: 0, z: 0 };

    pub fn new(x: i64, z: i64) -> Self {
        Self { x, z }
    }

    pub fn bit_and(&self, v: i64) -> Self {
        Self {
            x: self.x & v,
            z: self.z & v,
        }
    }

    pub fn len_sq(&self) -> i64 {
        self.x * self.x + self.z * self.z
    }

    pub fn as_i64vec3(&self) -> I64Vec3 {
        I64Vec3 {
            x: self.x,
            y: 0,
            z: self.z,
        }
    }
}

impl Add for ChunkPos {
    type Output = ChunkPos;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for ChunkPos {
    type Output = ChunkPos;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            z: self.z - rhs.z,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SnappedChunkPos(pub(crate) ChunkPos);
