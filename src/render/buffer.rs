use crate::render::GpuState;

use super::vertex::VertexLayout;
use std::marker::PhantomData;
use wgpu::{Buffer, BufferUsages, IndexFormat, RenderPass};

pub struct GenericRenderBuffer<V> {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,
    vertex_capacity: u64, // bytes
    index_capacity: u64,  // bytes
    _marker: PhantomData<V>,
}

impl<V: VertexLayout + bytemuck::Pod> GenericRenderBuffer<V> {
    /// wgpu はサイズ 0 のバッファを許可しないため、最低 4 バイトを確保する。
    pub fn new(
        gui_state: &GpuState,
        initial_vertex_capacity: u64,
        initial_index_capacity: u64,
    ) -> Self {
        let device = &gui_state.device;
        let vc = initial_vertex_capacity.max(4);
        let ic = initial_index_capacity.max(4);

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GenericRenderBuffer::vertex"),
            size: vc,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GenericRenderBuffer::index"),
            size: ic,
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            vertex_buffer,
            index_buffer,
            index_count: 0,
            vertex_capacity: vc,
            index_capacity: ic,
            _marker: PhantomData,
        }
    }

    pub fn update(&mut self, gpu_state: &GpuState, vertices: &[V], indices: &[u32]) {
        let v_bytes: &[u8] = bytemuck::cast_slice(vertices);
        let i_bytes: &[u8] = bytemuck::cast_slice(indices);

        // 頂点バッファが不足なら再作成
        if v_bytes.len() as u64 > self.vertex_capacity {
            let new_cap = (v_bytes.len() as u64).next_power_of_two();
            self.vertex_buffer = gpu_state.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("GenericRenderBuffer::vertex"),
                size: new_cap,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertex_capacity = new_cap;
        }

        // インデックスバッファが不足なら再作成
        if i_bytes.len() as u64 > self.index_capacity {
            let new_cap = (i_bytes.len() as u64).next_power_of_two();
            self.index_buffer = gpu_state.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("GenericRenderBuffer::index"),
                size: new_cap,
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.index_capacity = new_cap;
        }

        if !v_bytes.is_empty() {
            gpu_state
                .queue
                .write_buffer(&self.vertex_buffer, 0, v_bytes);
        }

        if !i_bytes.is_empty() {
            gpu_state.queue.write_buffer(&self.index_buffer, 0, i_bytes);
        }

        self.index_count = indices.len() as u32;
    }
}

impl<V: VertexLayout> GenericRenderBuffer<V> {
    pub fn bind_to_render_pass<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), self.index_format());
    }

    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    pub fn index_format(&self) -> IndexFormat {
        IndexFormat::Uint32
    }
}
