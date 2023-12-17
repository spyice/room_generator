use super::room::{self, Room, StructureDimensions};
use bevy::prelude::*;
use dyn_clone::DynClone;
use grid::Grid;
use iter_num_tools::lin_space;

pub trait AesthetiziseRoom: DynClone {
    fn generate_features(&self, rooms: &mut Room, destructive: bool);
}
dyn_clone::clone_trait_object!(AesthetiziseRoom);

#[derive(Clone, Debug, serde::Deserialize)]
pub enum Aesthetics {
    Pillars(Pillars),
    CellularAutomata(CellularAutomata),
}

impl Aesthetics {
    pub fn generate_features(&self, room: &mut Room, destructive: bool) {
        match self {
            Aesthetics::Pillars(x) => x.generate_features(room, destructive),
            Aesthetics::CellularAutomata(x) => x.generate_features(room, destructive),
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Pillars {
    pub amount: usize,
    pub pillar_size: usize,
    pub generation_type: PillarGenerationType,
}
#[derive(Clone, Debug, serde::Deserialize)]
pub enum PillarGenerationType {
    Axis(Axis),
    BothAxes,
}
#[derive(Clone, Debug, serde::Deserialize)]
pub enum Axis {
    X,
    Y,
}

impl AesthetiziseRoom for Pillars {
    fn generate_features(&self, rooms: &mut Room, _destructive: bool) {
        let mut pillar_anchors: Vec<UVec2> = Vec::new();
        let mut pillar_tiles: Vec<UVec2> = Vec::new();

        let evenly_spaced_x = lin_space(0..rooms.length(), self.amount + 1);
        let evenly_spaced_y = lin_space(0..rooms.height(), self.amount + 1);

        match self.generation_type {
            PillarGenerationType::Axis(ref axis) => match axis {
                Axis::X => {
                    for i in 0..self.amount {
                        let pillar_anchor = UVec2::new(
                            evenly_spaced_x.clone().nth(i + 1).unwrap() as u32,
                            evenly_spaced_y
                                .clone()
                                .nth(fastrand::usize(0..evenly_spaced_y.len()))
                                .unwrap() as u32,
                        );
                        pillar_anchors.push(pillar_anchor);
                    }
                }
                Axis::Y => {
                    for i in 0..self.amount {
                        let pillar_anchor = UVec2::new(
                            evenly_spaced_x
                                .clone()
                                .nth(fastrand::usize(0..evenly_spaced_x.len()))
                                .unwrap() as u32,
                            evenly_spaced_y.clone().nth(i + 1).unwrap() as u32,
                        );
                        pillar_anchors.push(pillar_anchor);
                    }
                }
            },
            PillarGenerationType::BothAxes => {
                for i in 0..self.amount {
                    for j in 0..self.amount {
                        let pillar_anchor = UVec2::new(
                            evenly_spaced_x.clone().nth(i + 1).unwrap() as u32,
                            evenly_spaced_y.clone().nth(j + 1).unwrap() as u32,
                        );
                        pillar_anchors.push(pillar_anchor);
                    }
                }
            }
        }

        // fill pillar_tiles with tiles that belong to the pillar, based on the size
        for pillar_anchor in pillar_anchors.iter() {
            for i in pillar_anchor.x..pillar_anchor.x + self.pillar_size as u32 {
                for j in pillar_anchor.y..pillar_anchor.y + self.pillar_size as u32 {
                    pillar_tiles.push(UVec2::new(
                        i.saturating_sub((self.pillar_size / 2) as u32),
                        j.saturating_sub((self.pillar_size / 2) as u32),
                    ));
                }
            }
        }

        // poke walls into the room
        for tile_position in &pillar_tiles {
            rooms.set_tile(*tile_position, room::Tile::Wall);
        }
    }
}

/// modified version of https://www.roguebasin.com/index.php?title=Cellular_Automata_Method_for_Generating_Random_Cave-Like_Levels
#[derive(Clone, Debug, serde::Deserialize)]
pub struct CellularAutomata {
    pub iterations: usize,
    pub wall_percentage: f32,
}

/// this code stems from https://www.roguebasin.com/index.php?title=Cellular_Automata_Method_for_Generating_Random_Cave-Like_Levels
/// and has been adapted for rust. the semantics are the same (hopefully)
impl CellularAutomata {
    fn generate(&self, room: &mut Room, destructive: bool) {
        let grid = room.get_grid();
        let mut new_grid: Grid<bool> = Grid::new(grid.rows(), grid.cols());

        self.random_fill(&mut new_grid);

        // should the algorithm overwrite EVERYTHING that is inside the room?
        // yes -> every tile is random. no -> all WALL tiles are put back into the grid
        if !destructive {
            for y in 0..grid.rows() {
                for x in 0..grid.cols() {
                    match grid[y][x] {
                        room::Tile::Wall => new_grid[y][x] = true,
                        _ => continue,
                    };
                }
            }
        }

        for _ in 0..self.iterations {
            new_grid = self.step(&new_grid);
        }

        for y in 0..new_grid.rows() {
            for x in 0..new_grid.cols() {
                let tile = match new_grid[y][x] {
                    true => room::Tile::Wall,
                    false => room::Tile::Ground,
                };
                room.set_tile(UVec2::new(x as u32, y as u32), tile);
            }
        }
    }

    fn random_fill(&self, grid: &mut Grid<bool>) {
        let range = 4..grid.cols().saturating_sub(4);
        let random_column = if !range.is_empty() {
            fastrand::usize(range)
        } else {
            grid.cols() / 2
        };

        for y in 0..grid.rows() {
            for x in 0..grid.cols() {
                if x == 0 || y == 0 || x == grid.cols() - 1 || y == grid.rows() - 1 {
                    grid[y][x] = true;
                } else if x != random_column && fastrand::f32() < self.wall_percentage {
                    grid[y][x] = true;
                }
            }
        }
    }

    fn step(&self, grid: &Grid<bool>) -> Grid<bool> {
        let mut new_grid = Grid::new(grid.rows(), grid.cols());

        for y in 0..grid.rows() {
            for x in 0..grid.cols() {
                if x == 0 || y == 0 || x == grid.cols() - 1 || y == grid.rows() - 1 {
                    new_grid[y][x] = true;
                } else {
                    new_grid[y][x] = self.place_wall_logic(&grid, x, y);
                }
            }
        }

        new_grid
    }

    fn place_wall_logic(&self, grid: &Grid<bool>, x: usize, y: usize) -> bool {
        self.count_adjacent_walls(grid, x, y) >= 5 || self.count_nearby_walls(grid, x, y) <= 2
    }

    fn count_adjacent_walls(&self, grid: &Grid<bool>, x: usize, y: usize) -> usize {
        let mut walls: usize = 0;

        for map_x in x.saturating_sub(1)..=x + 1 {
            for map_y in y.saturating_sub(1)..=y + 1 {
                if grid.get(map_y, map_x).is_some_and(|tile| *tile) {
                    walls += 1;
                }
            }
        }
        walls
    }

    fn count_nearby_walls(&self, grid: &Grid<bool>, x: usize, y: usize) -> usize {
        let mut walls: usize = 0;

        for map_x in x.saturating_sub(2)..=x + 2 {
            for map_y in y.saturating_sub(2)..=y + 2 {
                if ((map_x as i32) - (x as i32)).abs() == 2
                    && ((map_y as i32) - (y as i32)).abs() == 2
                {
                    continue;
                }

                if grid.get(map_y, map_x).is_some_and(|tile| !*tile) {
                    walls += 1;
                }
            }
        }
        walls
    }
}

impl AesthetiziseRoom for CellularAutomata {
    fn generate_features(&self, rooms: &mut Room, destructive: bool) {
        self.generate(rooms, destructive);
    }
}
