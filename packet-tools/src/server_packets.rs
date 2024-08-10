use serde::{Deserialize, Serialize};

use crate::UnsizedPacket;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerPacket {
    SetMap(String),
    CreateFile { name: String, contents: Vec<u8> },
    SetPlayers(Vec<(u8, String)>),
    SetId(u8),
    StartGame,
}

impl UnsizedPacket for ServerPacket {}
