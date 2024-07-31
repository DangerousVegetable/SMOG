use bevy::math::vec2;

use crate::{network::packets::{GamePacket, IndexedGamePacket}, solver::{particle::{Kind, Particle, GROUND}, Solver}};

#[derive(Default, Clone)]
pub struct Player {
    pub id: u8,
    pub motors: Vec<usize>,
    pub power: isize,
}

impl Player {
    pub fn new(id: u8) -> Self {
        Self {
            id, 
            ..Default::default()
        }
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
                    self.solver.particles[ind].enable(Kind::Motor(acc));
                }
            }
            GamePacket::Spawn(pos) => {
                self.solver.add_particle(GROUND.place(pos).velocity(vec2(0., -0.5)));
            }
            GamePacket::Tank(pos) => {
                self.solver.add_tread(pos, 0., 5);
                if packet.id == self.player.id {
                    let last_ind = self.solver.size() - 1;
                    self.player.motors = vec![last_ind-2, last_ind-1, last_ind];
                }   
            }
        }
    }
}