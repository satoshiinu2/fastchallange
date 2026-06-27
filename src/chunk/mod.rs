use log::info;
use std::{
    ops::{Add, Sub},
    sync::mpsc,
    thread,
};

use glam::{DVec3, I64Vec3};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk::{
    entry::ChunkEntry,
    generate::{ChunkMeshData, generate_height_map},
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
    const MAX_REMOVALS_PER_FRAME: usize = 8;

    pub fn new() -> Self {
        let (req_tx, req_rx) = mpsc::channel::<(SnappedChunkPos, usize)>();
        let (res_tx, res_rx) = mpsc::channel::<ChunkMeshData>();

        // ワーカースレッド
        thread::spawn(move || {
            while let Ok((pos, lod)) = req_rx.recv() {
                let height_map = generate_height_map(pos, lod); // 重い処理
                if res_tx.send(height_map).is_err() {
                    break;
                }
            }
        });

        Self {
            entries: FxHashMap::default(),
            last_updated_pos: None,
            radius: 10,
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

        let dist2: ChunkPos = (pos.bit_and(!1)) - (idk_pos.bit_and(!1)); // 0001
        if dist2.len_sq() >= 8 * 8 {
            lod_level += 1;
        }

        let dist4: ChunkPos = (pos.bit_and(!3)) - (idk_pos.bit_and(!3)); // 0011
        if dist4.len_sq() >= 16 * 16 {
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
        if let Some(entry) = self.entries.remove(&pos) {
            // self.chunk_mesh_update_queue.remove(entry);
            // self.chunk_recreate_queue.remove((pos, entry.lod_level));
            return true;
        }

        return false;
    }

    pub fn update_position(&mut self, pos: DVec3) {
        let c = (pos / Self::SIZE as f64).as_i64vec3();
        let new_pos = ChunkPos::new(c.x, c.z);

        match self.last_updated_pos {
            Some(last) => {
                let delta = new_pos - last;
                if delta.x == 0 && delta.z == 0 {
                    return;
                }

                info!("Chunk position updated: {:?} -> {:?}", last, new_pos);
                self.last_updated_pos = Some(new_pos);

                if delta.x.abs() > self.radius || delta.z.abs() > self.radius {
                    info!("Jump detected, full rebuild.");
                    self.rebuild_full(new_pos);
                } else {
                    self.add_incoming_chunks(last, new_pos);
                }
            }
            None => {
                info!("Initial chunk position: {:?}", new_pos);
                self.last_updated_pos = Some(new_pos);
                self.rebuild_full(new_pos);
            }
        }

        // 範囲外に出たエントリを削除キューへ
        let r = self.radius;
        let out_of_range: Vec<_> = self
            .entries
            .keys()
            .filter(|k| {
                let d = k.0 - new_pos;
                d.x.abs() > r || d.z.abs() > r
            })
            .cloned()
            .collect();

        if !out_of_range.is_empty() {
            info!("Queuing {} chunks for removal.", out_of_range.len());
        }
        self.removal_queue.extend(out_of_range);

        // 削除を少しずつ処理
        let drain_count = self.removal_queue.len().min(Self::MAX_REMOVALS_PER_FRAME);
        for key in self.removal_queue.drain(..drain_count).collect::<Vec<_>>() {
            self.remove_render_chunk(key);
        }
    }

    fn add_incoming_chunks(&mut self, old: ChunkPos, new: ChunkPos) {
        let r = self.radius;
        let dx = new.x - old.x;
        let dz = new.z - old.z;

        // X方向の新しい列
        if dx != 0 {
            let x = if dx > 0 { new.x + r } else { new.x - r };
            for dz2 in -r..=r {
                self.try_add_chunk(ChunkPos::new(x, new.z + dz2));
            }
        }

        // Z方向の新しい列（コーナーはX側で既にカバー済みだが contains_key で弾ける）
        if dz != 0 {
            let z = if dz > 0 { new.z + r } else { new.z - r };
            for dx2 in -r..=r {
                self.try_add_chunk(ChunkPos::new(new.x + dx2, z));
            }
        }
    }

    fn try_add_chunk(&mut self, candidate: ChunkPos) {
        let (snapped, lod_level, should_gen) = self.get_snapped_xzpos(candidate);
        if !should_gen {
            return;
        }
        let key = SnappedChunkPos(snapped);
        match self.entries.get(&key) {
            None => {
                self.recreate_queue.push((key, lod_level));
            }
            Some(entry) if entry.lod_level != lod_level => {
                // LOD変化 → 再生成
                self.recreate_queue.push((key, lod_level));
            }
            _ => {}
        }
    }

    fn rebuild_full(&mut self, center: ChunkPos) {
        let r = self.radius;

        // 範囲外を削除キューへ
        let out_of_range: Vec<_> = self
            .entries
            .keys()
            .filter(|k| {
                let d = k.0 - center;
                d.x.abs() > r || d.z.abs() > r
            })
            .cloned()
            .collect();
        self.removal_queue.extend(out_of_range);

        // 全候補をスキャン
        for dx in -r..=r {
            for dz in -r..=r {
                self.try_add_chunk(center + ChunkPos::new(dx, dz));
            }
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
