use crate::chunk::ChunkManager;

impl ChunkManager {
    /// メインループで毎フレーム呼ぶ
    pub fn flush_queues(&mut self) {
        // 1. 削除
        let drain_count = self.removal_queue.len().min(Self::MAX_REMOVALS_PER_FRAME);
        for key in self.removal_queue.drain(..drain_count).collect::<Vec<_>>() {
            self.remove_render_chunk(key);
            self.in_flight.remove(&key);
        }

        // 2. 生成リクエストをスレッドへ投げる
        for (pos, lod) in self.recreate_queue.drain(..) {
            if self.in_flight.insert(pos) {
                // 重複投入防止
                let _ = self.mesh_sender.send((pos, lod));
            }
        }

        // 3. 完成したメッシュを受け取ってGPUへ
        while let Ok(mesh_data) = self.mesh_receiver.try_recv() {
            self.in_flight.remove(&mesh_data.position);
            if self.entries.contains_key(&mesh_data.position) {
                // キャンセルされてたらスキップ
                continue;
            }
            self.create_render_chunk(mesh_data);
            // GPUアップロードはここ
        }
    }
}
