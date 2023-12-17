use bevy::prelude::*;
use bresenham::Bresenham;
use itertools::Itertools;

use super::{
    generation::{MapArea, MapResource, WorldgenSettings},
    room::{is_overlapping, DoorOrientation, Room, RoomDimensions, Structure, StructureDimensions},
    util::{find_out_door_orientation, maybe_flip_bl_tr, IsizeTupleConverter},
};
use crate::map::util::*;

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct AdjacentTiles {
    pub room1: (usize, Vec<UVec2>), // (room_id, list of tiles)
    pub room2: (usize, Vec<UVec2>),
}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
#[rustfmt::skip]
pub enum RoomConnectionType {
    Adjacent(AdjacentTiles),                    // 2rooms are right next to each other. Contains adjacent(to the other room) tiles for each room
    Separated,                                  // 2rooms have distance between them, but no other non-main room inbetween. a hallway can be made
    SeparatedRoomsInbetween(Box<[usize]>),   // 2rooms have distance between them, and other rooms inbetween exist. CONTAINS IDS OF INBETWEEN-ROOMS
    SeparatedNoSolution,                     // 2rooms have distance between them, and no other rooms are in between, but a hallway cannot be made
    #[default]
    Unknown,                                    // default or something else. should never happen
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RoomConnection {
    pub room1_id: usize,
    pub room2_id: usize,
    pub data: RoomConnectionType,
}

impl RoomConnection {
    fn new(room1_id: usize, room2_id: usize, ctype: RoomConnectionType) -> Self {
        Self {
            room1_id,
            room2_id,
            data: ctype,
        }
    }
    fn is_adjacent(&self) -> bool {
        match self.data {
            RoomConnectionType::Adjacent(_) => true,
            _ => false,
        }
    }
}

enum LHallwayOrientation {
    RightUp,
    UpRight,
    LeftUp,
    UpLeft,
}

// TODO: change name
pub fn connect_rooms(worldgen: Res<WorldgenSettings>, mut map: ResMut<MapResource>) {
    let mut connections = vec![];

    // first we prepare connections for all the main rooms from the graph
    for (room1_id, room2_id, _) in map
        .map_area()
        .graph
        .as_ref()
        .expect("cannot connect rooms without a graph")
        .reassembled_graph
        .all_edges()
    {
        let room1 = &map.map_area().rooms[&room1_id];
        let room2 = &map.map_area().rooms[&room2_id];

        let ctype = find_out_connection_type(room1, room2, map.map_area(), &worldgen);
        connections.push(RoomConnection::new(room1_id, room2_id, ctype));
    }

    // while not all connections are adjacent, run the algorithm
    let mut iteration_count = 0;
    while !connections.iter().all(|c| c.is_adjacent()) {
        connections = reduce_connections(connections, &mut map, &worldgen);
        // failsafe
        // this can lead to unconnected graphs... for now
        iteration_count += 1;
        if iteration_count >= 10 {
            break;
        }
    }

    // save all connections into the map area
    map.map_area_mut().connections = Some(connections);
}

/// reduces the connections list to only contain connections of type Adjacent
// its also horribly inefficient!
fn reduce_connections(
    connections: Vec<RoomConnection>,
    map: &mut MapResource,
    worldgen: &WorldgenSettings,
) -> Vec<RoomConnection> {
    let mut new_connections = vec![];
    for connection in connections {
        match connection.data {
            RoomConnectionType::Adjacent(_) => {
                new_connections.push(connection);
            }
            RoomConnectionType::Separated => {
                let result =
                    create_and_connect_hallways(&connection, &mut new_connections, map, worldgen);
                match result {
                    Ok(_) => {}
                    Err(result) => {
                        new_connections.push(result);
                    }
                }
            }
            RoomConnectionType::SeparatedRoomsInbetween(ref inbetween_rooms) => {
                connect_multiple_hallways(
                    &connection,
                    inbetween_rooms,
                    &mut new_connections,
                    map,
                    worldgen,
                );
            }
            RoomConnectionType::SeparatedNoSolution => {
                //new_connections.push(connection); // CHANGE THIS
            }
            RoomConnectionType::Unknown => {
                warn!("unknown RoomConnectionType while reducing connections");
            }
        }
    }
    new_connections
}

/// looks at two rooms and determines their current connection type.
fn find_out_connection_type(
    room1: &impl Structure,
    room2: &impl Structure,
    map_area: &MapArea,
    worldgen: &WorldgenSettings,
) -> RoomConnectionType {
    // are the rooms next to each other?
    if let Some(overlap) = are_two_rooms_adjacent(room1, room2, worldgen.min_passage_width) {
        let adjacent_tiles = find_out_adjacent_tiles(room1, room2, map_area, overlap.1);
        return RoomConnectionType::Adjacent(adjacent_tiles);
    }

    // do the two rooms have other rooms in between? yes -> SeparatedAndRoomsInbetween
    if let Some(ids) = rooms_between(room1, room2, map_area) {
        return RoomConnectionType::SeparatedRoomsInbetween(ids.into());
    }

    // could a hallway be made between those two rooms? yes -> Separated
    if let Some(_) = can_rooms_be_connected(room1, room2, worldgen.min_passage_width) {
        return RoomConnectionType::Separated;
    }

    // if nothing else works...
    RoomConnectionType::SeparatedNoSolution
}

/// this function will (probably) create a hallway between multiple rooms, and for each created room, connect it to the previous and next room.
fn connect_multiple_hallways(
    connection: &RoomConnection,
    inbetween_rooms: &[usize],
    new_connections: &mut Vec<RoomConnection>,
    map: &mut MapResource,
    worldgen: &WorldgenSettings,
) {
    // rust moment
    let inbetween_rooms: Vec<usize> = inbetween_rooms.clone().into();
    // build an iterator over all rooms part of the connection
    // includes R1 (source) -> [all inbetween rooms] -> R2 (source)
    let iterator = [connection.room1_id]
        .into_iter()
        .chain(inbetween_rooms)
        .chain([connection.room2_id]);
    // for each pair of rooms (R1->R2; R2->R3; etc)
    for (room1_id, room2_id) in iterator.tuple_windows() {
        let room1 = &map.map_area().rooms[&room1_id];
        let room2 = &map.map_area().rooms[&room2_id];
        // find out how those two rooms should be connected
        let ctype = find_out_connection_type(room1, room2, map.map_area(), &worldgen);

        // we cannot create new hallways here, because it's not guaranteed that the resulting RoomConnectionType == Separated
        // so we push the new connection back onto new_connections. they will be handled by the next iteration
        new_connections.push(RoomConnection::new(room1_id, room2_id, ctype));
    }
}

/// this function will (probably) create a hallway between two rooms, and connect the resulting new room (hallway) to the source rooms
fn create_and_connect_hallways(
    connection: &RoomConnection,
    new_connections: &mut Vec<RoomConnection>,
    map: &mut MapResource,
    worldgen: &WorldgenSettings,
) -> Result<(), RoomConnection> {
    let max_mallway_width = worldgen.max_passage_width.max(worldgen.min_passage_width);

    // fetch the two rooms that need to be connected
    let room1 = &map.map_area().rooms[&connection.room1_id];
    let room2 = &map.map_area().rooms[&connection.room2_id];
    // create a hallway between the two rooms (note: this could be multiple rooms)
    let hallways = create_hallway_dimensions(
        room1,
        room2,
        worldgen.min_passage_width,
        max_mallway_width,
        map.map_area(),
        worldgen,
    );

    // if create_hallway_dimensions returned valid hallways
    if let Ok(room_dimensions) = hallways {
        let mut new_room_ids = vec![];

        // create an actual room from just the dimensions and push it onto the list of all rooms
        for &hallway in room_dimensions.into_iter() {
            let created_room = turn_dimensions_into_room(&hallway, map);
            // we also need a ids vector to create connections between old and new rooms
            new_room_ids.push(created_room.id());
            map.map_area_mut()
                .rooms
                .insert(created_room.id(), created_room);
        }

        // because we just connected two rooms (R1, R2) with a hallway (R3), there are new adjacent connections
        // between R1->R3 and R3->R2. put those into the list of all connections
        // note that because the function "create_hallway_dimensions" can return multiple dimensions in the case of
        // an L shape hallway being created, we need to pairwise iterate over the list using iter().tuple_windows()

        // iterate over R1(source)->R3->R4->R5->...->Rn->R2(source)
        let iterator = [connection.room1_id]
            .into_iter()
            .chain(new_room_ids)
            .chain([connection.room2_id]);
        for (room1_id, room2_id) in iterator.into_iter().tuple_windows() {
            // fetch the two rooms that need to be connected
            let room1 = &map.map_area().rooms[&room1_id];
            let room2 = &map.map_area().rooms[&room2_id];
            let ctype = find_out_connection_type(room1, room2, map.map_area(), &worldgen);

            // the hallway should always be placed next to a room. so it MUST be adjacent to room1 and room2
            //assert!(matches!(ctype, RoomConnectionType::Adjacent(_)));

            new_connections.push(RoomConnection::new(room1_id, room2_id, ctype));
        }
        return Ok(());
    } else {
        match hallways.unwrap_err() {
            CreateHallwayError::Unknown => return Ok(()),
            CreateHallwayError::OverlapNotEnough => return Ok(()),
            CreateHallwayError::CouldNotMakeLShapedHallway(ids) => {
                return Err(RoomConnection {
                    room1_id: ids.0,
                    room2_id: ids.2,
                    data: RoomConnectionType::SeparatedRoomsInbetween([ids.1].into()),
                })
            }
        }
    }
}

// source: self-modified version of https://stackoverflow.com/questions/306316/determine-if-two-rectangles-overlap-each-other
/// returns either Some(overlap, orientation) or None if no overlap is present (rooms are not adjacent)
fn are_two_rooms_adjacent(
    room1: &impl StructureDimensions,
    room2: &impl StructureDimensions,
    min_hallway_width: u32,
) -> Option<(i32, DoorOrientation)> {
    let (overlap_x, overlap_y) = common_edge(room1, room2);

    // if one room shares tiles with another room on any axis
    if overlap_x >= min_hallway_width as i32 || overlap_y >= min_hallway_width as i32 {
        // if the room is adjacent on one axis but not another
        if overlap_x < 0 || overlap_y < 0 {
            return None;
        }
        let overlap = overlap_x.max(overlap_y);
        let orientation = find_out_door_orientation(overlap_x, overlap_y);
        return Some((overlap, orientation));
    }
    return None;
}

/// Returns None if there are no rooms present between room1 and room2.
///
/// Returns Some(Vec<usize>) with Vec<usize> = room IDs of the rooms that are between room1 and room2
fn rooms_between(
    room1: &impl Structure,
    room2: &impl Structure,
    map: &MapArea,
) -> Option<Vec<usize>> {
    // let the library handle returning points
    let list_of_points = Bresenham::new(
        room1.center_grid().as_isize(),
        room2.center_grid().as_isize(),
    )
    .collect::<Vec<(isize, isize)>>();

    let mut inbetween_room_ids: Vec<usize> = vec![];
    for &point in list_of_points.iter() {
        let point = (point.0 as i32, point.1 as i32);
        // if this position contains a room.........
        if let Some(found_room_id) = map.point_to_room(point) {
            // we dont want to add the two "to be connected" rooms to the list, and we dont wnat any duplicate ids either
            if found_room_id == room1.id()
                || found_room_id == room2.id()
                || inbetween_room_ids.contains(&found_room_id)
            {
                continue;
            }
            inbetween_room_ids.push(found_room_id)
        }
    }

    // if IDs exist, return them. otherwise return None
    if !inbetween_room_ids.is_empty() {
        Some(inbetween_room_ids)
    } else {
        None
    }
}

/// tries to create a hallway between room 1 and 2.
/// it does this by first constructing a hallway between 1 and 2, and then checking that hallway for collisions with other rooms
/// iteratively it tries to reduce the room width until the minimum width is reached.
/// if a hallway still cannot be made, None is returned.
/// if a hallway can be made, at any point (earlier -> wider), Some(RoomDimensions) is returned
fn can_rooms_be_connected(
    room1: &impl Structure,
    room2: &impl Structure,
    min_hallway_width: u32,
) -> Option<RoomDimensions> {
    /* let hallways = create_hallway_dimensions(room1, room2, min_hallway_width);
    if let Ok(hallways) = hallways {

    } */
    Some(RoomDimensions {
        anchor: IVec2::default(),
        height: 0,
        length: 0,
    })
}

#[derive(Debug, Default)]
enum CreateHallwayError {
    #[default]
    Unknown,
    OverlapNotEnough,
    CouldNotMakeLShapedHallway((usize, usize, usize)), // contains ID of room that overlapped,
}

/// this function tries to create a hallway between two rooms. it returns dimensions for potentially multiple rooms. if no hallway can be made, return an error
fn create_hallway_dimensions(
    room1: &impl Structure,
    room2: &impl Structure,
    min_hallway_width: u32,
    max_hallway_width: u32,
    map: &MapArea,
    worldgen: &WorldgenSettings,
) -> Result<Box<[RoomDimensions]>, CreateHallwayError> {
    let (overlap_x, overlap_y) = common_edge(room1, room2);
    let overlap = overlap_x.max(overlap_y);
    let orientation = find_out_door_orientation(overlap_x, overlap_y);

    let mut new_hallways: Vec<RoomDimensions>;

    // case: straight hallway
    // can a straight hallway between the two rooms be made? yes -> straight hallway
    if overlap >= min_hallway_width as i32 {
        /* println!(
            "straight hallway from room1 {} to room2 {} has overlap {}",
            room1.id(),
            room2.id(),
            overlap
        ); */
        new_hallways = [generate_straight_hallway(
            room1,
            room2,
            overlap as u32,
            max_hallway_width,
            &orientation,
            worldgen.threshold,
        )]
        .into();
        return Ok(new_hallways.into());
    }

    // case: L shaped hallway
    // the overlap between the two rooms is not enough to create a straight line between them, we need to make an L shaped hallway
    // check if room1 is the left room, if not, room2 is assigned left room. -> algo needs to do less cases
    let (flipped, left_room, right_room) =
        maybe_flip_bl_tr(room1.anchor_grid().x < room2.anchor_grid().x, room1, room2);

    // if the left room is lower than the right room on the y axis
    //   |-->  [right]
    //   |       |^
    // [left] ---|
    // then its reachable by going right->up or up-right
    // otherwise we have this situation
    // [left] <----
    //  |^         |
    //  |----- [right]
    // and we need to go left and up to reach the other room
    let mut orientations = if left_room.anchor_grid().y < right_room.anchor_grid().y {
        vec![LHallwayOrientation::RightUp, LHallwayOrientation::UpRight]
    } else {
        vec![LHallwayOrientation::LeftUp, LHallwayOrientation::UpLeft]
    };

    loop {
        let orientation = orientations.pop();
        if orientation.is_none() {
            return Err(CreateHallwayError::OverlapNotEnough);
        }
        new_hallways = generate_l_shaped_hallway(
            left_room,
            right_room,
            min_hallway_width, // change this. Please honest to0 god Please change this
            max_hallway_width,
            &orientation.unwrap(),
        );

        // if the order of initial rooms were flipped, we need to flip the order of new hallways, or else they will be connected in a wrong way later on
        if flipped {
            new_hallways.reverse()
        }

        let mut overlapping_rooms = vec![];
        for hallway in &new_hallways {
            for room in map.rooms.values() {
                if is_overlapping(hallway, room) {
                    //println!("overlapping room: {}. between {} and {}", room.id(), room1.id(), room2.id() );
                    if room.id() != room1.id()
                        && room.id() != room2.id()
                        && !overlapping_rooms.contains(&room.id())
                    {
                        overlapping_rooms.push(room.id());
                    }
                }
            }
        }
        if !overlapping_rooms.is_empty() {
            /* println!(
                "overlapping rooms: {:?}. between {} and {}",
                overlapping_rooms,
                room1.id(),
                room2.id()
            ); */
            return Err(CreateHallwayError::CouldNotMakeLShapedHallway((
                room1.id(),
                overlapping_rooms.first().unwrap().clone(),
                room2.id(),
            )));
        }
        return Ok(new_hallways.into());
    }
}

/// creates a dimensions of a new room that can connect two other rooms.
fn generate_straight_hallway(
    room1: &impl StructureDimensions,
    room2: &impl StructureDimensions,
    overlap: u32,
    max_hallway_width: u32,
    orientation: &DoorOrientation,
    threshold: u32,
) -> RoomDimensions {
    let hallway_anchor;
    let hallway_length;

    match orientation {
        // if orientation == vertical, the door is | shaped. so we need to care about X coordinates
        DoorOrientation::Vertical => {
            let (_, bl_room, tr_room) =
                maybe_flip_bl_tr(room1.anchor_grid().x < room2.anchor_grid().x, room1, room2);

            hallway_anchor = IVec2::new(
                bl_room.anchor_grid_end().x,
                (bl_room.anchor_grid().y).max(tr_room.anchor_grid().y),
            );
            hallway_length = (tr_room.anchor_grid().x - bl_room.anchor_grid_end().x).max(1);

            let mut hallway_width = overlap.min(max_hallway_width);
            let hallway_width = overlap;
            /* if hallway_width >= threshold {
                hallway_width = bl_room.height().min(tr_room.height()) as u32
            } */

            //println!("{hallway_width}");

            return RoomDimensions {
                anchor: hallway_anchor,
                height: hallway_width as usize,
                length: hallway_length as usize,
            };
        }
        // if orientation == horizontal, the door is _ shaped. so we need to care about Y coordinates
        DoorOrientation::Horziontal => {
            let (_, bl_room, tr_room) =
                maybe_flip_bl_tr(room1.anchor_grid().y < room2.anchor_grid().y, room1, room2);

            hallway_anchor = IVec2::new(
                (bl_room.anchor_grid().x).max(tr_room.anchor_grid().x),
                bl_room.anchor_grid_end().y,
            );
            hallway_length = (tr_room.anchor_grid().y - bl_room.anchor_grid_end().y).max(1);

            let mut hallway_width = overlap.min(max_hallway_width);
            let hallway_width = overlap;
            /* if hallway_width >= threshold {
                hallway_width = bl_room.length().min(tr_room.length()) as u32
            } */

            return RoomDimensions {
                anchor: hallway_anchor,
                height: hallway_length as usize,
                length: hallway_width as usize,
            };
        }
    }
}

/// tries to create an L shaped hallway between room1 and room2.
/// using the LHallwayOrientation provided
fn generate_l_shaped_hallway(
    left_room: &dyn StructureDimensions,
    right_room: &dyn StructureDimensions,
    min_hallway_width: u32,
    max_hallway_width: u32,
    orientation: &LHallwayOrientation,
) -> Vec<RoomDimensions> {
    let mut new_rooms = vec![];
    let hallway_width = max_hallway_width;

    type LHO = LHallwayOrientation;
    let (middle_room_anchor, orientation1, orientation2) = match orientation {
        LHO::RightUp | LHO::UpLeft => {
            let middle_room_anchor = IVec2::new(
                (right_room.center_grid().x - hallway_width as f32 / 2.) as i32,
                (left_room.center_grid().y - hallway_width as f32 / 2.) as i32,
            );
            let orientation1 = DoorOrientation::Vertical;
            let orientation2 = DoorOrientation::Horziontal;
            (middle_room_anchor, orientation1, orientation2)
        }
        LHO::UpRight | LHO::LeftUp => {
            let middle_room_anchor = IVec2::new(
                (left_room.center_grid().x - hallway_width as f32 / 2.) as i32,
                (right_room.center_grid().y - hallway_width as f32 / 2.) as i32,
            );
            let orientation1 = DoorOrientation::Horziontal;
            let orientation2 = DoorOrientation::Vertical;
            (middle_room_anchor, orientation1, orientation2)
        }
    };

    let middle_room = RoomDimensions {
        anchor: middle_room_anchor,
        height: hallway_width as usize,
        length: hallway_width as usize,
    };
    // we have to "remake" the room dimensions due to rusts type system preventing me from just passing in a dyn trait into an impl trait parameter
    let left_room = RoomDimensions::from_dyn_structure_dim(left_room);
    let right_room = RoomDimensions::from_dyn_structure_dim(right_room);

    let hallway_a = generate_straight_hallway(
        &left_room,
        &middle_room,
        hallway_width,
        max_hallway_width,
        &orientation1,
        9999,
    );
    let hallway_b = generate_straight_hallway(
        &middle_room,
        &right_room,
        hallway_width,
        max_hallway_width,
        &orientation2,
        9999,
    );

    new_rooms.push(hallway_a);
    new_rooms.push(middle_room);
    new_rooms.push(hallway_b);

    //dbg!("{:?}", new_rooms.clone());
    new_rooms
}

fn turn_dimensions_into_room(input: &impl StructureDimensions, map: &mut MapResource) -> Room {
    Room::new(
        map.next_room_id(),
        input.length(),
        input.height(),
        input.anchor_grid(),
        false,
    )
}

/// create an AdjacentTiles object which contains tiles coordinates from both rooms.
/// the tiles in room1 are next to the tiles in room2
fn find_out_adjacent_tiles(
    room1: &impl Structure,
    room2: &impl Structure,
    map: &MapArea,
    orientation: DoorOrientation,
) -> AdjacentTiles {
    let mut tiles_room1 = vec![];
    let mut tiles_room2 = vec![];

    let bl_room; // bottom-left room
    let tr_room; // top-right room
    let flipped; // were room1 and room2 flipped? sometimes room2 is further bottom/left than room1 and to keep the algorithm cases simpler theyre swapped. this var keeps track if swapping happened
    let bl_anchor; // anchor point of bottom-left room
    let start; // start coordinate for for loop
    let end; // end coordinate for for loop

    match orientation {
        DoorOrientation::Vertical => {
            (flipped, bl_room, tr_room) =
                maybe_flip_bl_tr(room1.anchor_grid().x < room2.anchor_grid().x, room1, room2);
            bl_anchor = IVec2::new(bl_room.anchor_grid_end().x - 1, bl_room.anchor_grid().y);
            start = (bl_anchor.y).max(tr_room.anchor_grid().y);
            end = (tr_room.anchor_grid_end().y).min(bl_room.anchor_grid_end().y);
        }
        DoorOrientation::Horziontal => {
            (flipped, bl_room, tr_room) =
                maybe_flip_bl_tr(room1.anchor_grid().y < room2.anchor_grid().y, room1, room2);
            bl_anchor = IVec2::new(bl_room.anchor_grid().x, bl_room.anchor_grid_end().y - 1);
            start = (bl_anchor.x).max(tr_room.anchor_grid().x);
            end = (tr_room.anchor_grid_end().x).min(bl_room.anchor_grid_end().x);
        }
    }
    for i in start..end {
        // find out concrete points for both rooms to look at
        let (point_r1, point_r2) = match orientation {
            DoorOrientation::Vertical => (
                (bl_anchor.x as i32, i as i32),
                ((bl_anchor.x + 1) as i32, i as i32),
            ),
            DoorOrientation::Horziontal => (
                (i as i32, bl_anchor.y as i32),
                (i as i32, (bl_anchor.y + 1) as i32),
            ),
        };

        // do the two rooms contain their respective point?
        let exists_r1 = map.point_to_room(point_r1);
        let exists_r2 = map.point_to_room(point_r2);

        // if both rooms contain their point, the tiles are adjacent.
        if exists_r1.is_some() && exists_r2.is_some() {
            // convert global point position to local, respective to each room. if a local position can't be made, something went wrong, so abort.
            let Some(local_r1) = bl_room.global_to_local(IVec2::new(point_r1.0, point_r1.1)) else {
                continue;
            };
            let Some(local_r2) = tr_room.global_to_local(IVec2::new(point_r2.0, point_r2.1)) else {
                continue;
            };

            // we might have flipped the room order, so to apply the correct tiles to the correct room,
            // we have to check whether the tiles 'belong' to the correct room
            if !flipped {
                tiles_room1.push(local_r1);
                tiles_room2.push(local_r2);
            } else {
                tiles_room1.push(local_r2);
                tiles_room2.push(local_r1);
            }
        }
    }

    AdjacentTiles {
        room1: (room1.id(), tiles_room1.into()),
        room2: (room2.id(), tiles_room2.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjacent() {
        // 1|2 x
        let room1 = Room::_new_only_spatial(5, 5, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(5, 5, IVec2::new(5, 0));
        let result = are_two_rooms_adjacent(&room1, &room2, 0);
        assert!(result.is_some());
        assert!(result.unwrap().0 == 5);
        assert!(result.unwrap().1 == DoorOrientation::Vertical);

        // 2
        // 1 y
        let room1 = Room::_new_only_spatial(5, 5, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(5, 5, IVec2::new(0, 5));
        let result = are_two_rooms_adjacent(&room1, &room2, 0);
        assert!(result.is_some());
        assert!(result.unwrap().0 == 5);
        assert!(result.unwrap().1 == DoorOrientation::Horziontal);

        // 2|1 x
        let room1 = Room::_new_only_spatial(5, 5, IVec2::new(-5, 0));
        let room2 = Room::_new_only_spatial(5, 5, IVec2::new(0, 0));
        let result = are_two_rooms_adjacent(&room1, &room2, 0);
        assert!(result.is_some());
        assert!(result.unwrap().0 == 5);
        assert!(result.unwrap().1 == DoorOrientation::Vertical);

        // 1
        // 2 y
        let room1 = Room::_new_only_spatial(2, 2, IVec2::new(2, 0));
        let room2 = Room::_new_only_spatial(2, 2, IVec2::new(0, 0));
        let result = are_two_rooms_adjacent(&room1, &room2, 0);
        assert!(result.is_some());
        assert!(result.unwrap().0 == 2);
        assert!(result.unwrap().1 == DoorOrientation::Vertical);
    }

    #[test]
    fn test_not_adjacent() {
        let room1 = Room::_new_only_spatial(5, 5, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(5, 5, IVec2::new(6, 0));
        assert!(are_two_rooms_adjacent(&room1, &room2, 0).is_none());
        let room1 = Room::_new_only_spatial(5, 5, IVec2::new(0, 3));
        let room2 = Room::_new_only_spatial(5, 5, IVec2::new(6, 0));
        assert!(are_two_rooms_adjacent(&room1, &room2, 0).is_none());
        let room1 = Room::_new_only_spatial(5, 5, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(5, 5, IVec2::new(-5, -5));
        assert!(are_two_rooms_adjacent(&room1, &room2, 0).is_none());
        let room1 = Room::_new_only_spatial(5, 5, IVec2::new(6, 0));
        let room2 = Room::_new_only_spatial(5, 5, IVec2::new(0, 0));
        assert!(are_two_rooms_adjacent(&room1, &room2, 0).is_none());
        let room1 = Room::_new_only_spatial(2, 2, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(2, 2, IVec2::new(2, 2));
        assert!(are_two_rooms_adjacent(&room1, &room2, 0).is_none());
        let room1 = Room::_new_only_spatial(2, 2, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(2, 2, IVec2::new(-2, -2));
        assert!(are_two_rooms_adjacent(&room1, &room2, 0).is_none());
        let room1 = Room::_new_only_spatial(2, 2, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(2, 2, IVec2::new(-2, 2));
        assert!(are_two_rooms_adjacent(&room1, &room2, 0).is_none());
        let room1 = Room::_new_only_spatial(2, 2, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(2, 2, IVec2::new(-2, 2));
        assert!(are_two_rooms_adjacent(&room1, &room2, 0).is_none());
    }

    #[test]
    fn test_line_overlap() {
        // success cases
        let a1 = 0;
        let a2 = 10;
        let b1 = 5;
        let b2 = 15;
        assert!(line_overlap(a1, a2, b1, b2) == 5);
        let a1 = 0;
        let a2 = 10;
        let b1 = -10;
        let b2 = 5;
        assert!(line_overlap(a1, a2, b1, b2) == 5);

        let a1 = 10;
        let a2 = 20;
        let b1 = -10;
        let b2 = 13;
        assert!(line_overlap(a1, a2, b1, b2) == 3);
        let a1 = 10;
        let a2 = 20;
        let b1 = 3;
        let b2 = 23;
        let x = line_overlap(a1, a2, b1, b2);
        assert!(x == 10);

        // fail cases
        let a1 = 0;
        let a2 = 10;
        let b1 = 15;
        let b2 = 25;
        assert!(line_overlap(a1, a2, b1, b2) == -5);
        let a1 = 10;
        let a2 = 20;
        let b1 = -5;
        let b2 = 5;
        assert!(line_overlap(a1, a2, b1, b2) == -5);
    }

    #[test]
    fn test_common_edge() {
        let room1 = Room::_new_only_spatial(5, 5, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(5, 5, IVec2::new(5, 0));
        let (x, y) = common_edge(&room1, &room2);
        assert!(x == 0);
        assert!(y == 5);

        let room1 = Room::_new_only_spatial(5, 5, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(5, 5, IVec2::new(10, 0));
        let (x, y) = common_edge(&room1, &room2);
        assert!(x == -5);
        assert!(y == 5);

        let room1 = Room::_new_only_spatial(5, 5, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(5, 5, IVec2::new(-5, 3));
        let (x, y) = common_edge(&room1, &room2);
        assert!(x == 0);
        assert!(y == 2);

        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(-25, 0));
        let room2 = Room::_new_only_spatial(10, 10, IVec2::new(-36, 3));
        let (x, y) = common_edge(&room1, &room2);
        println!("{x}, {y}");
        assert!(x == -1);
        assert!(y == 7);
    }

    /* #[test]
    fn test_reduce_connections() {
        let connections = vec![
            RoomConnection {
                room1_id: 0,
                room2_id: 1,
                connection_type: RoomConnectionType::Adjacent(AdjacentTiles::default()),
            },
            RoomConnection {
                room1_id: 1,
                room2_id: 2,
                connection_type: RoomConnectionType,
            },
        ];
    } */

    /* #[test]
    fn test_adjacent_tiles() {
        let room1 = Room::new(0, 2, 2, IVec2::new(0, 0), true);
        let room2 = Room::new(1, 2, 2, IVec2::new(2, 0), true);
        let map_area = MapArea {
            rooms: vec![room1.clone(), room2.clone()],
            triangulation: None,
            graph: None,
            connections: None,
        };
        let adjacent_tiles_ex = AdjacentTiles {
            room1: (room1.id(), vec![UVec2::new(1, 0), UVec2::new(1, 1)].into()),
            room2: (room2.id(), vec![UVec2::new(0, 0), UVec2::new(0, 1)].into()),
        };
        let adjacent_tiles_gen =
            find_out_adjacent_tiles(&room1, &room2, &map_area, DoorOrientation::Vertical);

        println!("{:?}", adjacent_tiles_gen);
        assert!(adjacent_tiles_ex == adjacent_tiles_gen);

        // case: vertical
        let room1 = Room::new(0, 5, 5, IVec2::new(0, 0), true);
        let room2 = Room::new(1, 5, 5, IVec2::new(5, 3), true);
        let map_area = MapArea {
            rooms: vec![room1.clone(), room2.clone()],
            triangulation: None,
            graph: None,
            connections: None,
            initial_connections: Vec::new(),
        };
        let adjacent_tiles_ex = AdjacentTiles {
            room1: (room1.id(), vec![UVec2::new(4, 3), UVec2::new(4, 4)].into()),
            room2: (room2.id(), vec![UVec2::new(0, 0), UVec2::new(0, 1)].into()),
        };
        let adjacent_tiles_gen =
            find_out_adjacent_tiles(&room1, &room2, &map_area, DoorOrientation::Vertical);

        println!("{:?}", adjacent_tiles_gen);
        assert!(adjacent_tiles_ex == adjacent_tiles_gen);

        // case: horizontal
        let room1 = Room::new(0, 5, 5, IVec2::new(0, 0), true);
        let room2 = Room::new(1, 5, 5, IVec2::new(3, 5), true);
        let map_area = MapArea {
            rooms: vec![room1.clone(), room2.clone()],
            triangulation: None,
            graph: None,
            connections: None,
        };
        let adjacent_tiles_ex = AdjacentTiles {
            room1: (room1.id(), vec![UVec2::new(3, 4), UVec2::new(4, 4)].into()),
            room2: (room2.id(), vec![UVec2::new(0, 0), UVec2::new(1, 0)].into()),
        };
        let adjacent_tiles_gen =
            find_out_adjacent_tiles(&room1, &room2, &map_area, DoorOrientation::Horziontal);
        assert!(adjacent_tiles_ex == adjacent_tiles_gen);

        // case: horizontal && negative position
        let room1 = Room::new(0, 5, 5, IVec2::new(-2, -5), true);
        let room2 = Room::new(1, 5, 5, IVec2::new(1, 0), true);
        let map_area = MapArea {
            rooms: vec![room1.clone(), room2.clone()],
            triangulation: None,
            graph: None,
            connections: None,
        };
        let adjacent_tiles_ex = AdjacentTiles {
            room1: (room1.id(), vec![UVec2::new(3, 4), UVec2::new(4, 4)].into()),
            room2: (room2.id(), vec![UVec2::new(0, 0), UVec2::new(1, 0)].into()),
        };
        let adjacent_tiles_gen =
            find_out_adjacent_tiles(&room1, &room2, &map_area, DoorOrientation::Horziontal);
        assert!(adjacent_tiles_ex == adjacent_tiles_gen);

        // case: vertical && negative position
        /* let room1 = Room::new(0, 30, 30, RoomShape::Rect, IVec2::new(0, 0), true);
        let room2 = Room::new(1, 10, 10, RoomShape::Rect, IVec2::new(-10, 15), true);
        let map_area = MapArea {
            rooms: vec![room1.clone(), room2.clone()],
            triangulation: None,
            graph: None,
        };
        let adjacent_tiles_ex = AdjacentTiles {
            room1: (room1.id(), vec![UVec2::new(3, 4), UVec2::new(4, 4)].into()),
            room2: (room1.id(), vec![UVec2::new(0, 0), UVec2::new(1, 0)].into()),
        };
        let adjacent_tiles_gen =
            find_out_adjacent_tiles(&room1, &room2, &map_area, DoorOrientation::Vertical);
        assert!(adjacent_tiles_ex == adjacent_tiles_gen); */
    }

    #[test]
    fn test_create_hallway_dimensions() {
        let worldgen = WorldgenSettings::default();
        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(10, 10, IVec2::new(20, 0));
        let map_area = MapArea {
            rooms: vec![room1.clone(), room2.clone()],
            triangulation: None,
            graph: None,
            connections: None,
        };
        let r = create_hallway_dimensions(&room1, &room2, 4, 9999, &map_area, &worldgen).unwrap();
        assert!(r.len() == 1);
        assert!(r[0].anchor == IVec2::new(10, 0));

        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(10, 10, IVec2::new(0, 20));
        let map_area = MapArea {
            rooms: vec![room1.clone(), room2.clone()],
            triangulation: None,
            graph: None,
            connections: None,
        };
        let r = create_hallway_dimensions(&room1, &room2, 4, 9999, &map_area, &worldgen).unwrap();
        assert!(r.len() == 1);
        assert!(r[0].anchor == IVec2::new(0, 10));

        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(10, 10, IVec2::new(-20, 0));
        let map_area = MapArea {
            rooms: vec![room1.clone(), room2.clone()],
            triangulation: None,
            graph: None,
            connections: None,
        };
        let r = create_hallway_dimensions(&room1, &room2, 4, 9999, &map_area, &worldgen).unwrap();
        assert!(r.len() == 1);
        assert!(r[0].anchor == IVec2::new(-10, 0));

        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(10, 10, IVec2::new(0, -20));
        let map_area = MapArea {
            rooms: vec![room1.clone(), room2.clone()],
            triangulation: None,
            graph: None,
            connections: None,
        };
        let r = create_hallway_dimensions(&room1, &room2, 4, 9999, &map_area, &worldgen).unwrap();
        assert!(r.len() == 1);
        assert!(r[0].anchor == IVec2::new(0, -10));

        //assert!(false);
    } */
}
