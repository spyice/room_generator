use bevy::prelude::*;

use crate::GameState;

use self::{
    generation::{MapResource, RegenerateRoomsEvent, WorldgenRng, WorldgenSettings},
    visuals::WorldgenGizmos,
};

pub mod room;
pub mod util;
pub mod aesthetics;
pub mod connecting;
pub mod generation;
pub mod graphing;
pub mod postprocess;
pub mod separation;
pub mod visuals;
pub mod presets;

pub struct MapPlugin;
impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<WorldgenSettings>();
        app.register_type::<WorldgenGizmos>();
        app.init_resource::<WorldgenGizmos>();
        app.init_resource::<WorldgenSettings>();
        app.init_resource::<MapResource>();
        app.add_event::<RegenerateRoomsEvent>();
        //app.add_systems(OnEnter(GameState::Playing), setup);

        app.add_systems(PostStartup, init_worldgen_rng);

        app.add_systems(
            PreUpdate,
            (
                generation::despawn_chunks,
                (
                    // init
                    self::init_worldgen_rng,
                    presets::init_preset_resource,
                    // generation with presets
                    generation::generate_rooms,
                    //generation::custom_rooms,
                    generation::determine_main_rooms,
                    // separation
                    separation::separate_rooms,
                    // graphing
                    graphing::get_triangulation,
                    graphing::make_graphs,
                    //graphing::assign_meaning,
                    //generation::replace_rooms,
                    //separation::separate_rooms,
                    // making connections between rooms
                    connecting::connect_rooms,
                    graphing::remake_graphs,
                    //postprocessing
                    postprocess::strip_unconnected_rooms,
                    postprocess::outer_walls,
                    postprocess::aesthetizise,
                    postprocess::carve_path,
                    postprocess::outer_walls, // do it again just to be sure
                    postprocess::carve_doors,
                    //postprocess::remove_random_walls,
                    // spawn rooms into the world
                    visuals::spawn_rooms_visuals,
                )
                    .chain(),
            )
                .run_if(on_event::<RegenerateRoomsEvent>()),
        );

        app.add_systems(
            PostUpdate,
            check_for_worldgen_changes.run_if(in_state(GameState::Playing)),
        );

        app.add_systems(
            PostUpdate,
            (
                visuals::gizmo_triangulation,
                visuals::gizmo_room_middle_circle,
                visuals::gizmo_graph_edges,
            )
                .chain(),
        );
    }
}

fn check_for_worldgen_changes(
    worldgen: Res<WorldgenSettings>,
    mut events_w: EventWriter<RegenerateRoomsEvent>,
) {
    if worldgen.is_changed() {
        events_w.send(RegenerateRoomsEvent);
    }
}

fn init_worldgen_rng(world: &mut World) {
    let settings = world.get_resource::<WorldgenSettings>().unwrap();
    let rng = WorldgenRng::new(settings.global_seed);
    world.insert_resource(rng);
}
