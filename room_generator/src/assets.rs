use bevy::prelude::*;
use bevy_asset_loader::asset_collection::AssetCollection;


#[derive(AssetCollection, Resource)]
pub struct TextureAtlases {
    #[asset(texture_atlas(tile_size_x = 8., tile_size_y = 8., columns = 1, rows = 1))]
    #[asset(path = "8x8.png")]
    pub mmro_atlas: Handle<TextureAtlas>,
}
