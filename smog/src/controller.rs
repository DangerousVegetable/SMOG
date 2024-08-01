use bevy::math::{vec2, Vec2};

use crate::{network::packets::{GamePacket, IndexedGamePacket}, solver::{particle::{Kind, Particle, GROUND}, Solver}};

#[derive(Default, Clone)]
pub struct Player {
    pub id: u8,
    pub motors: Vec<usize>,
    pub gear: usize,
}

impl Player {
    const BASE_POWER: f32 = 16.;
    const GEAR_POWER: f32 = 2.;
    const MAX_GEAR: usize = 5;

    pub fn new(id: u8) -> Self {
        Self {
            id, 
            ..Default::default()
        }
    }

    pub fn get_power(&self) -> f32 {
        Self::BASE_POWER * f32::powf(Self::GEAR_POWER, self.gear as f32)
    }

    pub fn gear_up(&mut self) {
        self.gear = usize::min(self.gear + 1, Self::MAX_GEAR);
    }

    pub fn gear_down(&mut self) {
        self.gear = usize::max(self.gear, 1) - 1;
    }
}

#[derive(Clone)]
pub struct Controller {
    pub solver: Solver,
    pub player: Player,
}

impl Controller {
    pub fn new(id: u8, solver: Solver) -> Self {
        Self {
            solver,
            player: Player::new(id)
        }
    }

    pub fn handle_packets(&mut self, packets: &Vec<IndexedGamePacket>) {
        for packet in packets {
            self.handle_packet(packet);
        }
    }

    pub fn handle_packet(&mut self, packet: &IndexedGamePacket) {
        match packet.contents {
            GamePacket::Motor(ind, acc) => {
                let ind = ind as usize;
                if self.solver.particles.get(ind).map_or(false, |p| p.is_motor()) {
                    self.solver.particles[ind].set_kind(Kind::Motor(acc));
                }
            }
            GamePacket::Spawn(pos) => {
                self.solver.add_particle(GROUND.position(pos).velocity(vec2(0., -0.5)));
            }
            GamePacket::Tank(pos) => {
                self.solver.add_tread(pos, 0., 7);
                if packet.id == self.player.id {
                    let last_ind = self.solver.size() - 1;
                    self.player.motors = vec![last_ind-2, last_ind-1, last_ind];
                }   
            }
            _ => ()
        }
    }

    pub fn add_particle(&self, pos: Vec2) -> GamePacket {
        GamePacket::Spawn(pos)
    }

    pub fn move_player(&self, coeff: f32) -> Vec<GamePacket> {
        self.player.motors.iter()
            .map(|ind| {
                GamePacket::Motor(*ind as u32, coeff*self.player.get_power())
            })
            .collect()
    }
}