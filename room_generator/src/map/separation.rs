use bevy::prelude::*;
use itertools::Itertools;

use crate::map::room::{is_overlapping, StructureDimensions};

use super::{
    generation::{MapResource, WorldgenSettings},
    room::Room,
};

pub fn separate_rooms(mut map_res: ResMut<MapResource>, worldgen: Res<WorldgenSettings>) {
    let rooms = &mut map_res.map_area_mut().rooms;
    let list_of_indices = (0..rooms.len()).collect::<Box<[usize]>>();

    let mut iteration_count = 0;
    while rooms_overlap_exists(&rooms.values().collect_vec()) {
        if iteration_count >= 5000 {
            break;
        }

        for (a, b) in list_of_indices.iter().tuple_combinations() {
            if is_overlapping(&rooms[a], &rooms[b]) {
                // TODO: the *2 sucks ass
                let direction = rooms[b].center_grid() - rooms[a].center_grid();
                let direction = direction.normalize_or_zero();

                if !rooms[a].is_position_fixed {
                    move_room(
                        &mut rooms.get_mut(a).unwrap(),
                        -direction,
                        worldgen.separation_factor,
                    );
                }
                if !rooms[b].is_position_fixed {
                    move_room(
                        &mut rooms.get_mut(b).unwrap(),
                        direction,
                        worldgen.separation_factor,
                    );
                }
            }
        }
        iteration_count += 1;
    }
    println!("-- iteration count for separation stage: {iteration_count} --");
}

fn move_room(room: &mut Room, direction: Vec2, factor: f32) {
    let move_amount = direction * factor;
    room.offset_anchor_grid(move_amount.as_ivec2());
}

fn rooms_overlap_exists(rooms: &[&Room]) -> bool {
    for (i, &room1) in rooms.iter().enumerate() {
        for (j, &room2) in rooms.iter().enumerate() {
            if i == j {
                continue;
            }
            if is_overlapping(room1, room2) {
                return true;
            }
        }
    }
    return false;
}
