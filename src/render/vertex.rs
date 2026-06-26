use bytemuck::{Pod, Zeroable};
use wgpu::VertexBufferLayout;

use crate::chunk::ChunkManager;

pub trait VertexLayout: bytemuck::Pod + bytemuck::Zeroable + Copy {
    fn layout() -> VertexBufferLayout<'static>;
}

#[macro_export]
macro_rules! impl_vertex_layout {
    (
        $struct_name:ty,
        $( $location:expr => $format:ident , $offset:expr ),* $(,)?
    ) => {
        impl $crate::render::vertex::VertexLayout for $struct_name {
            fn layout() -> wgpu::VertexBufferLayout<'static> {
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<$struct_name>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        $(
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::$format,
                                offset: $offset as wgpu::BufferAddress,
                                shader_location: $location,
                            },
                        )*
                    ],
                }
            }
        }
    };
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TerrainVertex {
    pub position: [u32; 1], // 4bytes offset: 0
}

impl_vertex_layout!(
    TerrainVertex,
    0 => Uint32, 0,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HeightMap {
    pub data: [f32; ChunkManager::SIZE * ChunkManager::SIZE],
}
