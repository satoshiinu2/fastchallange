use glam::Vec4;
use log::info;
use wgpu::util::DeviceExt;

use crate::chunk::ChunkManager;

pub struct TerrainPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub vp_buffer: wgpu::Buffer,    // 共有 VP matrix uniform
    pub index_buffer: wgpu::Buffer, // 全チャンク共有、16x16 グリッドの indices
    pub index_count: u32,

    pub global_chunks_buffer: wgpu::Buffer,
    pub global_bind_group: wgpu::BindGroup,
}

impl TerrainPipeline {
    pub const MAX_CHUNKS_PER_DRAW: usize = 2048;

    pub fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        surface_format: wgpu::TextureFormat,
        max_chunks: usize,
    ) -> Self {
        info!("Max chunks updated to: {}", max_chunks);

        // 1. 最初から、新しい一括描画用（Bindingが2つ）のレイアウトを作成する
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Terrain BGL"),
            entries: &[
                // @binding(0) var<uniform> all_chunks: array<ChunkData, 48>;
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // @binding(1) var<uniform> vp_matrix: mat4x4<f32>;
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // 2. この正しいレイアウトを使ってパイプラインレイアウトを作成
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Terrain Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Terrain Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // インデックスバッファなどの生成処理
        let indices = Self::build_grid_indices(ChunkManager::MESH_SIZE as u32);
        let index_count = indices.len() as u32;
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Terrain Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let vp_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VP Matrix Uniform"),
            size: std::mem::size_of::<glam::Mat4>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let global_chunks_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Global Chunks Uniform Buffer"),
            size: (std::mem::size_of::<GpuChunkData>() * max_chunks) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 最初で作った bind_group_layout をそのまま使ってバンドグループを生成
        let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Global Terrain BindGroup"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: global_chunks_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: vp_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            pipeline,
            vp_buffer,
            index_buffer,
            index_count,
            global_chunks_buffer,
            global_bind_group,
        }
    }

    pub fn build_grid_indices(n: u32) -> Vec<u32> {
        let mut indices = Vec::with_capacity(((n - 1) * (n - 1) * 6) as usize);
        for z in 0..n - 1 {
            for x in 0..n - 1 {
                let tl = z * n + x;
                let tr = tl + 1;
                let bl = tl + n;
                let br = bl + 1;
                // 2 triangles
                indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
            }
        }
        indices
    }

    pub fn update_vp(&self, queue: &wgpu::Queue, vp: &glam::Mat4) {
        queue.write_buffer(
            &self.vp_buffer,
            0,
            bytemuck::cast_slice(&vp.to_cols_array()),
        );
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuHeightMap {
    pub data: [f32; 292],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuShaowMap {
    pub data: [u32; 73],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuChunkData {
    pub rel_pos: Vec4,            // 16バイト アライメントのためVec4
    pub lod_level: u32,           // 4バイト
    pub _padding: [u32; 3],       // 12バイトのパディング
    pub height_map: GpuHeightMap, // 292 * 16 = 1168バイト
    pub shadow_map: GpuShaowMap,  // 73 * 16 = 1168バイト
    pub _padding2: [u32; 3],      // 12バイトのパディング
}
