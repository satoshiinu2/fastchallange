use glam::{Vec2, Vec3};
use wgpu::util::DeviceExt;

use crate::render::model::ModelVertex;

pub struct Primitive {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub texture_bind_group: wgpu::BindGroup,
}

pub struct Model {
    pub primitives: Vec<Primitive>,
}

impl Model {
    pub fn load_glb(
        raw_data: &[u8],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> anyhow::Result<Self> {
        let (document, buffers, images) = gltf::import_slice(raw_data)?;

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Model Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // 1x1白テクスチャ(materialなしprimitive用フォールバック)
        let fallback_view = Self::create_texture(device, queue, 1, 1, &[255, 255, 255, 255]);

        let mut primitives = Vec::new();

        for mesh in document.meshes() {
            for prim in mesh.primitives() {
                let reader = prim.reader(|buffer| Some(&buffers[buffer.index()]));

                let positions: Vec<Vec3> = reader
                    .read_positions()
                    .ok_or_else(|| anyhow::anyhow!("no positions"))?
                    .map(Vec3::from)
                    .collect();

                let normals: Vec<Vec3> = reader
                    .read_normals()
                    .map(|it| it.map(Vec3::from).collect())
                    .unwrap_or_else(|| vec![Vec3::Y; positions.len()]);

                let uvs: Vec<Vec2> = reader
                    .read_tex_coords(0)
                    .map(|it| it.into_f32().map(Vec2::from).collect())
                    .unwrap_or_else(|| vec![Vec2::ZERO; positions.len()]);

                let indices: Vec<u32> = reader
                    .read_indices()
                    .ok_or_else(|| anyhow::anyhow!("no indices"))?
                    .into_u32()
                    .collect();

                let vertices: Vec<ModelVertex> = (0..positions.len())
                    .map(|i| ModelVertex {
                        position: positions[i],
                        normal: normals[i],
                        uv: uvs[i],
                    })
                    .collect();

                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Model Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Model Index Buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

                let tex_view = prim
                    .material()
                    .pbr_metallic_roughness()
                    .base_color_texture()
                    .and_then(|info| {
                        let img = &images[info.texture().source().index()];
                        Some(Self::create_texture_from_gltf_image(device, queue, img))
                    })
                    .unwrap_or_else(|| fallback_view.clone());

                let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Model Texture BindGroup"),
                    layout: texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&tex_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                });

                primitives.push(Primitive {
                    vertex_buffer,
                    index_buffer,
                    index_count: indices.len() as u32,
                    texture_bind_group,
                });
            }
        }

        Ok(Self { primitives })
    }

    fn create_texture_from_gltf_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &gltf::image::Data,
    ) -> wgpu::TextureView {
        // gltfクレートはRGB8を返すことがあるのでRGBA8に詰め直す
        let rgba: Vec<u8> = match img.format {
            gltf::image::Format::R8G8B8 => img
                .pixels
                .chunks(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
            gltf::image::Format::R8G8B8A8 => img.pixels.clone(),
            other => panic!("unsupported gltf image format: {other:?}"),
        };
        Self::create_texture(device, queue, img.width, img.height, &rgba)
    }

    fn create_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> wgpu::TextureView {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Model Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }
}

pub struct ModelInstance {
    pub m_buffer: wgpu::Buffer,
    pub m_bind_group: wgpu::BindGroup,
}

impl ModelInstance {
    pub fn new(device: &wgpu::Device, m_matrix_bgl: &wgpu::BindGroupLayout) -> Self {
        let m_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Model M Matrix Uniform"),
            size: std::mem::size_of::<glam::Mat4>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let m_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Model M Matrix BindGroup"),
            layout: m_matrix_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: m_buffer.as_entire_binding(),
            }],
        });

        Self {
            m_buffer,
            m_bind_group,
        }
    }

    pub fn update(&self, queue: &wgpu::Queue, model: &glam::Mat4) {
        queue.write_buffer(
            &self.m_buffer,
            0,
            bytemuck::cast_slice(&model.to_cols_array()),
        );
    }
}
