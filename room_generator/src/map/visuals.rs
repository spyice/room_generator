use super::generation::{MapArea, MapResource, WorldgenSettings};
use super::room::StructureDimensions;
use super::room::{Room, Tile};
use crate::assets::TextureAtlases;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_text_mode::{TextModeSpriteSheetBundle, TextModeTextureAtlasSprite};
use itertools::Itertools;

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
                let color = match *tile {
                    Tile::Ground => Color::hex("#222222").unwrap(),
                    Tile::Wall => Color::hex("#222222").unwrap(),
                };

                let tile_entity = commands.spawn(TextModeSpriteSheetBundle {
                    sprite: TextModeTextureAtlasSprite {
                        index: 0,
                        fg: color,
                        bg: color,
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