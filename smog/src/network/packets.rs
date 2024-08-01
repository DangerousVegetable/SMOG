use bevy::{log::error, math::{vec2, Vec2}};
use packet_tools::{IndexedPacket, Packet};
pub use server::PACKET_SIZE;

pub type IndexedGamePacket = IndexedPacket<GamePacket, PACKET_SIZE>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GamePacket {
    None,
    Spawn(Vec2),
    Motor(u32, f32),
    Tank(Vec2),
}

impl Packet<PACKET_SIZE> for GamePacket {
    fn to_bytes(&self) -> [u8; PACKET_SIZE] {
        let mut bytes = vec![];
        match self {
            Self::Spawn(pos) => {
                bytes.push(0);
                bytes.extend(&f32::to_be_bytes(pos.x));
                bytes.extend(&f32::to_be_bytes(pos.y));
            } 
            Self::Motor(ind, acc) => {
                bytes.push(1);
                bytes.extend(&u32::to_be_bytes(*ind));
                bytes.extend(&f32::to_be_bytes(*acc));
            }
            Self::Tank(pos) => {
                bytes.push(2);
                bytes.extend(&f32::to_be_bytes(pos.x));
                bytes.extend(&f32::to_be_bytes(pos.y));
            }
            _ => bytes = vec![0u8; 9]
        }

        bytes.try_into().unwrap()
    }

    fn from_bytes(value: &[u8; PACKET_SIZE]) -> Self {
        let kind = value[0];
        match kind {
            0 => {
                let x = f32::from_be_bytes(value[1..5].try_into().unwrap());
                let y = f32::from_be_bytes(value[5..9].try_into().unwrap());
                Self::Spawn(vec2(x, y))
            },
            1 => {
                let ind = u32::from_be_bytes(value[1..5].try_into().unwrap());
                let acc = f32::from_be_bytes(value[5..9].try_into().unwrap());
                Self::Motor(ind, acc)
            },
            2 => {
                let x = f32::from_be_bytes(value[1..5].try_into().unwrap());
                let y = f32::from_be_bytes(value[5..9].try_into().unwrap());
                Self::Tank(vec2(x, y))
            },
            _ => {
                error!("receive damaged packet from server");
                Self::None
            }
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn test_conversion() {
        let v = vec![
            GamePacket::Spawn(vec2(10.1, 32.2)), 
            GamePacket::Motor(69000, 53.2),
            GamePacket::Tank(vec2(10.9, 32.)), 
        ];
        for p in v {
            assert_eq!(p, GamePacket::from_bytes(&p.to_bytes()));
        }
    }
}