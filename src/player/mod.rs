use glam::{DVec3, Vec3};

use crate::{chunk::ChunkManager, key::KeyBindings, render::anim::FlightAnimation};

mod collision;

pub struct Player {
    pub position: DVec3,
    pub velocity: DVec3,
    pub rotation: Vec3,

    pub flight_animation: FlightAnimation,
}

impl Player {
    pub fn new() -> Self {
        Self {
            position: DVec3::new(0.0, 50.0, 0.0),
            velocity: DVec3::ZERO,
            rotation: Vec3::ZERO,
            flight_animation: FlightAnimation::new(),
        }
    }

    pub fn physics_update(
        &mut self,
        key_bind: &KeyBindings,
        acceleration_rate: f64,
        dt: f32,
        chunk_manager: &ChunkManager,
        do_collision_check: bool,
    ) -> DVec3 {
        let dt = dt as f64;
        let previous_position = self.position;
        let mut input_vec: Vec3 = Vec3::ZERO;

        if key_bind.right.is_down {
            input_vec.x += 1.0;
        }
        if key_bind.left.is_down {
            input_vec.x -= 1.0;
        }
        if key_bind.rise.is_down {
            input_vec.y += 1.0;
        }
        if key_bind.descent.is_down {
            input_vec.y -= 1.0;
        }
        if key_bind.forward.is_down {
            input_vec.z += 1.0;
        }
        if key_bind.backward.is_down {
            input_vec.z -= 1.0;
        }

        let radians_x = self.rotation.y.to_radians() + std::f32::consts::PI / 2.0;
        let radians_z = self.rotation.y.to_radians();
        let mut acceleration = Vec3::ZERO;
        acceleration.x = f32::sin(radians_z) * input_vec.z + f32::sin(radians_x) * input_vec.x;
        acceleration.y = input_vec.y;
        acceleration.z = f32::cos(radians_z) * input_vec.z + f32::cos(radians_x) * input_vec.x;

        let acceleration = acceleration.as_dvec3() * acceleration_rate;

        // 加速度を速度に足す (v = v0 + a * t)
        self.velocity += acceleration * dt;

        // 速度を位置に足す (p = p0 + v * t)
        self.position += self.velocity * dt;
        if do_collision_check {
            collision::resolve_heightmap_collision(self, chunk_manager, previous_position);
        }

        // 操作されていなかったら減速
        let friction = 5.0;

        // v = v0 * exp(-f * dt)
        let damping = (-friction * dt).exp();

        if acceleration.x.abs() < f64::EPSILON || acceleration.x * self.velocity.x < 0.0 {
            self.velocity.x *= damping;
        }

        if acceleration.y.abs() < f64::EPSILON || acceleration.y * self.velocity.y < 0.0 {
            self.velocity.y *= damping;
        }

        if acceleration.z.abs() < f64::EPSILON || acceleration.z * self.velocity.z < 0.0 {
            self.velocity.z *= damping;
        }

        acceleration
    }
}
