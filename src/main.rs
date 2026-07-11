use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Instant,
};

use anyhow::Result;
use winit::{
    event::{ElementState, MouseButton, MouseScrollDelta},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window},
};

use crate::{
    chunk::ChunkManager,
    key::KeyBindings,
    perf::PerformanceManagers,
    player::Player,
    render::{GpuState, Renderer, camera::CameraMode, window::run_client},
};

mod chunk;
mod key;
mod perf;
mod player;
mod render;

pub struct GlobalState {
    window: Arc<Window>,
    gpu_state: GpuState,

    player: Player,
    renderer: Renderer,
    chunk_manager: ChunkManager,
    perf_man: PerformanceManagers,
    key_bindings: KeyBindings,

    last_frame_time: Instant,
    was_changed_render_distance: AtomicU8,

    cursor_locked: bool,
    camera_mode: CameraMode,
    camera_distance: f32,
    acceleration_rate: f64,
    do_collision_check: bool,

    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
}
impl GlobalState {
    const CHUNK_MIN: i64 = 50;
    const CHUNK_MAX: i64 = 300;
    const ACCEL_MIN: f64 = 10.0;
    const ACCEL_MAX: f64 = 300.0;
    const CAM_DIST_MIN: f32 = 1.0;
    const CAM_DIST_MAX: f32 = 100.0;

    fn new(window: Arc<Window>, gpu_state: GpuState) -> Result<Self> {
        let chunk_manager = ChunkManager::new();
        let renderer = Renderer::new(&gpu_state, chunk_manager.estimate_generated_chunks())?;
        let perf_man = PerformanceManagers::new();
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

        Ok(Self {
            window,
            gpu_state,
            player: Player::new(),
            renderer,
            chunk_manager,
            perf_man,
            key_bindings,
            last_frame_time: Instant::now(),
            cursor_locked: false,
            camera_mode: CameraMode::default(),
            camera_distance: 5.0,
            acceleration_rate: 100.0,
            was_changed_render_distance: AtomicU8::new(0),
            do_collision_check: true,
            egui_ctx,
            egui_state,
        })
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32(); // 秒単位のt
        self.last_frame_time = now;

        let acceleration = self.player.physics_update(
            &self.key_bindings,
            self.acceleration_rate,
            dt,
            &self.chunk_manager,
            self.do_collision_check,
        );

        self.player
            .flight_animation
            .update(self.player.velocity, acceleration, dt);

        self.renderer
            .camera
            .update_position(&self.player, self.camera_mode, self.camera_distance);

        self.chunk_manager
            .update_position(self.renderer.camera.position);

        self.chunk_manager.flush_queues(&mut self.perf_man);

        let raw_input = self.egui_state.take_egui_input(&self.window);

        let full_output = self.egui_ctx.run_ui(raw_input, |ctx| {
            egui::Window::new("Info").show(ctx, |ui| {
                ui.label(format!("FPS: {:.1}", 1.0 / dt));
                ui.add_space(8.0);

                ui.label(format!("Position: {:.1}", self.player.position));
                ui.label(format!(
                    "Velocity: {:.1} ({:.1} m/s)",
                    self.player.velocity,
                    self.player.velocity.length()
                ));
                ui.add_space(8.0);

                ui.label(format!(
                    "Chunk count: {:}",
                    self.chunk_manager.entries.len()
                ));
                ui.add_space(8.0);

                ui.label(format!("Render: {:}", self.perf_man.render.formatted()));
                ui.label(format!(
                    "Generation: {:}",
                    self.perf_man.generation.formatted()
                ));

                ui.add(
                    egui::Slider::new(
                        &mut self.acceleration_rate,
                        Self::ACCEL_MIN..=Self::ACCEL_MAX,
                    )
                    .text("Acceleration rate"),
                );

                let r_dis_res = ui.add(
                    egui::Slider::new(
                        &mut self.chunk_manager.radius,
                        Self::CHUNK_MIN..=Self::CHUNK_MAX,
                    )
                    .text("Render distance"),
                );

                if r_dis_res.changed() {
                    self.was_changed_render_distance.store(1, Ordering::Relaxed);
                }

                ui.add(
                    egui::Slider::new(
                        &mut self.camera_distance,
                        Self::CAM_DIST_MIN..=Self::CAM_DIST_MAX,
                    )
                    .text("Camera distance"),
                );

                ui.add(egui::Checkbox::new(
                    &mut self.do_collision_check,
                    "Collision",
                ));
            });
        });

        if self.was_changed_render_distance.swap(0, Ordering::Relaxed) != 0 {
            self.on_changed_render_distance();
        }

        self.renderer.render(
            &mut self.perf_man,
            &self.gpu_state,
            &self.chunk_manager,
            &self.player,
            self.camera_mode,
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

    fn key_down(&mut self, key: PhysicalKey) {
        self.key_bindings.on_key_change::<true>(key);

        if let PhysicalKey::Code(KeyCode::Escape) = key {
            let _ = self.window.set_cursor_grab(CursorGrabMode::None);
            let _ = self.window.set_cursor_visible(true);
            self.cursor_locked = false;
        }

        if let PhysicalKey::Code(KeyCode::F5) = key {
            self.camera_mode.toggle();
        }
    }

    fn key_up(&mut self, key: PhysicalKey) {
        self.key_bindings.on_key_change::<false>(key);
    }

    pub fn handle_mouse_input(&mut self, state: ElementState, button: MouseButton) {
        if state == ElementState::Pressed && button == MouseButton::Left {
            let _ = self.window.set_cursor_grab(CursorGrabMode::Confined);
            let _ = self.window.set_cursor_visible(false);
            self.cursor_locked = true;
        }
    }

    pub fn handle_mouse_motion(&mut self, delta_x: f64, delta_y: f64) {
        if self.cursor_locked {
            self.player.rotation.y += delta_x as f32 * 0.1;
            self.player.rotation.x += delta_y as f32 * 0.1;

            // クランプ処理
            self.player.rotation.x = self.player.rotation.x.clamp(-89.0, 89.0);
        }
    }

    pub fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        if self.cursor_locked && self.camera_mode == CameraMode::ThirdPerson {
            let scroll_amount = match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
            };

            self.camera_distance -= scroll_amount * 0.5;

            self.camera_distance = self
                .camera_distance
                .clamp(Self::CAM_DIST_MIN, Self::CAM_DIST_MAX);
        }
    }

    fn on_changed_render_distance(&mut self) {
        self.chunk_manager.rebuild_full();
        self.renderer.rebuild_terrain_pipeline(
            &self.gpu_state,
            self.chunk_manager.estimate_generated_chunks(),
        )
    }
}

fn main() {
    run_client();
}
