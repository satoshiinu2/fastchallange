use crate::{
    chunk::{ChunkManager, entry::ChunkEntry},
    render::{GpuState, terrain::TerrainPipeline},
};

impl ChunkManager {
    pub fn flush_queues(&mut self, gpu_state: &GpuState, terrain: &TerrainPipeline) {
        // 古いチャンク削除
        let drain_count = self.removal_queue.len().min(Self::MAX_REMOVALS_PER_FRAME);
        for key in self.removal_queue.drain(..drain_count).collect::<Vec<_>>() {
            self.remove_render_chunk(key);
            self.in_flight.remove(&key);
        }

        // 生成リクエストをスレッドへ投げる
        for (pos, lod) in self.recreate_queue.drain(..) {
            if self.in_flight.insert(pos) {
                // 重複投入防止
                let _ = self.mesh_sender.send((pos, lod));
            }
        }

        // 完成したメッシュを受け取ってGPUへ
        while let Ok(mesh_data) = self.mesh_receiver.try_recv() {
            self.in_flight.remove(&mesh_data.position);

            // removal_queue に積まれていたらキャンセル扱い
            if self.removal_queue.contains(&mesh_data.position) {
                continue;
            }

            let entry = ChunkEntry::new(
                mesh_data.position,
                mesh_data.lod_level,
                mesh_data.height_map,
                gpu_state,
                &terrain.bind_group_layout,
                &terrain.vp_buffer,
            );

            self.create_render_chunk(entry);
            // GPUアップロードはここ
        }
    }
}
