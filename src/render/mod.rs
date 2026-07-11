use std::borrow::Cow;

use anyhow::Result;
use egui::FullOutput;
use glam::Mat4;
use wgpu::{
    CurrentSurfaceTexture::Success, Device, RenderPass, ShaderModule, ShaderModuleDescriptor,
    ShaderSource, Texture, TextureView,
};

use crate::{
    chunk::ChunkManager,
    perf::PerformanceManagers,
    player::Player,
    render::{
        camera::{Camera, CameraMode},
        model::{
            ModelPipeline,
            loader::{Model, ModelInstance},
        },
        terrain::{GpuChunkData, GpuHeightMap, GpuShaowMap, TerrainPipeline},
    },
};

pub mod anim;
pub mod camera;
pub mod model;
pub mod terrain;
pub mod vertex;
pub mod window;

pub struct Renderer {
    pub aspect_ratio: f32,
    pub depth_texture_view: Option<TextureView>,
    pub depth_texture: Option<Texture>,
    pub camera: Camera,

    terrain_pipeline: TerrainPipeline,
    terrain_shader: ShaderModule,
    model_pipeline: ModelPipeline,
    _model_shader: ShaderModule,

    player_model: Model,
    player_model_instance: ModelInstance,

    egui_renderer: egui_wgpu::Renderer,
}

impl Renderer {
    pub fn new(gpu_state: &GpuState, max_chunks: usize) -> Result<Self> {
        let terrain_shader = Self::create_shader_module(
            &gpu_state.device,
            include_str!("../assets/terrain.wgsl"),
            Some("Terrain Shader"),
        );

        let terrain_pipeline = TerrainPipeline::new(
            &gpu_state.device,
            &terrain_shader,
            gpu_state.config.format,
            max_chunks,
        );

        let model_shader = Self::create_shader_module(
            &gpu_state.device,
            include_str!("../assets/model.wgsl"),
            Some("Model Shader"),
        );

        let model_pipeline =
            ModelPipeline::new(&gpu_state.device, &model_shader, gpu_state.config.format);

        let egui_renderer = egui_wgpu::Renderer::new(
            &gpu_state.device,
            gpu_state.config.format,
            egui_wgpu::RendererOptions {
                ..Default::default()
            },
        );

        let player_model = Model::load_glb(
            include_bytes!("../assets/model.vrm"),
            &gpu_state.device,
            &gpu_state.queue,
            &model_pipeline.texture_bgl,
        )?;

        let player_model_instance =
            ModelInstance::new(&gpu_state.device, &model_pipeline.m_matrix_bgl);

        Ok(Self {
            aspect_ratio: 1.0,
            depth_texture_view: None,
            depth_texture: None,
            camera: Camera::new(),
            terrain_pipeline,
            terrain_shader,
            model_pipeline,
            _model_shader: model_shader,
            player_model,
            egui_renderer,
            player_model_instance,
        })
    }

    pub fn rebuild_terrain_pipeline(&mut self, gpu_state: &GpuState, max_chunks: usize) {
        self.terrain_pipeline = TerrainPipeline::new(
            &gpu_state.device,
            &self.terrain_shader,
            gpu_state.config.format,
            max_chunks,
        );
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
        perf_man: &mut PerformanceManagers,
        gpu_state: &GpuState,
        chunk_manager: &ChunkManager,
        player: &Player,
        camera_mode: CameraMode,
        egui_ctx: &egui::Context,
        egui_state: &mut egui_winit::State,
        window: &winit::window::Window,
        full_output: FullOutput,
    ) {
        let mut render_perf_man = perf_man.render.start();

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

                let p = Mat4::perspective_infinite_lh(
                    self.camera.fov.to_radians(),
                    self.aspect_ratio,
                    0.1,
                );
                let vp = p * self.camera.get_v_matrix();

                // 3. レンダーパスを開始（元のシンプルな構造に戻す）
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Main Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface_view,
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
                        view: &depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });

                // 地形描画
                self.render_terrain(&gpu_state, chunk_manager, &mut pass, vp);

                // モデル描画
                if camera_mode != CameraMode::FirstPerson {
                    self.render_models(&gpu_state, player, &mut pass, vp);
                }

                // ここで使わなくなるのでドロップする
                drop(pass);

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

        render_perf_man.end();
    }

    fn render_terrain(
        &self,
        gpu_state: &GpuState,
        chunk_manager: &ChunkManager,
        pass: &mut RenderPass,
        vp_mat: Mat4,
    ) {
        self.terrain_pipeline.update_vp(&gpu_state.queue, &vp_mat);

        // チャンクのデータを集めてまとめる
        let mut chunks_to_draw = Vec::new();
        for entry in chunk_manager.entries.values() {
            if chunks_to_draw.len() >= TerrainPipeline::MAX_CHUNKS_PER_DRAW {
                break;
            }

            let chunk_w_pos = entry.position.0.as_i64vec3() * ChunkManager::SIZE as i64;
            let rel = chunk_w_pos.as_dvec3() - self.camera.position;

            chunks_to_draw.push(GpuChunkData {
                rel_pos: rel.as_vec3().extend(0.0),
                lod_level: entry.lod_level as u32,
                _padding: [0; 3],
                height_map: GpuHeightMap {
                    data: *bytemuck::cast_ref(&entry.height_map),
                },
                shadow_map: GpuShaowMap {
                    data: *bytemuck::cast_ref(&entry.shadow_map),
                },
                _padding2: [0; 3],
            });
        }

        let total_chunks = chunks_to_draw.len() as u32;

        // バッファに一括転送
        if total_chunks > 0 {
            gpu_state.queue.write_buffer(
                &self.terrain_pipeline.global_chunks_buffer,
                0,
                bytemuck::cast_slice(&chunks_to_draw),
            );

            pass.set_pipeline(&self.terrain_pipeline.pipeline);
            pass.set_index_buffer(
                self.terrain_pipeline.index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.set_bind_group(0, &self.terrain_pipeline.global_bind_group, &[]);

            pass.draw_indexed(0..self.terrain_pipeline.index_count, 0, 0..total_chunks);
        }
    }

    fn render_models(
        &self,
        gpu_state: &GpuState,
        player: &Player,
        pass: &mut RenderPass,
        vp_mat: Mat4,
    ) {
        let model_transform = {
            let rotation = player.flight_animation.rotation_quat();
            let mut t = Mat4::IDENTITY;

            t *= Mat4::from_translation((player.position - self.camera.position).as_vec3());
            t *= Mat4::from_quat(rotation);

            t
        };

        self.model_pipeline.update_vp(&gpu_state.queue, &vp_mat);
        self.player_model_instance
            .update(&gpu_state.queue, &model_transform);

        pass.set_pipeline(&self.model_pipeline.pipeline);
        pass.set_bind_group(0, &self.model_pipeline.vp_matrix_bind_group, &[]);
        pass.set_bind_group(1, &self.player_model_instance.m_bind_group, &[]);

        for prim in &self.player_model.primitives {
            pass.set_bind_group(2, &prim.texture_bind_group, &[]);
            pass.set_vertex_buffer(0, prim.vertex_buffer.slice(..));
            pass.set_index_buffer(prim.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..prim.index_count, 0, 0..1);
        }
    }
}

pub struct GpuState {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
}
