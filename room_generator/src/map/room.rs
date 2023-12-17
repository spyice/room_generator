use bevy::prelude::*;
use grid::*;

use super::aesthetics;

pub trait StructureDimensions {
    fn height(&self) -> usize;
    fn length(&self) -> usize;
    fn anchor_grid(&self) -> IVec2;
    fn offset_anchor_grid(&mut self, offset: IVec2);

    fn center_grid(&self) -> Vec2 {
        Vec2 {
            x: self.anchor_grid().x as f32 + self.length() as f32 / 2.,
            y: self.anchor_grid().y as f32 + self.height() as f32 / 2.,
        }
    }
    fn anchor_world(&self, tile_size: UVec2) -> IVec2 {
        IVec2::new(
            self.anchor_grid().x * tile_size.x as i32,
            self.anchor_grid().y * tile_size.y as i32,
        )
    }
    fn center_world(&self, tile_size: UVec2) -> Vec2 {
        Vec2 {
            x: (self.anchor_grid().x as f32 + self.length() as f32 / 2.) * tile_size.x as f32,
            y: (self.anchor_grid().y as f32 + self.height() as f32 / 2.) * tile_size.y as f32,
        }
    }

    fn anchor_grid_end(&self) -> IVec2 {
        IVec2 {
            x: self.anchor_grid().x + self.length() as i32,
            y: self.anchor_grid().y + self.height() as i32,
        }
    }

    /// converts local position to global position
    fn local_to_global(&self, local_coordinates: UVec2) -> IVec2 {
        self.anchor_grid() + local_coordinates.as_ivec2()
    }
    /// converts global IVEC position to local UVEC position within the room. If its out of bounds, returns None
    fn global_to_local(&self, global_coordinates: IVec2) -> Option<UVec2> {
        let output = global_coordinates - self.anchor_grid();
        if output.x >= self.length() as i32 || output.y >= self.height() as i32 {
            return None;
        }
        if output.x < 0 || output.y < 0 {
            return None;
        }
        return Some(output.as_uvec2());
    }

    // other stuff
    fn is_point_inside(&self, point: (i32, i32)) -> bool {
        let x = point.0 >= self.anchor_grid().x
            && point.0 < self.anchor_grid().x + self.length() as i32;
        let y = point.1 >= self.anchor_grid().y
            && point.1 < self.anchor_grid().y + self.height() as i32;

        x && y
    }
}

impl<T> StructureDimensions for &T
where
    T: StructureDimensions,
{
    fn height(&self) -> usize {
        self.to_owned().height()
    }

    fn length(&self) -> usize {
        self.to_owned().length()
    }

    fn anchor_grid(&self) -> IVec2 {
        self.to_owned().anchor_grid()
    }

    fn offset_anchor_grid(&mut self, _offset: IVec2) {
        println!("offset anchor grid called with reference.....");
    }
}

pub trait Structure: StructureDimensions {
    fn id(&self) -> usize;
}

#[derive(Debug, Clone, Copy, Default)]
pub enum Tile {
    #[default]
    Ground,
    Wall,
}

#[derive(Debug, Clone, Copy, Reflect, PartialEq, Eq)]
pub enum DoorOrientation {
    Vertical,
    Horziontal,
}

#[derive(Debug, Clone, Copy)]
pub struct RoomDimensions {
    pub anchor: IVec2,
    pub height: usize,
    pub length: usize,
}
impl StructureDimensions for RoomDimensions {
    fn height(&self) -> usize {
        self.height
    }

    fn length(&self) -> usize {
        self.length
    }

    fn anchor_grid(&self) -> IVec2 {
        self.anchor
    }

    fn offset_anchor_grid(&mut self, offset: IVec2) {
        self.anchor += offset;
    }
}

impl RoomDimensions {
    pub fn from_dyn_structure_dim(t: &dyn StructureDimensions) -> Self {
        Self {
            anchor: t.anchor_grid(),
            height: t.height(),
            length: t.length(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum RoomType {
    Normal,
    Shop,
    Boss,
}

#[derive(Clone, Debug)]
pub struct RoomDetails {
    pub is_main: bool,
    pub room_type: RoomType,
    pub aesthetic_modifiers: Vec<aesthetics::Aesthetics>,
}

#[derive(Debug, Clone, Component)]
pub struct Room {
    room_id: usize,
    data: Grid<Tile>,
    world_pos: IVec2,
    pub details: RoomDetails,
    pub is_position_fixed: bool,
    pub is_visible: bool,
}
impl Room {
    pub fn new(
        room_id: usize,
        length: usize,
        height: usize,
        world_pos: IVec2,
        is_main: bool,
    ) -> Self {
        Self {
            room_id,
            data: grid::Grid::new(height, length),
            world_pos,
            details: RoomDetails {
                is_main,
                room_type: RoomType::Normal,
                aesthetic_modifiers: Vec::new(),
            },
            is_position_fixed: false,
            is_visible: true,
        }
    }
    // only used for unit tests
    pub fn _new_only_spatial(length: usize, height: usize, world_pos: IVec2) -> Self {
        Self {
            room_id: fastrand::usize(0..=10000),
            data: grid::Grid::new(height, length),
            world_pos,
            details: RoomDetails {
                is_main: true,
                room_type: RoomType::Normal,
                aesthetic_modifiers: Vec::new(),
            },
            is_position_fixed: false,
            is_visible: true,
        }
    }
    /* pub fn new_from_dimensions(r_dim: impl StructureDimensions, map: &mut MapResource) -> Room {
        Room::new(
            map.next_room_id(),
            r_dim.length(),
            r_dim.height(),
            RoomShape::Rect,
            r_dim.anchor_grid(),
            false,
        )
    } */

    pub fn new2(
        room_id: usize,
        dimensions: impl StructureDimensions,
        details: RoomDetails,
    ) -> Self {
        Self {
            room_id,
            data: grid::Grid::new(dimensions.height(), dimensions.length()),
            world_pos: dimensions.anchor_grid(),
            details,
            is_position_fixed: false,
            is_visible: true,
        }
    }

    #[inline(always)]
    pub fn id(&self) -> usize {
        self.room_id
    }

    #[inline(always)]
    pub fn get_tile(&self, position: UVec2) -> Option<&Tile> {
        self.data.get(position.y as usize, position.x as usize)
    }
    /// sets the tile to a new Tile. if the tile doesnt exist (position doesnt apply to this tile), then false is returned. else true
    #[inline(always)]
    pub fn set_tile(&mut self, position: UVec2, tile: Tile) -> bool {
        let t = self.data.get_mut(position.y as usize, position.x as usize);
        if t.is_none() {
            return false;
        }
        *t.unwrap() = tile;
        true
    }

    #[inline(always)]
    pub fn get_area_grid(&self) -> usize {
        self.length() * self.height()
    }
    #[inline(always)]
    pub fn _get_area_world(&self, tile_size: UVec2) -> usize {
        self.length() * tile_size.x as usize * self.height() * tile_size.y as usize
    }

    pub fn fill_edges(&mut self) {
        /* match self.shape {
            RoomShape::Rect => {
                fill_rect(self);
            }
            RoomShape::Circle => {
                fill_circle(self);
            }
        } */
        fill_rect(self);
    }

    // returns true if door was added to the room successfully, and false if internal checks gave an error
    /* pub fn _add_door(&mut self, door: &Door) -> bool {
        //TODO: add validation
        self.doors.push(door.clone());
        true
    }
    pub fn get_doors(&self) -> &Vec<Door> {
        &self.doors
    } */

    pub fn get_grid(&self) -> Grid<Tile> {
        self.data.clone()
    }
}

impl StructureDimensions for Room {
    fn height(&self) -> usize {
        self.data.rows()
    }

    fn length(&self) -> usize {
        self.data.cols()
    }

    fn anchor_grid(&self) -> IVec2 {
        self.world_pos
    }

    fn offset_anchor_grid(&mut self, offset: IVec2) {
        self.world_pos += offset;
    }
}
impl Structure for Room {
    fn id(&self) -> usize {
        self.room_id
    }
}
/* pub trait RoomDescriptor {
    fn get_details(&self) -> RoomDetails;
}

pub struct  */

#[derive(Debug)]
pub struct RoomWithDetailsNoId {
    pub dimensions: RoomDimensions,
    pub details: RoomDetails,
}
impl StructureDimensions for RoomWithDetailsNoId {
    fn height(&self) -> usize {
        self.dimensions.height()
    }

    fn length(&self) -> usize {
        self.dimensions.length()
    }

    fn anchor_grid(&self) -> IVec2 {
        self.dimensions.anchor_grid()
    }

    fn offset_anchor_grid(&mut self, offset: IVec2) {
        self.dimensions.offset_anchor_grid(offset)
    }
}

#[derive(Deref, DerefMut, Debug, Clone)]
pub struct StructureCollection<T> {
    structures: Vec<T>,
}
impl<T> StructureCollection<T>
where
    T: StructureDimensions,
{
    pub fn new(room_dims: impl Into<Vec<T>>) -> Self {
        Self {
            structures: room_dims.into(),
        }
    }
    fn min_x(&self) -> i32 {
        self.structures
            .iter()
            .map(|t| t.anchor_grid().x)
            .min()
            .unwrap()
    }
    fn min_y(&self) -> i32 {
        self.structures
            .iter()
            .map(|t| t.anchor_grid().y)
            .min()
            .unwrap()
    }
    fn max_x(&self) -> i32 {
        self.structures
            .iter()
            .map(|t| t.anchor_grid_end().x)
            .max()
            .unwrap()
    }
    fn max_y(&self) -> i32 {
        self.structures
            .iter()
            .map(|t| t.anchor_grid_end().y)
            .max()
            .unwrap()
    }
}
impl<T> StructureDimensions for StructureCollection<T>
where
    T: StructureDimensions,
{
    fn height(&self) -> usize {
        (self.max_y() - self.min_y()).abs() as usize
    }

    fn length(&self) -> usize {
        (self.max_x() - self.min_x()).abs() as usize
    }

    fn anchor_grid(&self) -> IVec2 {
        IVec2 {
            x: self.min_x(),
            y: self.min_y(),
        }
    }

    fn offset_anchor_grid(&mut self, offset: IVec2) {
        self.structures.iter_mut().for_each(|r| {
            r.offset_anchor_grid(offset);
        });
    }
}

// source: https://stackoverflow.com/questions/306316/determine-if-two-rectangles-overlap-each-other
/// does this structure overlap another structure?
pub fn is_overlapping<T: StructureDimensions, U: StructureDimensions>(a: T, b: U) -> bool {
    let a_x1 = a.anchor_grid().x;
    let a_x2 = a.anchor_grid().x + a.length() as i32;
    let a_y1 = a.anchor_grid().y;
    let a_y2 = a.anchor_grid().y + a.height() as i32;
    let b_x1 = b.anchor_grid().x;
    let b_x2 = b.anchor_grid().x + b.length() as i32;
    let b_y1 = b.anchor_grid().y;
    let b_y2 = b.anchor_grid().y + b.height() as i32;

    if a_x1 < b_x2 && a_x2 > b_x1 && a_y1 < b_y2 && a_y2 > b_y1 {
        return true;
    }
    false
}

/// distance (in grid coordinates) of this room to another room
pub fn distance_between_structures<T: StructureDimensions, U: StructureDimensions>(
    a: T,
    b: U,
) -> f32 {
    let a = a.center_grid();
    let b = b.center_grid();
    let dist = Vec2::distance(a, b);
    dist
}

fn fill_rect(room: &mut Room) {
    room.data.iter_col_mut(0).for_each(|x| *x = Tile::Wall);
    room.data
        .iter_col_mut(room.length() - 1)
        .for_each(|x| *x = Tile::Wall);
    room.data.iter_row_mut(0).for_each(|x| *x = Tile::Wall);
    room.data
        .iter_row_mut(room.height() - 1)
        .for_each(|x| *x = Tile::Wall);
    //room.data.
}
fn _fill_circle(room: &mut Room) {
    let room_center = Vec2::new(
        room.length() as f32 / 2. - 0.5,
        room.height() as f32 / 2. - 0.5,
    );
    for y in 0..room.data.rows() {
        for x in 0..room.data.cols() {
            let max_radius = ((room.length().min(room.height()) as f32 / 2.) - 1.5).max(0.);
            if Vec2::distance(Vec2::new(x as f32, y as f32), room_center) > max_radius {
                let tile = room.data.get_mut(y, x).unwrap();
                *tile = Tile::Wall;
            }
        }
    }
    let tile = room
        .data
        .get_mut(room_center.y as usize, room_center.x as usize)
        .unwrap();
    *tile = Tile::Wall;
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn rooms_are_overlapping() {
        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(10, 10, IVec2::new(6, 6));
        assert!(is_overlapping(&room1, &room2));

        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(10, 10, IVec2::new(-8, -8));
        assert!(is_overlapping(&room1, &room2));

        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let room2 = Room::_new_only_spatial(10, 10, IVec2::new(12, 12));
        assert!(!is_overlapping(&room1, &room2));
    }

    #[test]
    fn test_is_point_inside() {
        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let point = (1, 1);
        assert!(room1.is_point_inside(point));

        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let point = (9, 9);
        assert!(room1.is_point_inside(point));

        let room1 = Room::_new_only_spatial(20, 20, IVec2::new(0, 0));
        let point = (16, 16);
        assert!(room1.is_point_inside(point));

        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(-10, -10));
        let point = (-10, -1);
        assert!(room1.is_point_inside(point));

        let room1 = Room::_new_only_spatial(8, 8, IVec2::new(-10, -10));
        let point = (-7, -10);
        assert!(room1.is_point_inside(point));

        // fail cases

        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let point = (25, 0);
        assert!(!room1.is_point_inside(point));

        let room1 = Room::_new_only_spatial(26, 26, IVec2::new(-2, -1));
        let point = (25, 25);
        assert!(!room1.is_point_inside(point));

        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let point = (-10, 25);
        assert!(!room1.is_point_inside(point));

        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, -9));
        let point = (0, -10);
        assert!(!room1.is_point_inside(point));

        let room1 = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let point = (10, 0);
        assert!(!room1.is_point_inside(point));
    }

    #[test]
    fn door_validation() {}

    #[test]
    fn test_global_to_local() {
        let room = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let global_coordinates = IVec2::new(0, 0);
        assert!(room.global_to_local(global_coordinates).unwrap() == UVec2::new(0, 0));

        let room = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let global_coordinates = IVec2::new(9, 9);
        assert!(room.global_to_local(global_coordinates).unwrap() == UVec2::new(9, 9));

        let room = Room::_new_only_spatial(10, 10, IVec2::new(0, 0));
        let global_coordinates = IVec2::new(15, 15);
        assert!(room.global_to_local(global_coordinates).is_none());

        let room = Room::_new_only_spatial(10, 10, IVec2::new(-5, -5));
        let global_coordinates = IVec2::new(2, 3);
        assert!(room.global_to_local(global_coordinates).unwrap() == UVec2::new(7, 8));

        let room = Room::_new_only_spatial(10, 10, IVec2::new(-5, -5));
        let global_coordinates = IVec2::new(2, 5);
        assert!(room.global_to_local(global_coordinates).is_none());

        // with negative room position

        let room = Room::_new_only_spatial(10, 10, IVec2::new(-13, -13));
        let global_coordinates = IVec2::new(-10, -10);
        assert!(room.global_to_local(global_coordinates).unwrap() == UVec2::new(3, 3));

        let room = Room::_new_only_spatial(10, 10, IVec2::new(-5, -5));
        let global_coordinates = IVec2::new(-6, -5);
        assert!(room.global_to_local(global_coordinates).is_none());

        let room = Room::_new_only_spatial(10, 10, IVec2::new(-5, -5));
        let global_coordinates = IVec2::new(2, 5);
        assert!(room.global_to_local(global_coordinates).is_none());
    }
}
