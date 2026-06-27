use glam::{DVec3, Mat4, Vec3};

use crate::key::KeyBindings;

pub struct Camera {
    pub position: DVec3,
    pub velocity: DVec3,
    pub rotation: Vec3,
    pub fov: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: DVec3::new(0.0, 50.0, 0.0),
            velocity: DVec3::ZERO,
            rotation: Vec3::new(90.0, 0.0, 0.0),
            fov: 72.0,
        }
    }

    pub fn get_v_matrix(&self) -> Mat4 {
        let rotation = self.rotation;

        let mut mat = Mat4::IDENTITY;

        mat *= Mat4::from_rotation_x(-rotation.x.to_radians());
        mat *= Mat4::from_rotation_y(-rotation.y.to_radians());
        mat *= Mat4::from_rotation_z(-rotation.z.to_radians());

        mat
    }

    pub fn physics_update(&mut self, key_bind: &KeyBindings, dt: f64) {
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

        let acceleration_rate = 25.0;

        let acceleration = acceleration.as_dvec3() * acceleration_rate;

        // 加速度を速度に足す (v = v0 + a * t)
        self.velocity += acceleration * dt;

        // 速度を位置に足す (p = p0 + v * t)
        self.position += self.velocity * dt;

        // 操作されていなかったら減速
        if acceleration.length_squared() < f64::EPSILON {
            let friction = 5.0;

            // v = v0 * exp(-f * dt)
            self.velocity *= (-friction * dt).exp();
        }
    }
}
