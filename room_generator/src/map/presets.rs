use std::{
    fs::{self},
    io::{self},
};

use bevy::{prelude::*, utils::HashMap};

use serde::{Deserialize, Serialize};

use crate::map::room::{RoomDetails, RoomDimensions, RoomWithDetailsNoId, StructureCollection};

use super::{aesthetics, generation::WorldgenRng};

#[derive(Deserialize, Debug, Clone, Default)]
pub struct PresetsConfig {
    pub start: Vec<String>,
    pub normal: Vec<String>,
    pub boss: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Preset {
    pub name: String,
    pub rooms: HashMap<String, PresetRoom>,
    pub connections: Vec<PresetRoomConnection>,
    pub modifiers: Vec<PositionalModifier>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PresetRoom {
    pub size: PresetRoomSize,
    pub position: PresetRoomPosition,
    pub aesthetics: Vec<aesthetics::Aesthetics>,
}

#[derive(Resource, Default, Debug)]
pub struct RoomPresetResource {
    config: PresetsConfig,
    presets: Vec<Preset>,
}
impl RoomPresetResource {
    pub fn get_preset_by_name(&self, input: &str) -> Option<Preset> {
        for preset in self.presets.iter() {
            if preset.name == input {
                return Some(preset.clone());
            }
        }
        None
    }

    pub fn get_preset_by_type(&self, input: &str, rng: &mut WorldgenRng) -> Option<Preset> {
        let names = self.get_preset_names_in_type(input);
        match names {
            Some(inner) => {
                // just pick a random one
                let random_choice = inner[rng.usize(0..inner.len())].as_str();
                self.get_preset_by_name(random_choice)
            }
            None => return None,
        }
    }

    fn get_preset_names_in_type(&self, input: &str) -> Option<Vec<String>> {
        let preset_names = match input {
            "start" => self.config.start.clone(),
            "normal" => self.config.normal.clone(),
            "boss" => self.config.boss.clone(),
            _ => return None,
        };

        Some(preset_names)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum PresetRoomSize {
    Fixed(UVec2),
    Range((u32, u32), (u32, u32)),
    Dynamic,
}
trait PresetInfoSource<T> {
    fn get(&self, rng: &mut WorldgenRng) -> Option<T>;
}

impl PresetInfoSource<UVec2> for PresetRoomSize {
    fn get(&self, rng: &mut WorldgenRng) -> Option<UVec2> {
        match self {
            PresetRoomSize::Fixed(xy) => Some(*xy),
            PresetRoomSize::Range(x, y) => {
                let x = rng.u32(x.0..=x.1);
                let y = rng.u32(y.0..=y.1);
                Some(UVec2 { x, y })
            }
            PresetRoomSize::Dynamic => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum PresetRoomPosition {
    Fixed(i32, i32),
    Dynamic,
}

impl PresetInfoSource<IVec2> for PresetRoomPosition {
    fn get(&self, _rng: &mut WorldgenRng) -> Option<IVec2> {
        match self {
            PresetRoomPosition::Fixed(x, y) => Some(IVec2 { x: *x, y: *y }),
            PresetRoomPosition::Dynamic => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PresetRoomConnection {
    pub room1: String,
    pub room2: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PositionalModifier {
    NextTo(String, String),
    SameAxis(String, String),
    DistanceAway(String, String, i32),
}

pub fn init_preset_resource(world: &mut World) {
    let config = read_config().unwrap_or(PresetsConfig::default());
    let presets = read_all_presets().unwrap_or(Vec::new());

    let resource = RoomPresetResource { config, presets };

    world.insert_resource::<RoomPresetResource>(resource);
}

// source: https://www.thorsten-hans.com/weekly-rust-trivia-get-all-files-in-a-directory/ (modified)
fn read_all_presets() -> io::Result<Vec<Preset>> {
    let path = "room_generator/assets/worldgen/presets";
    let entries = fs::read_dir(path)?;

    let file_names: Vec<String> = entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.is_file() {
                path.file_name()?.to_str().map(|s| s.to_owned())
            } else {
                None
            }
        })
        .collect();

    let mut presets = Vec::new();
    for file_name in file_names.iter() {
        let file_path = format!("{}/{}", path, file_name);
        let file_contents = fs::read_to_string(file_path).unwrap();
        let result = ron::de::from_str::<Preset>(&file_contents);
        match result {
            Ok(preset) => presets.push(preset),
            Err(error) => {
                warn!("could not read preset file. log: {:?}", error);
            }
        }
    }
    Ok(presets)
}

fn read_config() -> Result<PresetsConfig, ()> {
    let path = "room_generator/assets/worldgen/config.ron";

    let file_contents = fs::read_to_string(path).unwrap();
    let result = ron::de::from_str::<PresetsConfig>(&file_contents);
    match result {
        Ok(x) => Ok(x),
        Err(_) => Err(()),
    }
}

pub fn generate_rooms_from_preset(
    preset: &Preset,
    rng: &mut WorldgenRng,
) -> (
    StructureCollection<RoomWithDetailsNoId>,
    Vec<(usize, usize)>,
) {
    let mut rooms = vec![];
    let mut connections = vec![];
    let mut name_to_id_map = HashMap::new();


    for (index, (key, preset_room)) in preset.rooms.iter().enumerate() {
        
        name_to_id_map.insert(key, index);
        let mut modifiers = preset.modifiers.clone();
        modifiers.retain_mut(|modifier| {
            match modifier {
                PositionalModifier::NextTo(x, y) => {
                    key.eq(x) || key.eq(y)
                },
                PositionalModifier::SameAxis(x, y) => {
                    key.eq(x) || key.eq(y)
                },
                PositionalModifier::DistanceAway(x,y,_) => {
                    key.eq(x) || key.eq(y)
                },
            }
        });
        let dimensions = calculate_dimensions(preset_room, rng);
        let details = calculate_details(preset_room);
        
        rooms.push(RoomWithDetailsNoId {
            dimensions,
            details,
        });
    }
    for connection in preset.connections.iter() {
        connections.push((
            *name_to_id_map.get(&connection.room1).unwrap(),
            *name_to_id_map.get(&connection.room2).unwrap(),
        ))
    }

    (StructureCollection::new(rooms), connections)
}

fn calculate_dimensions(preset_room: &PresetRoom, rng: &mut WorldgenRng) -> RoomDimensions {
    let anchor = calculate_anchor(preset_room, rng);
    let size = calculate_size(preset_room, rng);
    let dimensions = RoomDimensions {
        anchor,
        length: size.x as usize,
        height: size.y as usize,
    };
    return dimensions;
}

fn calculate_details(preset_room: &PresetRoom) -> RoomDetails {
    RoomDetails {
        is_main: false,
        room_type: super::room::RoomType::Normal,
        aesthetic_modifiers: preset_room.aesthetics.clone(),
    }
}

fn calculate_anchor(preset_room: &PresetRoom, rng: &mut WorldgenRng) -> IVec2 {
    if let Some(anchor) = preset_room.position.get(rng) { return anchor; }
    
    let anchor = IVec2::splat(0);
    return anchor;
}

fn calculate_size(preset_room: &PresetRoom, rng: &mut WorldgenRng) -> UVec2 {
    if let Some(size) = preset_room.size.get(rng) {
        return size;
    } 
    return UVec2::splat(10);
}

