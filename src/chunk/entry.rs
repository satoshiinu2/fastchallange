use wgpu::util::DeviceExt;

use crate::{
    chunk::{ChunkManager, SnappedChunkPos},
    render::GpuState,
};

pub const HEIGHT_MAP_SIZE: usize = ChunkManager::SIZE * ChunkManager::SIZE;

pub struct ChunkEntry {
    pub position: SnappedChunkPos,
    pub lod_level: usize,
    pub height_map: [f32; HEIGHT_MAP_SIZE],

    // gpu resources
    pub height_map_buffer: wgpu::Buffer,
    pub rel_pos_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}
impl ChunkEntry {
    pub fn new(
        position: SnappedChunkPos,
        lod_level: usize,
        height_map: [f32; HEIGHT_MAP_SIZE],
        gpu_state: &GpuState,
        bind_group_layout: &wgpu::BindGroupLayout,
        vp_buffer: &wgpu::Buffer,
    ) -> Self {
        let device = &gpu_state.device;

        let height_map_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HeightMap Uniform"),
            contents: bytemuck::cast_slice(&height_map),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let pos = position.0;
        let rel_pos_data = [0, 0, 0, 0];
        let rel_pos_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Relative Pos Uniform"),
            contents: bytemuck::cast_slice(&rel_pos_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ChunkEntry BindGroup"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: height_map_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: rel_pos_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: vp_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            position,
            lod_level,
            height_map,
            height_map_buffer,
            rel_pos_buffer,
            bind_group,
        }
    }

    pub fn upload_height_map(&self, queue: &wgpu::Queue) {
        let height_map_vec4: &[[f32; 4]; HEIGHT_MAP_SIZE / 4] =
            bytemuck::cast_ref(&self.height_map);

        queue.write_buffer(
            &self.height_map_buffer,
            0,
            bytemuck::cast_slice(height_map_vec4),
        );
    }
}
