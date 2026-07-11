use glam::{DVec3, EulerRot, Mat4, Quat, Vec3};

use crate::player::Player;

#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub enum CameraMode {
    FirstPerson,
    #[default]
    ThirdPerson,
}

impl CameraMode {
    pub fn toggle(&mut self) {
        *self = match self {
            CameraMode::FirstPerson => CameraMode::ThirdPerson,
            CameraMode::ThirdPerson => CameraMode::FirstPerson,
        }
    }
}

pub struct Camera {
    pub position: DVec3,
    pub rotation: Quat,
    pub fov: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: DVec3::ZERO,
            rotation: Quat::IDENTITY,
            fov: 72.0,
        }
    }

    pub fn get_v_matrix(&self) -> Mat4 {
        let mut mat = Mat4::IDENTITY;

        mat *= Mat4::from_quat(self.rotation.conjugate());

        mat
    }

    pub fn update_position(&mut self, player: &Player, mode: CameraMode, distance: f32) {
        match mode {
            CameraMode::FirstPerson => {
                self.position = player.position;

                self.rotation = Self::deg_to_quat(player.rotation);
            }
            CameraMode::ThirdPerson => {
                let rotation = Self::deg_to_quat(player.rotation);

                let offset = rotation * Vec3::Z * distance;

                self.position = player.position - offset.as_dvec3();

                self.rotation = rotation
            }
        }
    }

    fn deg_to_quat(rotation: Vec3) -> Quat {
        Quat::from_euler(
            EulerRot::YXZ,
            rotation.y.to_radians(), // yaw
            rotation.x.to_radians(), // pitch
            rotation.z.to_radians(), // roll
        )
    }
}
