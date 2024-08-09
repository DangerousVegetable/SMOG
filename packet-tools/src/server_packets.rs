use serde::{Deserialize, Serialize};

use crate::UnsizedPacket;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerPacket {
    SetMap(String),
    UploadFile { name: String, contents: Vec<u8> },
    SetId(u8),
    StartGame,
}

impl UnsizedPacket for ServerPacket {}
