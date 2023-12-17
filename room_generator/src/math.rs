use bevy::prelude::*;

#[inline(always)]
pub fn rotation_from_direction(vector: &Vec2) -> Quat {
    let angle = vector.y.atan2(vector.x);
    Quat::from_rotation_z(angle)
}

// source: http://phrogz.net/round-to-nearest-via-modulus-division
#[inline(always)]
#[allow(unused)]
pub fn round_to_nearest(input: f32, multiple: u32) -> f32 {
    let multiple = multiple as f32;

    let half = multiple / 2.;
    return input + half - (input - half) % multiple;
}
#[inline(always)]
#[allow(unused)]
pub fn round_to_nearest_vec(input: Vec2, multiple: UVec2) -> Vec2 {
    let multiple = multiple.as_vec2();

    return Vec2::new(
        input.x + (multiple.x / 2.) - (input.x - (multiple.x / 2.)) % multiple.x,
        input.y + (multiple.y / 2.) - (input.y - (multiple.y / 2.)) % multiple.y,
    );
}
