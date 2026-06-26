use std::borrow::Cow;

use glam::Mat4;
use wgpu::{
    Device, RenderPass, ShaderModule, ShaderModuleDescriptor, ShaderSource, Texture, TextureView,
};

use crate::{
    key::KeyBindings,
    render::{buffer::GenericRenderBuffer, camera::Camera, vertex::VertexLayout},
};

pub mod buffer;
pub mod camera;
pub mod vertex;
pub mod window;

pub struct Renderer {
    aspect_ratio: f32,
    pub(crate) depth_texture_view: Option<TextureView>,
    depth_texture: Option<Texture>,
    pub(crate) camera: Camera,
    shader: ShaderModule,
}

impl Renderer {
    pub fn new(gpu_state: &GpuState) -> Self {
        let shader = Self::create_shader_module(
            &gpu_state.device,
            include_str!("../shader/terrain.wgsl"),
            Some("Terrain Shader"),
        );

        Self {
            aspect_ratio: 1.0,
            depth_texture_view: None,
            depth_texture: None,
            camera: Camera::new(),
            shader,
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

    pub fn render(&self, gpu_state: &GpuState) {
        match gpu_state.surface.get_current_texture() {
            Ok(output) => {
                let surface_view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                // Clone the depth_texture_view to avoid a simultaneous mutable and immutable borrow
                // of renderer. The TextureView is cheap to clone as it contains Arcs.
                let depth_texture_view_owned = self
                    .depth_texture_view
                    .as_ref()
                    .expect("Depth texture view not initialized")
                    .clone();

                let command_buffer =
                    self.render_inner(gpu_state, &surface_view, &depth_texture_view_owned);

                gpu_state.queue.submit(std::iter::once(command_buffer));
                output.present();
            }
            Err(e) => log::error!("Surface error: {:?}", e),
        }
    }

    fn render_inner(
        &self,
        gpu_state: &GpuState,
        surface_view: &wgpu::TextureView,
        depth_texture_view: &wgpu::TextureView,
    ) -> wgpu::CommandBuffer {
        let mut encoder =
            gpu_state
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Main Render Encoder"),
                });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Main Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
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

        let p_matrix =
            Mat4::perspective_infinite_lh(self.camera.fov.to_radians(), self.aspect_ratio, 0.1);
        let v_matrix = self.camera.get_v_matrix();

        let vp_matrix = p_matrix * v_matrix;

        drop(render_pass);

        encoder.finish()
    }

    fn issue_draw<'pass, V: VertexLayout, const I: usize>(
        render_pass: &mut RenderPass<'pass>,
        buffer: &'pass GenericRenderBuffer<V>,
        bind_groups: [wgpu::BindGroup; I],
    ) {
        for (i, bg) in bind_groups.iter().enumerate() {
            render_pass.set_bind_group(i as u32, bg, &[]);
        }

        buffer.bind_to_render_pass(render_pass);
        render_pass.draw_indexed(0..buffer.index_count(), 0, 0..1);
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
