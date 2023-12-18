use super::generation::{MapArea, MapResource, WorldgenSettings};
use super::room::StructureDimensions;
use super::room::{Room, Tile};
use crate::assets::TextureAtlases;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_text_mode::{TextModeSpriteSheetBundle, TextModeTextureAtlasSprite};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

pub type Palette = Vec<Color>;

#[derive(Serialize, Deserialize, Debug, Resource)]
pub struct ColorPalettes {
    f_ground: Palette,
    b_ground: Palette,
    f_wall: Palette,
    b_wall: Palette,
}
impl ColorPalettes {
    pub fn init() -> Self {
        Self {
            f_ground: vec![Color::hex("#222222").unwrap()],
            b_ground: vec![
                Color::hex("#222222").unwrap(),
                //Color::hex("#F1DEDE").unwrap(),
            ],
            f_wall: vec![
                Color::hex("#777777").unwrap(),
                //Color::hex("76657b").unwrap(),
            ],
            b_wall: vec![Color::hex("#777777").unwrap()],
        }
    }
    pub fn get_random_from(&self, palette: &Palette) -> Color {
        palette[fastrand::usize(0..999999) % palette.len()]
    }
}

#[derive(Debug, Resource, Reflect)]
#[reflect(Resource)]
pub struct WorldgenGizmos {
    pub show_graph_edges: bool,
    pub show_triangulation: bool,
    pub show_middle_of_rooms: bool,
    pub show_doors: bool,
}

impl Default for WorldgenGizmos {
    fn default() -> Self {
        Self {
            show_graph_edges: true,
            show_triangulation: false,
            show_middle_of_rooms: true,
            show_doors: true,
        }
    }
}

pub fn spawn_rooms_visuals(
    mut map_res: ResMut<MapResource>,
    mut commands: Commands,
    atlases: Res<TextureAtlases>,
    worldgen: Res<WorldgenSettings>,
    palettes: Res<ColorPalettes>,
) {
    let map_area = map_res.map_area_mut();
    for room in map_area.rooms.values() {
        if !room.is_visible {
            continue;
        }

        let mut children: Vec<Entity> = Vec::new();
        for y in 0..room.height() {
            for x in 0..room.length() {
                // safety: x and y exist for sure
                let tile = room.get_tile(UVec2::new(x as u32, y as u32)).unwrap();
                let index = 0usize;
                let fg = match *tile {
                    Tile::Ground => palettes.get_random_from(&palettes.f_ground),
                    Tile::Wall => palettes.get_random_from(&palettes.f_wall),
                };
                let bg = match *tile {
                    Tile::Ground => palettes.get_random_from(&palettes.b_ground),
                    Tile::Wall => palettes.get_random_from(&palettes.b_wall),
                };

                let tile_entity = commands.spawn(TextModeSpriteSheetBundle {
                    sprite: TextModeTextureAtlasSprite {
                        index,
                        fg,
                        bg,
                        anchor: Anchor::BottomLeft,
                        ..default()
                    },
                    texture_atlas: atlases.basic_tile.clone(),
                    transform: Transform::from_translation(Vec3::new(
                        x as f32 * worldgen.tile_size.x as f32,
                        y as f32 * worldgen.tile_size.y as f32,
                        -10.,
                    )),
                    ..default()
                });
                children.push(tile_entity.id());
            }
        }
        let spatial_bundle = SpatialBundle::from_transform(Transform::from_translation(
            room.anchor_world(worldgen.tile_size).as_vec2().extend(-10.),
        ));

        // TODO: FIND a way to only have room IN ECS ONLY instead of here (ECS) AND MapArea !!!
        commands
            .spawn((
                spatial_bundle,
                room.clone(),
                Name::new(format!("Room: {}", room.id().to_string())),
            ))
            .push_children(&children);
    }
    commands.spawn(map_res.map_area_mut().clone());
}

pub fn gizmo_graph_edges(
    mut gizmos: Gizmos,
    worldgen: Res<WorldgenSettings>,
    map_area_query: Query<&MapArea>,
    gizmo_settings: Res<WorldgenGizmos>,
) {
    if !gizmo_settings.show_graph_edges {
        return;
    }
    for map_area in map_area_query.iter() {
        let graph = &map_area.graph.as_ref().unwrap().reassembled_graph;
        for edge in graph.all_edges() {
            let a = &map_area.rooms[&edge.0];
            let b = &map_area.rooms[&edge.1];
            gizmos.line_2d(
                a.center_world(worldgen.tile_size),
                b.center_world(worldgen.tile_size),
                Color::ORANGE_RED,
            );
        }
    }
}

#[allow(unused)]
pub fn gizmo_triangulation(
    mut gizmos: Gizmos,
    worldgen: Res<WorldgenSettings>,
    map_area_query: Query<&MapArea>,
    gizmo_settings: Res<WorldgenGizmos>,
) {
    if !gizmo_settings.show_triangulation {
        return;
    }
    for map_area in map_area_query.iter() {
        for (ia, ib, ic) in map_area
            .triangulation
            .as_ref()
            .unwrap()
            .triangles
            .iter()
            .tuples()
        {
            let a = map_area.get_main_rooms()[*ia].center_world(worldgen.tile_size);
            let b = map_area.get_main_rooms()[*ib].center_world(worldgen.tile_size);
            let c = map_area.get_main_rooms()[*ic].center_world(worldgen.tile_size);
            gizmos.line_2d(a, b, Color::GREEN);
            gizmos.line_2d(b, c, Color::GREEN);
            gizmos.line_2d(a, c, Color::GREEN);
        }
    }
}

pub fn gizmo_room_middle_circle(
    mut gizmos: Gizmos,
    worldgen: Res<WorldgenSettings>,
    room_query: Query<&Room>,
    gizmo_settings: Res<WorldgenGizmos>,
    map: Res<MapResource>,
) {
    if !gizmo_settings.show_middle_of_rooms {
        return;
    }
    for room in room_query.iter() {
        let color = if room.details.is_main {
            if map
                .map_area()
                .graph
                .as_ref()
                .unwrap()
                .main_path_rooms
                .contains(&room.id())
            {
                Color::DARK_GREEN
            } else {
                Color::BLUE
            }
        } else {
            Color::RED
        };
        gizmos.circle_2d(room.center_world(worldgen.tile_size), 5., color);
    }
}