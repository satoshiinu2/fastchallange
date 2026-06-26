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
}
impl GlobalState {
    fn new(window: Arc<Window>, gpu_state: GpuState) -> Self {
        let renderer = Renderer::new(&gpu_state);
        let chunk_manager = ChunkManager::new();
        let key_bindings = KeyBindings::new();

        Self {
            window,
            gpu_state,
            renderer,
            chunk_manager,
            key_bindings,
            last_frame_time: Instant::now(),
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f64(); // 秒単位のt
        self.last_frame_time = now;

        self.renderer.physics_update(&self.key_bindings, dt);

        self.chunk_manager
            .update_position(self.renderer.camera.position);

        self.chunk_manager.flush_queues();

        self.renderer.render(&self.gpu_state);
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
    }

    fn key_up(&mut self, key: winit::keyboard::PhysicalKey) {
        self.key_bindings.on_key_change::<false>(key);
    }
}

fn main() {
    run_client();
}
