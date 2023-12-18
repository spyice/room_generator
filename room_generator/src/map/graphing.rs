use bevy::prelude::*;
use itertools::Itertools;
use ordered_float::OrderedFloat;
use petgraph::{algo::min_spanning_tree, data::FromElements, prelude::UnGraphMap};

use super::{
    generation::{MapArea, MapResource, WorldgenRng, WorldgenSettings},
    room::{distance_between_structures, StructureDimensions},
};
use delaunator::{Point, Triangulation};

#[derive(Debug, Clone, Default, Deref, DerefMut)]
pub struct MyGraph(UnGraphMap<usize, f32>);

#[derive(Debug, Clone)]
pub struct RoomGraph {
    pub mst: MyGraph,
    pub reassembled_graph: MyGraph,
    pub main_path_rooms: Vec<usize>,
}

pub fn get_triangulation(mut map: ResMut<MapResource>) {
    let main_rooms = map.map_area_mut().get_main_rooms();
    let triangulation = triangulate(&main_rooms);
    //println!("{}", triangulation.len());
    map.map_area_mut().triangulation = Some(triangulation);
}

pub fn make_graphs(
    mut map: ResMut<MapResource>,
    worldgen: Res<WorldgenSettings>,
    mut rng: ResMut<WorldgenRng>,
) {
    if map.map_area().triangulation.is_none() {
        panic!("can't make graph with empty triangulation")
    }

    let rooms = map.map_area().get_main_rooms();
    let mut graph = MyGraph::default();

    // if the triangulation has no elements, check the hull
    // (edge case with rooms on one axis, or with only 2 rooms)
    if map.map_area().triangulation.as_ref().unwrap().is_empty() {
        for (&a, &b) in map
            .map_area()
            .triangulation
            .as_ref()
            .unwrap()
            .hull
            .iter()
            .tuple_windows()
        {
            let room1 = *rooms.get(a).unwrap();
            let room2 = *rooms.get(b).unwrap();
            let distance1 = distance_between_structures(room1, room2);
            graph.add_edge(room1.id(), room2.id(), distance1);
        }
    }

    // if the triangulation is not empty...
    // for every triangle in the triangulation...
    for (&a, &b, &c) in map
        .map_area()
        .triangulation
        .as_ref()
        .unwrap()
        .triangles
        .iter()
        .tuples()
    {
        // there are three rooms part of the triangulation
        let room1 = *rooms.get(a).unwrap();
        let room2 = *rooms.get(b).unwrap();
        let room3 = *rooms.get(c).unwrap();
        let distance1 = distance_between_structures(room1, room2);
        let distance2 = distance_between_structures(room2, room3);
        let distance3 = distance_between_structures(room1, room3);
        // add 1-2, 2-3, 3-1 to the graph with the distance between the two rooms as weight
        graph.add_edge(room1.id(), room2.id(), distance1);
        graph.add_edge(room2.id(), room3.id(), distance2);
        graph.add_edge(room1.id(), room3.id(), distance3);
    }

    // add initial_connections
    for (a, b) in &map.map_area().initial_connections {
        let room1 = *rooms.get(*a).unwrap();
        let room2 = *rooms.get(*b).unwrap();
        let distance1 = distance_between_structures(room1, room2);
        graph.add_edge(room1.id(), room2.id(), distance1);
    }

    // minimum spanning tree of the graph
    let mst = MyGraph(UnGraphMap::<_, _>::from_elements(min_spanning_tree(
        &graph.0,
    )));

    let (start_room, end_room) = rooms_with_longest_distance_between_them(&mst, map.map_area());
    let path_rooms = create_path_between_two_rooms(&mst, start_room, end_room);

    let room_graph = RoomGraph {
        mst: mst.clone(),
        reassembled_graph: reassemble_graph(
            &mst,
            &graph,
            &map.map_area().triangulation.as_ref().unwrap(),
            Some(&path_rooms),
            worldgen.graph_reassembly_percentage,
            &mut rng,
            &map.map_area()
        ),
        main_path_rooms: path_rooms,
    };

    map.map_area_mut().graph = Some(room_graph);
}

/// creates a delaunay triangulation from all passed in structures
fn triangulate(rooms: &[&impl StructureDimensions]) -> Triangulation {
    let mut points = Vec::with_capacity(rooms.len());
    for &room in rooms.iter() {
        let coords = room.center_grid();
        points.push(Point {
            x: coords.x as f64,
            y: coords.y as f64,
        })
    }
    delaunator::triangulate(&points)
}

/// this function takes a look at at all the main rooms which are edge rooms (only have one connection)
/// and finds the two rooms that are the LONGEST distance apart from each other.
/// better way: with pathfinding, but not implemented here
fn rooms_with_longest_distance_between_them(mst: &MyGraph, map: &MapArea) -> (usize, usize) {
    // edge_rooms are only the rooms which have ONE connection to another room
    let edge_rooms = map
        .get_main_rooms()
        .iter()
        .filter(|&&room| {
            // get edge count for room
            let edges = mst.edges(room.id());
            // if only one edge, then edge room. else false -> filtered out of the list
            edges.into_iter().count() == 1
        })
        .cloned()
        .collect_vec();

    // initialize with some random values that will never be reached.
    // type = (0: room1_id, 1: room2_id, 2: distance between r1 and r2)
    let mut longest_distance_rooms: (usize, usize, f32) = (9999, 9999, -9999.);
    for (&room1, &room2) in edge_rooms.iter().tuple_combinations() {
        let distance = distance_between_structures(room1, room2);
        if distance > longest_distance_rooms.2 {
            longest_distance_rooms = (room1.id(), room2.id(), distance);
        }
    }
    (longest_distance_rooms.0, longest_distance_rooms.1)
}

/// calculates the shortest path between start and end
/// returns a list of room ids.
fn create_path_between_two_rooms(graph: &MyGraph, start: usize, end: usize) -> Vec<usize> {
    let path_rooms = pathfinding::prelude::dfs(
        start,
        |current| {
            // all edges that contain "current room id" as a node
            let edges = graph.edges(*current).into_iter();

            // create vec of successors: (room_id, distance to current room id)
            let mut successors = Vec::<(usize, OrderedFloat<f32>)>::new();
            for edge in edges {
                successors.push((edge.1, OrderedFloat(edge.2.clone())));
            }
            // sort the successor vec by distance, ascending
            successors.sort_by(|a, b| a.1.cmp(&b.1));
            // convert back to only room_id
            successors.iter().map(|element| element.0).collect_vec()
        },
        |current| *current == end,
    );
    path_rooms.unwrap()
}

/// adds more edges into the graph
/// this usecase: add edges from "all_connections" back into the minimum spanning tree graph
/// ...to create cycles within the graph
fn reassemble_graph(
    mst: &MyGraph,
    all_connections: &MyGraph,
    triangulation: &Triangulation,
    main_path: Option<&Vec<usize>>,
    percentage: f32,
    rng: &mut WorldgenRng,
    map_area: &MapArea
) -> MyGraph {
    // make sure that percentage is between 0 and 1
    let percentage = percentage.min(1.).max(0.);

    let mut output = mst.clone();
    let mut available_connections = all_connections.clone();

    // prepare "available_connections" by removing all MST edges from the graph
    for (a, b, _) in output.all_edges() {
        available_connections.remove_edge(a, b);
    }
    // also remove all hull edges from the graph, not necessary, it just makes the output nicer.
    for (&a, &b) in triangulation.hull.iter().tuple_windows() {
        available_connections.remove_edge(a, b);
    }
    // also do not consider main_path, if it exists
    if let Some(main_path) = main_path {
        for node in main_path.iter() {
            available_connections.remove_node(*node);
        }
    }
    // also do not consider initial connections
    for (a, b) in map_area.initial_connections.iter() {
        available_connections.remove_edge(*a, *b);
    }

    // add available connections back into graph
    for edge in available_connections.0.all_edges() {
        if rng.f32() <= percentage {
            output.add_edge(edge.0, edge.1, edge.2.clone());
        }
    }

    output
}

/// this function remakes the connections between rooms
/// after the rooms have been connected
/// and only connects adjacent rooms via an edge
pub fn remake_graphs(mut map: ResMut<MapResource>) {
    let mut graph = MyGraph::default();

    for connection in map
        .map_area()
        .connections
        .as_ref()
        .clone()
        .expect("no connections for remake_graph were found")
        .iter()
    {
        let room1 = map.map_area().rooms.get(&connection.room1_id).unwrap();
        let room2 = map.map_area().rooms.get(&connection.room2_id).unwrap();
        let distance = distance_between_structures(room1, room2);
        graph.add_edge(room1.id(), room2.id(), distance);
    }

    map.map_area_mut().graph.as_mut().unwrap().reassembled_graph = graph;
}
