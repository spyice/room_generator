use bevy::{prelude::*, transform::TransformSystem};

use crate::GameConfiguration;

//use crate::{mouse_utils::MousePositionEvent, player::Player};

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                system_camera_follow_player.after(TransformSystem::TransformPropagate),
                update_camera_scale,
            ),
        );
    }
}

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct CameraFollowThis;

fn system_camera_follow_player(
    mut query_camera: Query<&mut Transform, (With<MainCamera>, Without<CameraFollowThis>)>,
    query_player: Query<&Transform, (With<CameraFollowThis>, Without<MainCamera>)>,
    config: Res<GameConfiguration>,
    time: Res<Time>,
) {
    let player_t = query_player
        .iter()
        .nth(0)
        .map(|t| t.translation)
        .unwrap_or(Vec3::ZERO)
        .truncate();

    for mut cam in query_camera.iter_mut() {
        let offset = Vec3::new(
            player_t.x, /* - mouse_player_offset.x*/
            player_t.y, /* - mouse_player_offset.y*/
            cam.translation.z,
        );
        cam.translation = cam.translation.lerp(
            offset,
            config.camera_follow_player_speed * time.delta_seconds(),
        );
    }
}

pub fn update_camera_scale(
    config: Res<GameConfiguration>,
    mut query: Query<&mut OrthographicProjection>,
) {
    if !config.is_changed() {
        return;
    }

    for mut cam in query.iter_mut() {
        cam.scale = 1. * config.camera_scale;
    }
}
