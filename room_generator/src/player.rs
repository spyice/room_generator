use bevy::{ecs::system::EntityCommands, prelude::*};
use bevy_rapier2d::prelude::*;

use crate::{
    GameConfiguration, GameState,
};

pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(
            PreUpdate,
            (
                process_player_movement_input, /*process_player_attack_input*/
            )
                .run_if(in_state(GameState::Playing)),
        );
    }
}

#[derive(Component)]
pub struct Character {}

#[derive(Bundle)]
pub struct PlayerBundle {
    pub sprite_bundle: SpriteBundle,
    pub character: Character,
    pub player: Player,
}

#[derive(Component)]
pub struct Player;

impl Player {
    pub fn spawn_player(ec: &mut EntityCommands) {
        ec.insert((
            Player,
            Transform::from_translation(Vec3::new(0., 000., 10.)),
            Name::new("Player"),
            RigidBody::Dynamic,
            Velocity::default(),
            LockedAxes::ROTATION_LOCKED_Z,
            Friction::coefficient(0.0),
        ));
    }
}

/// Takes in key codes (WASD, etc) and calls movement.add_direction
///
/// The input vector is not normalized, it is done in "movement"
pub fn process_player_movement_input(
    mut query: Query<(&Player, &mut Velocity)>,
    keyboard_input: Res<Input<KeyCode>>,
    config: Res<GameConfiguration>,
) {
    let mut player_velocity = match query.get_single_mut() {
        Ok(vel) => vel.1,
        Err(_) => return,
    };

    let mut v = Vec2::ZERO;
    if keyboard_input.pressed(KeyCode::A) {
        v.x += -1.;
    }
    if keyboard_input.pressed(KeyCode::D) {
        v.x += 1.;
    }
    if keyboard_input.pressed(KeyCode::W) {
        v.y += 1.;
    }
    if keyboard_input.pressed(KeyCode::S) {
        v.y += -1.;
    }
    player_velocity.linvel = v.normalize_or_zero() * config.player_speed;
}