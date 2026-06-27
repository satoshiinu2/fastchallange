use log::{info, warn};
use std::{
    ops::{Add, Sub},
    sync::mpsc,
    thread,
};

use glam::{DVec3, I64Vec3};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk::{
    entry::ChunkEntry,
    generate::{ChunkGenerator, ChunkMeshData},
};

mod entry;
mod generate;
mod queue;

pub struct ChunkManager {
    pub(crate) entries: FxHashMap<SnappedChunkPos, ChunkEntry>,
    last_updated_pos: Option<ChunkPos>,
    pub radius: i64,
    removal_queue: Vec<SnappedChunkPos>,
    recreate_queue: Vec<(SnappedChunkPos, usize)>, // (pos, lod_level)

    // 非同期生成用
    mesh_sender: mpsc::Sender<(SnappedChunkPos, usize)>, // スレッドへ
    mesh_receiver: mpsc::Receiver<ChunkMeshData>,        // スレッドから
    in_flight: FxHashSet<SnappedChunkPos>,               // 生成中のpos
}

impl ChunkManager {
    pub const SIZE: usize = 16;
    pub const MESH_SIZE: usize = Self::SIZE + 1;
    const MAX_REMOVALS_PER_FRAME: usize = 100;

    pub fn new() -> Self {
        let (req_tx, req_rx) = mpsc::channel::<(SnappedChunkPos, usize)>();
        let (res_tx, res_rx) = mpsc::channel::<ChunkMeshData>();

        // ワーカースレッド
        thread::spawn(move || {
            let generator = ChunkGenerator::new();
            while let Ok((pos, lod)) = req_rx.recv() {
                let height_map = generator.generate_chunk(pos, lod); // 重い処理
                if let Err(e) = res_tx.send(height_map) {
                    warn!("worker thread error: {}", e);
                    break;
                }
            }
        });

        Self {
            entries: FxHashMap::default(),
            last_updated_pos: None,
            radius: 100,
            removal_queue: Vec::new(),
            recreate_queue: Vec::new(),
            mesh_sender: req_tx,
            mesh_receiver: res_rx,
            in_flight: FxHashSet::default(),
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

    pub fn create_render_chunk(&mut self, entry: ChunkEntry) {
        self.remove_render_chunk(entry.position);
        self.entries.insert(entry.position, entry);
    }

    pub fn remove_render_chunk(&mut self, pos: SnappedChunkPos) -> bool {
        self.entries.remove(&pos).is_some()
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
        self.rebuild_full(new_pos);
    }

    fn rebuild_full(&mut self, center: ChunkPos) {
        let r = self.radius;
        let mut needed: FxHashSet<SnappedChunkPos> = FxHashSet::default();

        for dx in -r..=r {
            for dz in -r..=r {
                let candidate = center + ChunkPos::new(dx, dz);
                let (snapped, lod_level, should_gen) = self.get_snapped_xzpos(candidate);
                if !should_gen {
                    continue;
                }
                let key = SnappedChunkPos(snapped);
                if needed.insert(key) {
                    match self.entries.get(&key) {
                        None => self.recreate_queue.push((key, lod_level)),
                        Some(e) if e.lod_level != lod_level => {
                            self.recreate_queue.push((key, lod_level));
                        }
                        _ => {}
                    }
                }
            }
        }

        // neededに含まれないものを削除
        let to_remove: Vec<_> = self
            .entries
            .keys()
            .filter(|k| !needed.contains(k))
            .cloned()
            .collect();
        for key in to_remove {
            self.remove_render_chunk(key);
            self.in_flight.remove(&key);
        }
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
