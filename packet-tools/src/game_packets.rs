use bevy::{log::error, math::{vec2, Vec2}};

use crate::{IndexedPacket, Packet};

pub const PACKET_SIZE: usize = 9;
pub type IndexedGamePacket = IndexedPacket<GamePacket, {PACKET_SIZE}>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GamePacket {
    None,
    Spawn(Vec2),
    Motor(u32, f32),
    Muzzle(Vec2),
    Fire(u8),
}

impl Packet<{PACKET_SIZE}> for GamePacket {
    fn to_bytes(&self) -> [u8; PACKET_SIZE] {
        let mut bytes = vec![];
        match self {
            Self::Spawn(pos) => {
                bytes.push(1);
                bytes.extend(&f32::to_be_bytes(pos.x));
                bytes.extend(&f32::to_be_bytes(pos.y));
            } 
            Self::Motor(ind, acc) => {
                bytes.push(2);
                bytes.extend(&u32::to_be_bytes(*ind));
                bytes.extend(&f32::to_be_bytes(*acc));
            }
            Self::Muzzle(pos) => {
                bytes.push(3);
                bytes.extend(&f32::to_be_bytes(pos.x));
                bytes.extend(&f32::to_be_bytes(pos.y));
            }
            Self::Fire(bullet) => {
                bytes.push(4);
                bytes.push(*bullet);
                bytes.extend(&[0;7]);
            }
            Self::None => bytes = vec![0u8; 9]
        }

        bytes.try_into().unwrap()
    }

    fn from_bytes(value: &[u8; PACKET_SIZE]) -> Self {
        let kind = value[0];
        match kind {
            1 => {
                let x = f32::from_be_bytes(value[1..5].try_into().unwrap());
                let y = f32::from_be_bytes(value[5..9].try_into().unwrap());
                Self::Spawn(vec2(x, y))
            },
            2 => {
                let ind = u32::from_be_bytes(value[1..5].try_into().unwrap());
                let acc = f32::from_be_bytes(value[5..9].try_into().unwrap());
                Self::Motor(ind, acc)
            },
            3 => {
                let x = f32::from_be_bytes(value[1..5].try_into().unwrap());
                let y: f32 = f32::from_be_bytes(value[5..9].try_into().unwrap());
                Self::Muzzle(vec2(x, y))
            },
            4 => {
                let bullet = value[1];
                Self::Fire(bullet)
            }
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
            GamePacket::Muzzle(vec2(10.9, 32.)), 
            GamePacket::Fire(10),
        ];
        for p in v {
            assert_eq!(p, GamePacket::from_bytes(&p.to_bytes()));
        }
    }
}