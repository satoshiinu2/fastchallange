use glam::{DVec3, Quat, Vec3Swizzles};

pub struct FlightAnimation {
    rotation: Quat,
    last_yaw: f32,
}

impl FlightAnimation {
    const FORWARD_TILT: f32 = 80.0_f32.to_radians();
    const MAX_ROLL: f32 = 35.0_f32.to_radians();

    pub fn new() -> Self {
        Self {
            rotation: Quat::IDENTITY,
            last_yaw: 0.0,
        }
    }

    pub fn update(&mut self, velocity: DVec3, acceleration: DVec3, dt: f32) {
        let horizontal = velocity.xz();
        let h_length = horizontal.length();
        let a_length = acceleration.xz().length();

        let (yaw, pitch, roll) = if h_length < f64::EPSILON {
            (self.last_yaw, 0.0, 0.0)
        } else {
            let yaw = horizontal.x.atan2(horizontal.y) as f32;
            self.last_yaw = yaw;

            let side = (horizontal.x / h_length).clamp(-1.0, 1.0) as f32;

            let roll = -side * Self::MAX_ROLL;

            let fixed_hori = a_length.clamp(0.0, 1.0) as f32;
            let fixed_vert = (-acceleration.y).clamp(-0.5, 0.5) as f32;
            let pitch = (fixed_hori + fixed_hori * fixed_vert) * Self::FORWARD_TILT;

            (yaw, pitch, roll)
        };

        let target = Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, roll);

        let t = (dt * 8.0).clamp(0.0, 1.0);
        self.rotation = self.rotation.slerp(target, t);
    }

    pub fn rotation_quat(&self) -> Quat {
        self.rotation
    }
}
