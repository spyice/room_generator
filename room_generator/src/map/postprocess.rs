use bevy::{prelude::*, utils::HashSet};
use itertools::Itertools;

use crate::map::room::Room;

use super::{
    connecting::RoomConnectionType,
    generation::{MapResource, WorldgenSettings},
    room::StructureDimensions,
};

pub fn strip_unconnected_rooms(mut map: ResMut<MapResource>, worldgen: Res<WorldgenSettings>) {
    if !worldgen.clear_unconnected_rooms {
        return;
    }
    let mut rooms_with_connections = HashSet::new();
    for connection in map.map_area().connections.as_ref().unwrap() {
        rooms_with_connections.insert(connection.room1_id);
        rooms_with_connections.insert(connection.room2_id);
    }

    map.map_area_mut().rooms.values_mut().for_each(|r| {
        if !rooms_with_connections.contains(&r.id()) {
            r.is_visible = false;
        }
    });
}

pub fn outer_walls(mut map: ResMut<MapResource>) {
    for room in map.map_area_mut().rooms.values_mut() {
        room.fill_edges();
    }
}

/// some adjacent tiles are made into Ground tiles
pub fn carve_doors(mut map: ResMut<MapResource>, worldgen: Res<WorldgenSettings>) {
    let connections = map
        .map_area()
        .connections
        .as_ref()
        .expect("cannot carve doors without knowing about room connections")
        .clone();
    for c in connections.iter() {
        match &c.data {
            RoomConnectionType::Adjacent(adjacent_tiles) => {
                let room = map.map_area_mut().rooms.get_mut(&c.room1_id).unwrap();
                let mut tiles = adjacent_tiles.room1.1.clone();
                remove_outer_tiles(&mut tiles);
                cut(&mut tiles, &worldgen);
                carve(room, &tiles);

                let room = map.map_area_mut().rooms.get_mut(&c.room2_id).unwrap();
                let mut tiles = adjacent_tiles.room2.1.clone();
                remove_outer_tiles(&mut tiles);
                cut(&mut tiles, &worldgen);
                carve(room, &tiles);
            }
            _ => (),
        }
    }

    fn cut<T>(tiles: &mut Vec<T>, worldgen: &WorldgenSettings) {
        let l: i32 = tiles.len() as i32 - worldgen.max_passage_width as i32;
        if  l > 0 {
            for _ in 0..l {
              tiles.pop();
            }
        } 
    }

    fn carve(room: &mut Room, tiles: &[UVec2]) {
        for &tile_position in tiles.iter() {
            room.set_tile(tile_position, super::room::Tile::Ground);
        }
    }
    fn remove_outer_tiles<T>(tiles: &mut Vec<T>) {
        if tiles.len() >= 2 {
            tiles.remove(0);
            tiles.pop();
        } else {
            tiles.clear()
        }
    }
}

/// this function ENSURES that EVERY room can be entered, i.e. no walls blocking the entrance
pub fn carve_path(mut map: ResMut<MapResource>, worldgen: Res<WorldgenSettings>) {
    // huge mess
    fn successors(current_tile: IVec2, map: &MapResource) -> Vec<(IVec2, u32)> {
        let mut successors = vec![
            current_tile + IVec2::new(1, 0),
            current_tile + IVec2::new(0, 1),
            current_tile + IVec2::new(-1, 0),
            current_tile + IVec2::new(0, -1),
        ];
        // the points could not belong to any room, so we have to filter them. if the point is not in a room, remove it from successors
        successors.retain(|&e| map.map_area().point_to_room((e.x, e.y)).is_some());

        let successors: Vec<(IVec2, u32)> = successors
            .iter()
            .map(|e| {
                let room = map
                    .map_area()
                    .rooms
                    .get(&map.map_area().point_to_room((e.x, e.y)).unwrap())
                    .unwrap();
                let local_coordinates = room.global_to_local(*e).unwrap();
                let cost = match room.get_tile(local_coordinates).unwrap() {
                    crate::map::room::Tile::Ground => 1,
                    crate::map::room::Tile::Wall => 100,
                };

                (*e, cost)
            })
            .collect_vec();
        successors
    }

    // (chebyshev metric) as seen in https://chris3606.github.io/GoRogue/articles/grid_components/measuring-distance.html
    // i.e.
    // 2 2 2 2 2
    // 2 1 1 1 2
    // 2 1 0 1 2
    // 2 1 1 1 2
    // 2 2 2 2 2

    // multiplied by a constant
    /* fn prefer_inner_tiles(current_tile: IVec2, map: &MapResource) -> u32 {
        let room = map
            .map_area()
            .rooms
            .get(
                &map.map_area()
                    .point_to_room((current_tile.x, current_tile.y))
                    .unwrap(),
            )
            .unwrap();
        let local_coordinates = room.global_to_local(current_tile).unwrap().as_ivec2();
        let local_center_of_room = match room.global_to_local(room.center_grid().as_ivec2()) {
            Some(inner) => inner.as_ivec2(),
            None => IVec2::new((room.length() / 2) as i32, (room.height() / 2) as i32),
        };

        let cost = (local_center_of_room - local_coordinates).abs().as_uvec2();

        // chebvyshev metric
        cost.x.max(cost.y).pow(3)
    } */

    // get all edges of the graph
    for (room1, room2, _weight) in map
        .map_area()
        .graph
        .as_ref()
        .unwrap()
        .clone()
        .reassembled_graph
        .all_edges()
    {
        let room1 = map.map_area().rooms.get(&room1).unwrap();
        let room2 = map.map_area().rooms.get(&room2).unwrap();

        // run a* between every room
        let path = pathfinding::directed::astar::astar(
            &room1.center_grid().as_ivec2(),
            |current_tile| successors(*current_tile, &map),
            |h| {
                let distance = h.distance_squared(room2.center_grid().as_ivec2()).abs() as u32;
                //let inner_tile_preference = prefer_inner_tiles(*h, &map);
                //println!("{}, {}", distance, inner_tile_preference);
                //distance + inner_tile_preference
                distance
            },
            |s| *s == room2.center_grid().as_ivec2(),
        );

        if path.is_none() {
            println!(
                "no path between room {} and room {}",
                room1.id(),
                room2.id()
            );
            return;
        }

        for tile_position in path.unwrap().0.iter() {
            let point_to_room = map
                .map_area()
                .point_to_room((tile_position.x, tile_position.y))
                .unwrap();
            let room = map.map_area_mut().rooms.get_mut(&point_to_room).unwrap();
            let local_coordinates = room.global_to_local(*tile_position).unwrap();

            // carve the path
            for x in local_coordinates
                .x
                .saturating_sub(worldgen.min_passage_width / 2)
                ..=local_coordinates.x + worldgen.min_passage_width / 2
            {
                for y in local_coordinates
                    .y
                    .saturating_sub(worldgen.min_passage_width / 2)
                    ..=local_coordinates.y + worldgen.min_passage_width / 2
                {
                    let position: UVec2 = UVec2::new(x, y);
                    room.set_tile(position, crate::map::room::Tile::Ground);
                }
            }
        }
    }
}

pub fn aesthetizise(mut map: ResMut<MapResource>, _worldgen: Res<WorldgenSettings>) {
    for room in map.map_area_mut().rooms.values_mut() {
        for modifier in &room.details.aesthetic_modifiers.clone() {
            modifier.generate_features(room, false);
        }
    }
}

/* /// an implementation of a flood fill algorithm to catch wall formations that are very tiny
/// those walls are then removed
/// i.e.
/// ....
/// .##.
/// ....
/// ->
/// ....
/// ....
/// .... */
/* pub fn remove_random_walls(mut map: ResMut<MapResource>, worldgen: Res<WorldgenSettings>) {} */
