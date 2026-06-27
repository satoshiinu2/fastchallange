use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin_winit::DisplayBuilder;
use log::info;
use winit::application::ApplicationHandler;
use winit::event_loop::ActiveEventLoop;

use std::sync::Arc;

use winit::{
    event::*,
    event_loop::ControlFlow,
    event_loop::EventLoop,
    window::{Window, WindowAttributes, WindowId},
};

use crate::{GlobalState, render::GpuState};

struct App {
    global_state: Option<GlobalState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.global_state.is_some() {
            return;
        }

        let window_attributes = WindowAttributes::default().with_title("coresealed-rs");
        let template = ConfigTemplateBuilder::new();
        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));

        // Glutin の初期化
        let (window, _gl_config) = display_builder
            .build(event_loop, template, |configs| {
                configs
                    .reduce(|accum, config| {
                        if config.num_samples() > accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .unwrap()
            })
            .unwrap();

        let window = Arc::new(window.unwrap());

        let gpu_state = pollster::block_on(create_gpu_state(window.clone()));

        self.global_state = Some(GlobalState::new(window, gpu_state));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(global_state) = self.global_state.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::RedrawRequested => {
                global_state.update();
            }

            WindowEvent::Resized(size) => {
                global_state.resize(size.width, size.height);
            }

            WindowEvent::KeyboardInput { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseWheel { .. } => {
                let response = global_state
                    .egui_state
                    .on_window_event(&global_state.window, &event);
                if !response.consumed {
                    match event {
                        WindowEvent::KeyboardInput { event, .. } => {
                            let key = event.physical_key;
                            match event.state {
                                ElementState::Pressed => global_state.key_down(key),
                                ElementState::Released => global_state.key_up(key),
                            }
                        }
                        WindowEvent::MouseInput { state, button, .. } => {
                            global_state.handle_mouse_input(state, button);
                        }
                        _ => {}
                    }
                }
            }

            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        let Some(global_state) = self.global_state.as_mut() else {
            return;
        };

        if let DeviceEvent::MouseMotion { delta } = event {
            global_state.handle_mouse_motion(delta.0, delta.1);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(global_state) = &self.global_state {
            global_state.window.request_redraw();
        }
    }
}

#[allow(unused)]
pub fn run_client() {
    env_logger::init_from_env(
        env_logger::Env::default().default_filter_or("warn,fastchallange=info"),
    );

    info!("Starting desktop client");

    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App { global_state: None };

    event_loop.run_app(&mut app).unwrap();
}

async fn create_gpu_state(window: Arc<Window>) -> GpuState {
    let size = window.inner_size();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        flags: wgpu::InstanceFlags::default(),
        backend_options: wgpu::BackendOptions::default(),
        display: Default::default(),
        memory_budget_thresholds: Default::default(),
    });

    let surface = instance.create_surface(window.clone()).unwrap();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .expect("could not get adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: Default::default(),
            trace: wgpu::Trace::Off,
            experimental_features: Default::default(),
        })
        .await
        .expect("could not get device");

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(surface_caps.formats[0]);

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };

    surface.configure(&device, &config);

    GpuState {
        surface,
        device,
        queue,
        config,
    }
}
