use super::room::{DoorOrientation, StructureDimensions};
use bevy::prelude::*;

pub trait IsizeTupleConverter<T> {
    fn as_isize(&self) -> (T, T);
}
impl IsizeTupleConverter<isize> for Vec2 {
    fn as_isize(&self) -> (isize, isize) {
        (self.x as isize, self.y as isize)
    }
}

pub fn maybe_flip_bl_tr<'a>(
    comparison: bool,
    room1: &'a dyn StructureDimensions,
    room2: &'a dyn StructureDimensions,
) -> (
    bool,
    &'a dyn StructureDimensions,
    &'a dyn StructureDimensions,
) {
    if comparison {
        (false, room1, room2)
    } else {
        (true, room2, room1)
    }
}

pub fn find_out_door_orientation(overlap_x: i32, overlap_y: i32) -> DoorOrientation {
    let overlap = overlap_x.max(overlap_y);
    let door_orientation = if overlap == overlap_x {
        DoorOrientation::Horziontal
    } else {
        DoorOrientation::Vertical
    };
    door_orientation
}

/// given 2 rooms, find out if they share an edge. rooms can be separated by a long distance and still share an edge
///
/// [] --- [] -> will return y={overlap}
///
/// []
/// |
/// |
/// [] -> will return x={overlap}
///
/// []
///     [] -> will return x,y=negative  
pub fn common_edge(
    room1: &impl StructureDimensions,
    room2: &impl StructureDimensions,
) -> (i32, i32) {
    let a1_x = room1.anchor_grid().x;
    let a2_x = room1.anchor_grid_end().x;
    let a1_y = room1.anchor_grid().y;
    let a2_y = room1.anchor_grid_end().y;

    let b1_x = room2.anchor_grid().x;
    let b2_x = room2.anchor_grid_end().x;
    let b1_y = room2.anchor_grid().y;
    let b2_y = room2.anchor_grid_end().y;

    let overlap_x = line_overlap(a1_x, a2_x, b1_x, b2_x);
    let overlap_y = line_overlap(a1_y, a2_y, b1_y, b2_y);

    (overlap_x, overlap_y)
}

// try to find some type of source for this but ngl i came up with this myself
pub fn line_overlap(a1: i32, a2: i32, b1: i32, b2: i32) -> i32 {
    let start = a1.max(b1);
    let end = a2.min(b2);

    // this can return negative values
    end - start
}
