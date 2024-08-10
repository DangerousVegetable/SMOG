pub const ASSETS_PATH : &str = "assets";
pub const RELATIVE_MAPS_PATH : &str = "assets/maps";
pub const ASSETS_MAPS_PATH: &str = "maps/";
pub const MAP_FILE: &str = "map.smog";
pub const BACKGROUND_FILE: &str = "background.png";

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
