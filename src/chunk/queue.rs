use crate::
    chunk::{ChunkManager, entry::ChunkEntry}
;
use rayon::prelude::*;

impl ChunkManager {
    pub fn flush_queues(&mut self) {
        let generator = &self.generator;

        let results: Vec<_> = self
            .recreate_queue
            .drain(..)
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|(pos, lod)| generator.generate_chunk(pos, lod))
            .collect();

        for mesh_data in results {
            let entry = ChunkEntry::new(
                mesh_data.position,
                mesh_data.lod_level,
                mesh_data.height_map,
                mesh_data.shadow_map,
            );
            self.entries.insert(entry.position, entry);
        }
    }
}
