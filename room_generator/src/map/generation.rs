use super::{
    connecting::RoomConnection,
    graphing::RoomGraph,
    presets::{self, RoomPresetResource},
    room::{Room, StructureDimensions}
};
use bevy::{prelude::*, utils::HashMap};
use delaunator::Triangulation;
use itertools::Itertools;

#[derive(Debug, Resource)]
pub struct MapResource {
    map_area: MapArea,
}
impl Default for MapResource {
    fn default() -> Self {
        Self {
            map_area: MapArea {
                rooms: HashMap::new(),
                initial_connections: Vec::new(),
                triangulation: None,
                graph: None,
                connections: None,
            },
        }
    }
}
impl MapResource {
    pub fn map_area(&self) -> &MapArea {
        &self.map_area
    }
    pub fn map_area_mut(&mut self) -> &mut MapArea {
        &mut self.map_area
    }
    pub fn next_room_id(&mut self) -> usize {
        let current = self.map_area.rooms.len();
        current
    }
}

#[derive(Debug, Clone, Component)]
pub struct MapArea {
    pub rooms: HashMap<usize, Room>,
    pub initial_connections: Vec<(usize, usize)>,
    pub triangulation: Option<Triangulation>,
    pub graph: Option<RoomGraph>,
    pub connections: Option<Vec<RoomConnection>>,
}

impl MapArea {
    pub fn get_main_rooms(&self) -> Vec<&Room> {
        let rooms = self
            .rooms
            .values()
            .sorted_by(|&r1, &r2| Ord::cmp(&r1.id(), &r2.id()))
            .filter(|&r| r.details.is_main)
            .collect::<_>();
        rooms
    }
    /* pub fn mean_room_size_grid(&self) -> f32 {
        self.rooms
            .values()
            .fold(0., |a, b| a + b.get_area_grid() as f32) as f32
            / self.rooms.len() as f32
    } */
    /* pub fn mean_room_size_world(&self, tile_size: UVec2) -> f32 {
        self.rooms
            .iter()
            .fold(0., |a, b| a + b.get_area_world(tile_size) as f32) as f32
            / self.rooms.len() as f32
    } */

    pub fn point_to_room(&self, point: (i32, i32)) -> Option<usize> {
        // probably should  only iter through non-main rooms. not implemented for now
        for room in self.rooms.values() {
            if room.is_point_inside(point) {
                return Some(room.id());
            }
        }
        None
    }
}

#[derive(Debug, Clone, Deref, DerefMut, Resource)]
pub struct WorldgenRng(fastrand::Rng);
impl WorldgenRng {
    pub fn new(seed: u64) -> Self {
        Self(fastrand::Rng::with_seed(seed))
    }
}

#[derive(Debug, Clone, Reflect, Resource)]
#[reflect(Resource)]
pub struct WorldgenSettings {
    pub tile_size: UVec2,
    pub global_seed: u64,
    pub presets_to_spawn: usize,
    pub spawn_range: i32,
    pub snap_to: u32, // doesnt do anything anynmore
    pub main_room_threshold_multiplier: f32,
    pub separation_factor: f32,
    pub graph_reassembly_percentage: f32,
    pub clear_unconnected_rooms: bool,
    pub min_passage_width: u32,
    pub max_passage_width: u32,
    pub threshold: u32,
}
impl Default for WorldgenSettings {
    fn default() -> Self {
        let settings = Self {
            tile_size: UVec2 { x: 8, y: 8 },
            global_seed: 44,
            presets_to_spawn: 5,
            spawn_range: 100,
            snap_to: 1,
            main_room_threshold_multiplier: -1.0,
            graph_reassembly_percentage: 0.30,
            min_passage_width: 6,
            max_passage_width: 12,
            threshold: 9999,
            clear_unconnected_rooms: true,
            separation_factor: 2.,
        };
        settings
    }
}
#[derive(Debug, Clone, Copy, Reflect)]
pub struct RoomGenerationSettings {
    pub rooms_amount: usize,
    pub spawn_range: i32,
    pub min_length: usize,
    pub max_length: usize,
    pub min_height: usize,
    pub max_height: usize,
    pub snap_to: u32,
}

pub fn generate_rooms(
    worldgen: Res<WorldgenSettings>,
    mut map: ResMut<MapResource>,
    mut rng: ResMut<WorldgenRng>,
    presets: Res<RoomPresetResource>,
) {
    let mut rooms = HashMap::new();
    let mut room_id_count = 0usize;
    let mut initial_connections = Vec::new();

    for _ in 0..worldgen.presets_to_spawn {
        let preset = &presets.get_preset_by_type("normal", &mut rng);
        let preset = preset.clone().unwrap();

        let mut preset_rooms = presets::generate_rooms_from_preset(&preset, &mut rng);

        // world position
        let x = rng.i32(-worldgen.spawn_range..=worldgen.spawn_range);
        let y = rng.i32(-worldgen.spawn_range..=worldgen.spawn_range);
        let world_pos = IVec2::new(x, y);

        let previous_max_id = room_id_count;

        for dim in preset_rooms.0.iter_mut() {
            dim.offset_anchor_grid(world_pos);

            let room = Room::new2(room_id_count, dim.dimensions, dim.details.clone());
            rooms.insert(room_id_count, room);

            room_id_count += 1;
        }

        preset_rooms.1.iter_mut().for_each(|c| {
            c.0 += previous_max_id;
            c.1 += previous_max_id;
        });

        initial_connections.append(&mut preset_rooms.1);
    }

    map.map_area = MapArea {
        rooms,
        initial_connections,
        triangulation: None,
        graph: None,
        connections: None,
    };

    println!("init connections: {:?}", map.map_area.initial_connections);
}

/* pub fn custom_rooms(mut map: ResMut<MapResource>, worldgen: Res<WorldgenSettings>) {

    let mut rooms  = HashMap::new();
    /* rooms.insert(0, Room::new(0, 20, 20, IVec2::new(0, 0), true));
    rooms.insert(1, Room::new(1, 20, 20, IVec2::new(50, 15), true));
    rooms.insert(2, Room::new(2, 20, 20, IVec2::new(25, 12), true));
    rooms.insert(3, Room::new(3, 20, 20, IVec2::new(25, -12), true));

    rooms.insert(4, Room::new(4, 20, 20, IVec2::new(100, 100), true));
    rooms.insert(5, Room::new(5, 20, 20, IVec2::new(115, 115), true)); */

    let aesthetic = Aesthetics::Pillars(Pillars{amount: 5, pillar_size: 2, generation_type: super::aesthetics::PillarGenerationType::BothAxes});
    let middle = Room::new2(
        1, 
        RoomDimensions {anchor: IVec2{x: 0, y: -5}, height: 30, length: 30}, 
        RoomDetails {is_main: true, room_type: super::room::RoomType::Normal, aesthetic_modifiers: vec![aesthetic] });

    rooms.insert(0, Room::new(0, 20, 20, IVec2::new(-30, 10), true));
    rooms.insert(1, Room::new(1, 20, 20, IVec2::new(-0, 6), true));
    rooms.insert(2, Room::new(2, 20, 20, IVec2::new(30, 0), true));

    let initial_connections = vec![
        (0, 2),
    ];


    map.map_area = MapArea {
        rooms,
        initial_connections,
        triangulation: None,
        graph: None,
        connections: None,
    };
} */

pub fn determine_main_rooms(mut map: ResMut<MapResource>, _worldgen: Res<WorldgenSettings>) {
    //let mean_room_size = map.map_area().mean_room_size_grid();
    for room in map.map_area_mut().rooms.values_mut() {
        room.details.is_main = true;
        /* if room.get_area_grid() as f32 > mean_room_size * worldgen.main_room_threshold_multiplier {
            room.details.is_main = true;
        } */
    }
}

#[derive(Event)]
pub struct RegenerateRoomsEvent;

pub fn despawn_chunks(
    events: EventReader<RegenerateRoomsEvent>,
    query_rooms: Query<Entity, With<Room>>,
    query_map_areas: Query<Entity, With<MapArea>>,
    query_text: Query<Entity, With<Text>>,
    mut commands: Commands,
) {
    //println!("asd aasd {}", query_map_areas.iter().len());
    if events.len() > 0 {
        for ent in query_rooms.iter() {
            commands.entity(ent).despawn_recursive();
        }
        for ent in query_map_areas.iter() {
            commands.entity(ent).despawn();
        }
        for ent in query_text.iter() {
            commands.entity(ent).despawn();
        }
    }
}
