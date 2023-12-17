use bevy_inspector_egui::quick::WorldInspectorPlugin;
use player::Player;
use std::{fs::File, io::Read, time::Duration};

use crate::{
    camera::CameraPlugin,
    map::{visuals::ColorPalettes, MapPlugin},
    player::PlayerPlugin,
};

use bevy::{asset::ChangeWatcher, core_pipeline::{clear_color::ClearColorConfig, tonemapping::{DebandDither, Tonemapping}}};
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_rapier2d::prelude::{NoUserData, RapierConfiguration, RapierPhysicsPlugin};
use bevy_text_mode::TextModePlugin;
use camera::{MainCamera, CameraFollowThis};
use serde::{Deserialize, Serialize};

mod assets;
mod camera;
mod map;
mod player;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum GameState {
    #[default]
    AssetLoading,
    Playing,
}

fn main() {
    let mut app = App::new();
    app.insert_resource(GameConfiguration::read_config_from_file());
    app.register_type::<GameConfiguration>();
    app.insert_resource(ColorPalettes::init());
    app.add_state::<GameState>();
    app.add_loading_state(
        LoadingState::new(GameState::AssetLoading).continue_to_state(GameState::Playing),
    );
    app.add_collection_to_loading_state::<_, assets::TextureAtlases>(GameState::AssetLoading);
    app.add_plugins(
        DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(AssetPlugin {
                watch_for_changes: ChangeWatcher::with_delay(Duration::from_secs_f32(2.0)),
                ..Default::default()
            }),
    );
    app.add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(32.0));
    app.add_plugins(WorldInspectorPlugin::new());
    app.add_plugins(TextModePlugin);
    app.add_plugins((
        PlayerPlugin,
        CameraPlugin,
        MapPlugin,
    ));

    app.add_systems(Startup, rapier_setup);
    app.add_systems(Startup, set_config);
    app.add_systems(OnEnter(GameState::Playing), setup);
    app.run();
}

fn rapier_setup(mut rapier_config: ResMut<RapierConfiguration>) {
    rapier_config.gravity = Vec2::splat(0.);
}

fn set_config(mut config: ResMut<GameConfiguration>) {
    *config = GameConfiguration::read_config_from_file();
}

#[derive(Serialize, Deserialize, Debug, Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct GameConfiguration {
    pub player_to_mouse_rotation_speed: f32,
    pub camera_follow_player_speed: f32,
    pub player_speed: f32,
    pub camera_scale: f32,
}

impl GameConfiguration {
    pub fn read_config_from_file() -> Self {
        let mut file =
            File::open("room_generator/assets/config.yaml").expect("could not read config.yaml");
        let mut config_string = String::new();
        file.read_to_string(&mut config_string)
            .expect("could not read config.yaml to string");
        let config: GameConfiguration =
            serde_yaml::from_str(&config_string).expect("could not deserialize config.yaml");
        config
    }
}

fn setup(mut commands: Commands, config: Res<GameConfiguration>) {
    let mut player = commands.spawn(CameraFollowThis);
    Player::spawn_player(&mut player);

    let mut camera = Camera2dBundle::default();
    camera.camera_2d.clear_color = ClearColorConfig::Custom(Color::BLACK);
    camera.deband_dither = DebandDither::Enabled;
    camera.tonemapping = Tonemapping::AcesFitted;

    camera.transform = Transform::from_translation(Vec3::new(0., 0., 100.));
    camera.projection.scale = 1. * config.camera_scale;
    commands.spawn((camera, MainCamera));
}