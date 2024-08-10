use serde::{Deserialize, Serialize};

use crate::UnsizedPacket;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientPacket {
    SetName(String),
    RequestMap,
    Ok,
}

impl UnsizedPacket for ClientPacket {}

