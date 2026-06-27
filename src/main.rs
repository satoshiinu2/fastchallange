use std::{sync::Arc, time::Instant};

use winit::window::Window;

use crate::{
    chunk::ChunkManager,
    key::KeyBindings,
    render::{GpuState, Renderer, window::run_client},
};

mod chunk;
mod key;
mod render;

pub struct GlobalState {
    window: Arc<Window>,
    gpu_state: GpuState,
    renderer: Renderer,
    chunk_manager: ChunkManager,
    key_bindings: KeyBindings,
    last_frame_time: Instant,
    cursor_locked: bool,

    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
}
impl GlobalState {
    fn new(window: Arc<Window>, gpu_state: GpuState) -> Self {
        let renderer = Renderer::new(&gpu_state);
        let chunk_manager = ChunkManager::new();
        let key_bindings = KeyBindings::new();

        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            None, 
            None, 
            None, 
        );

        Self {
            window,
            gpu_state,
            renderer,
            chunk_manager,
            key_bindings,
            last_frame_time: Instant::now(),
            cursor_locked: false,
            egui_ctx,
            egui_state,
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f64(); // 秒単位のt
        self.last_frame_time = now;

        self.renderer.physics_update(&self.key_bindings, dt);

        self.chunk_manager
            .update_position(self.renderer.camera.position);

        self.chunk_manager
            .flush_queues();

        let raw_input = self.egui_state.take_egui_input(&self.window);

        let full_output = self.egui_ctx.run_ui(raw_input, |ctx| {
            egui::Window::new("Perf").show(ctx, |ui| {
                ui.label(format!("FPS: {:.1}", 1.0 / dt));
            });
        });

        self.renderer.render(
            &self.gpu_state,
            &self.chunk_manager,
            &self.egui_ctx,
            &mut self.egui_state,
            &self.window,
            full_output,
        );
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.gpu_state.config.width = width;
        self.gpu_state.config.height = height;
        self.gpu_state
            .surface
            .configure(&self.gpu_state.device, &self.gpu_state.config);

        self.renderer.resize(width, height, &self.gpu_state.device);
    }

    fn key_down(&mut self, key: winit::keyboard::PhysicalKey) {
        self.key_bindings.on_key_change::<true>(key);

        if let winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape) = key {
            let _ = self
                .window
                .set_cursor_grab(winit::window::CursorGrabMode::None);
            let _ = self.window.set_cursor_visible(true);
            self.cursor_locked = false;
        }
    }

    fn key_up(&mut self, key: winit::keyboard::PhysicalKey) {
        self.key_bindings.on_key_change::<false>(key);
    }

    pub fn handle_mouse_input(
        &mut self,
        state: winit::event::ElementState,
        button: winit::event::MouseButton,
    ) {
        if state == winit::event::ElementState::Pressed && button == winit::event::MouseButton::Left
        {
            let _ = self
                .window
                .set_cursor_grab(winit::window::CursorGrabMode::Confined);
            let _ = self.window.set_cursor_visible(false);
            self.cursor_locked = true;
        }
    }

    pub fn handle_mouse_motion(&mut self, delta_x: f64, delta_y: f64) {
        if self.cursor_locked {
            self.renderer.camera.rotation.y += delta_x as f32 * 0.1;
            self.renderer.camera.rotation.x += delta_y as f32 * 0.1;

            // クランプ処理
            self.renderer.camera.rotation.x = self.renderer.camera.rotation.x.clamp(-89.0, 89.0);
        }
    }
}

fn main() {
    run_client();
}
