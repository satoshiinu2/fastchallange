use std::borrow::Cow;

use egui::FullOutput;
use glam::Mat4;
use wgpu::{
    CurrentSurfaceTexture::Success, Device, ShaderModule, ShaderModuleDescriptor, ShaderSource,
    Texture, TextureView,
};

use crate::{
    chunk::ChunkManager,
    key::KeyBindings,
    render::{
        camera::Camera,
        terrain::{GpuChunkData, GpuHeightMap, GpuShaowMap, TerrainPipeline},
    },
};

pub mod camera;
pub mod terrain;
pub mod window;

pub struct Renderer {
    aspect_ratio: f32,
    pub(crate) depth_texture_view: Option<TextureView>,
    depth_texture: Option<Texture>,
    pub(crate) camera: Camera,

    pub terrain_pipeline: TerrainPipeline,

    egui_renderer: egui_wgpu::Renderer,
}

impl Renderer {
    pub fn new(gpu_state: &GpuState) -> Self {
        let shader = Self::create_shader_module(
            &gpu_state.device,
            include_str!("../shader/terrain.wgsl"),
            Some("Terrain Shader"),
        );

        let terrain_pipeline =
            TerrainPipeline::new(&gpu_state.device, &shader, gpu_state.config.format);

        let egui_renderer = egui_wgpu::Renderer::new(
            &gpu_state.device,
            gpu_state.config.format,
            egui_wgpu::RendererOptions {
                ..Default::default()
            },
        );

        Self {
            aspect_ratio: 1.0,
            depth_texture_view: None,
            depth_texture: None,
            camera: Camera::new(),
            terrain_pipeline,
            egui_renderer,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32, device: &wgpu::Device) {
        self.aspect_ratio = width as f32 / height as f32;
        self.create_depth_texture(device, width, height);
    }

    fn create_depth_texture(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.depth_texture_view =
            Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));
        self.depth_texture = Some(texture);
    }

    fn create_shader_module(device: &Device, source: &str, label: Option<&str>) -> ShaderModule {
        device.create_shader_module(ShaderModuleDescriptor {
            label,
            source: ShaderSource::Wgsl(Cow::Borrowed(source)),
        })
    }

    pub fn render(
        &mut self,
        gpu_state: &GpuState,
        chunk_manager: &ChunkManager,
        egui_ctx: &egui::Context,
        egui_state: &mut egui_winit::State,
        window: &winit::window::Window,
        full_output: FullOutput,
    ) {
        egui_state.handle_platform_output(window, full_output.platform_output);

        match gpu_state.surface.get_current_texture() {
            Success(output) => {
                let surface_view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let depth_view = self
                    .depth_texture_view
                    .as_ref()
                    .expect("depth not init")
                    .clone();

                // テクスチャの更新（フォントアトラスなど）
                for (id, delta) in &full_output.textures_delta.set {
                    self.egui_renderer.update_texture(
                        &gpu_state.device,
                        &gpu_state.queue,
                        *id,
                        delta,
                    );
                }

                // クリップされたシェイプをメッシュに変換
                let screen_descriptor = egui_wgpu::ScreenDescriptor {
                    size_in_pixels: [gpu_state.config.width, gpu_state.config.height],
                    pixels_per_point: 1.0,
                };
                let clipped_primitives =
                    egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

                let mut encoder =
                    gpu_state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Main Encoder"),
                        });

                // egui の頂点・インデックスバッファを事前に準備
                self.egui_renderer.update_buffers(
                    &gpu_state.device,
                    &gpu_state.queue,
                    &mut encoder,
                    &clipped_primitives,
                    &screen_descriptor,
                );

                // 地形描画
                self.render_terrain(
                    &gpu_state,
                    &surface_view,
                    &depth_view,
                    chunk_manager,
                    &mut encoder,
                );

                // egui描画パス（地形の後）
                {
                    let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("egui Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &surface_view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load, // Clearではなく既存の上に重ねる
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None, // eguiはデプス不要
                        ..Default::default()
                    });

                    let mut pass = pass.forget_lifetime();

                    self.egui_renderer
                        .render(&mut pass, &clipped_primitives, &screen_descriptor);
                }

                // 不要になったテクスチャを解放
                for id in &full_output.textures_delta.free {
                    self.egui_renderer.free_texture(id);
                }

                gpu_state.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
            _ => log::error!("Surface error"),
        }
    }

    fn render_terrain(
        &self,
        gpu_state: &GpuState,
        surface_view: &wgpu::TextureView,
        depth_texture_view: &wgpu::TextureView,
        chunk_manager: &ChunkManager,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let p = Mat4::perspective_infinite_lh(self.camera.fov.to_radians(), self.aspect_ratio, 0.1);
        let vp = p * self.camera.get_v_matrix();

        self.terrain_pipeline.update_vp(&gpu_state.queue, &vp);

        // 1. 存在する全チャンクのデータを1つの大きなVecに一気に回収する
        let mut chunks_to_draw = Vec::new();
        for entry in chunk_manager.entries.values() {
            if chunks_to_draw.len() >= TerrainPipeline::MAX_CHUNKS_PER_DRAW {
                break;
            }

            let chunk_w_pos = entry.position.0.as_i64vec3() * ChunkManager::SIZE as i64;
            let rel = chunk_w_pos.as_dvec3() - self.camera.position;

            // アラインメントのためにキャストしてコピー１
            let mut padded_heights = [[0.0f32; 4]; 73];
            let flat_src = &entry.height_map[..];
            let flat_dst: &mut [f32] = bytemuck::cast_slice_mut(&mut padded_heights);
            flat_dst[..289].copy_from_slice(flat_src);

            let mut padded_shadows = [[0.0f32; 4]; 73];
            let flat_src = &entry.shadow_map[..];
            let flat_dst: &mut [f32] = bytemuck::cast_slice_mut(&mut padded_shadows);
            flat_dst[..289].copy_from_slice(flat_src);

            chunks_to_draw.push(GpuChunkData {
                rel_pos: rel.as_vec3().extend(0.0),
                lod_level: entry.lod_level as u32,
                _padding: [0; 3],
                height_map: GpuHeightMap {
                    data: padded_heights,
                },
                shadow_map: GpuShaowMap {
                    data: padded_shadows,
                },
            });
        }

        let total_chunks = chunks_to_draw.len() as u32;

        // 2. 集まった全データを「1回だけ」でバッファに一括転送
        if total_chunks > 0 {
            gpu_state.queue.write_buffer(
                &self.terrain_pipeline.global_chunks_buffer,
                0,
                bytemuck::cast_slice(&chunks_to_draw),
            );
        }

        // 3. レンダーパスを開始（元のシンプルな構造に戻す）
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Main Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });

        if total_chunks > 0 {
            pass.set_pipeline(&self.terrain_pipeline.pipeline);
            pass.set_index_buffer(
                self.terrain_pipeline.index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.set_bind_group(0, &self.terrain_pipeline.global_bind_group, &[]);

            // 💥 全体のチャンク数（total_chunks）を指定して「たった1回」描画を呼び出す！
            pass.draw_indexed(0..self.terrain_pipeline.index_count, 0, 0..total_chunks);
        }
        drop(pass);
    }
    pub fn physics_update(&mut self, key_bind: &KeyBindings, dt: f64) {
        self.camera.physics_update(key_bind, dt);
    }
}

pub struct GpuState {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
}
