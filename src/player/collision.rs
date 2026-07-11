use glam::DVec3;

use crate::chunk::ChunkManager;

use super::Player;

#[derive(Clone, Copy)]
struct CapsuleCollision {
    radius: f64,
    height: f64,
}

impl CapsuleCollision {
    const fn new(radius: f64, height: f64) -> Self {
        Self { radius, height }
    }
}

const COLLISION_CAPSULE: CapsuleCollision = CapsuleCollision::new(0.35, 1.8);
const GROUND_EPSILON: f64 = 0.02;
const SWEEP_STEP_SIZE: f64 = 0.1;
const MAX_STEP_UP: f64 = 0.2;

// 壁判定用のリングサンプル点(中心は含まない)
const RING_OFFSETS: [(f64, f64); 8] = [
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (0.7071, 0.7071),
    (0.7071, -0.7071),
    (-0.7071, 0.7071),
    (-0.7071, -0.7071),
];

fn ring_offsets_scaled(radius: f64) -> [(f64, f64); 8] {
    RING_OFFSETS.map(|(x, z)| (x * radius, z * radius))
}

fn clamp_to_ground(position: DVec3, terrain_height: f64, capsule: CapsuleCollision) -> DVec3 {
    let mut clamped = position;
    let bottom_y = position.y - capsule.height * 0.5 + capsule.radius;
    let min_bottom_y = terrain_height + capsule.radius + GROUND_EPSILON;
    if bottom_y < min_bottom_y {
        clamped.y += min_bottom_y - bottom_y;
    }
    clamped
}

fn ground_height_at_center(chunk_manager: &ChunkManager, x: f64, z: f64) -> Option<f64> {
    chunk_manager.sample_terrain_height(x, z)
}

fn horizontal_push_out(
    chunk_manager: &ChunkManager,
    center: DVec3,
    capsule: CapsuleCollision,
) -> DVec3 {
    let mut push = DVec3::ZERO;
    let bottom_y = center.y - capsule.height * 0.5 + capsule.radius;

    for (ox, oz) in ring_offsets_scaled(capsule.radius) {
        let sx = center.x + ox;
        let sz = center.z + oz;
        if let Some(h) = chunk_manager.sample_terrain_height(sx, sz) {
            // その点の地形が自分の胴体の高さより高い = 壁
            if h > bottom_y + GROUND_EPSILON {
                let dir = DVec3::new(-ox, 0.0, -oz).normalize_or_zero();
                let penetration = h - bottom_y;
                push += dir * penetration.min(capsule.radius);
            }
        }
    }
    push
}
fn resolve_axis_step(
    previous_position: DVec3,
    candidate_position: DVec3,
    chunk_manager: &ChunkManager,
) -> DVec3 {
    if let Some(ground) =
        ground_height_at_center(chunk_manager, candidate_position.x, candidate_position.z)
    {
        let prev_ground =
            ground_height_at_center(chunk_manager, previous_position.x, previous_position.z)
                .unwrap_or(ground);
        let rise = ground - prev_ground;

        if rise > MAX_STEP_UP + GROUND_EPSILON {
            // この軸の移動だけをキャンセル
            return previous_position;
        }
    }
    candidate_position
}

fn resolve_step_position(
    previous_position: DVec3,
    desired_position: DVec3,
    capsule: CapsuleCollision,
    chunk_manager: &ChunkManager,
) -> DVec3 {
    // 軸を単独で解決
    let after_x = resolve_axis_step(
        previous_position,
        DVec3::new(desired_position.x, previous_position.y, previous_position.z),
        chunk_manager,
    );

    let after_z = resolve_axis_step(
        after_x,
        DVec3::new(after_x.x, previous_position.y, desired_position.z),
        chunk_manager,
    );

    let mut candidate = DVec3::new(after_z.x, desired_position.y, after_z.z);

    // 壁への貫入が残っていたら押し出す
    candidate += horizontal_push_out(chunk_manager, candidate, capsule);

    // Y方向を地面へclamp
    if let Some(ground) = ground_height_at_center(chunk_manager, candidate.x, candidate.z) {
        candidate = clamp_to_ground(candidate, ground, capsule);
    } else {
        candidate.y = previous_position.y;
    }

    candidate
}

fn sweep_step_count(distance: f64) -> usize {
    if distance <= f64::EPSILON {
        1
    } else {
        (distance / SWEEP_STEP_SIZE).ceil().max(1.0) as usize
    }
}

pub fn resolve_heightmap_collision(
    player: &mut Player,
    chunk_manager: &ChunkManager,
    previous_position: DVec3,
) {
    let movement = player.position - previous_position;
    let distance = movement.length();
    let steps = sweep_step_count(distance);
    let mut resolved_position = previous_position;

    for step in 1..=steps {
        let t = step as f64 / steps as f64;
        let desired_position = previous_position + movement * t;

        resolved_position = resolve_step_position(
            resolved_position,
            desired_position,
            COLLISION_CAPSULE,
            chunk_manager,
        );
    }

    player.position = resolved_position;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_to_ground_keeps_capsule_above_terrain() {
        let position = DVec3::new(1.0, 0.1, 2.0);
        let clamped = clamp_to_ground(position, 3.0, COLLISION_CAPSULE);
        assert!(clamped.y >= 3.0 + COLLISION_CAPSULE.radius + GROUND_EPSILON);
    }

    #[test]
    fn clamp_to_ground_leaves_already_clear_position_unchanged() {
        let position = DVec3::new(4.0, 5.0, 6.0);
        let clamped = clamp_to_ground(position, 3.0, COLLISION_CAPSULE);
        assert_eq!(clamped.y, position.y);
    }

    #[test]
    fn sweep_step_count_is_at_least_one() {
        assert_eq!(sweep_step_count(0.0), 1);
        assert_eq!(sweep_step_count(0.05), 1);
        assert_eq!(sweep_step_count(0.2), 2);
        assert_eq!(sweep_step_count(1.0), 10);
    }
}
