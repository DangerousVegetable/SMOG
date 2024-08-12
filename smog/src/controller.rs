use bevy::math::{vec2, Vec2};

use model::PlayerModel;
use packet_tools::game_packets::{GamePacket, IndexedGamePacket};

use solver::{
    particle::{Kind, Particle, GROUND, PROJECTILE_HEAVY},
    Solver,
};

use crate::network::client::LobbyInfo;

pub mod model;

#[derive(Clone)]
pub struct Player {
    pub id: u8,
    pub model: PlayerModel,
    pub gear: usize,
}

impl Player {
    const BASE_POWER: f32 = 16.;
    const GEAR_POWER: f32 = 2.;
    const MAX_GEAR: usize = 5;

    pub fn new(id: u8, model: PlayerModel) -> Self {
        Self { id, model, gear: 0 }
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
    pub player: Player,
    pub players: Vec<Player>,
}

impl Controller {
    pub fn new(id: u8, model: PlayerModel, players: Vec<(u8, PlayerModel)>) -> Self {
        Self {
            player: Player::new(id, model),
            players: players.into_iter().map(|p| Player::new(p.0, p.1)).collect(),
        }
    }

    pub fn get_player(&self, id: u8) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }

    pub fn handle_packets(&mut self, solver: &mut Solver, packets: &Vec<IndexedGamePacket>) {
        for packet in packets {
            self.handle_packet(solver, packet);
        }
    }

    pub fn handle_packet(&mut self, solver: &mut Solver, packet: &IndexedGamePacket) {
        let Some(player) = self.get_player(packet.id) else {
            return;
        };
        // check if player's tank is active
        if solver.connections[player.model.center_connection]
            .2
            .durability()
            < 0.
        {
            return;
        }

        match packet.contents {
            GamePacket::Motor(ind, acc) => {
                let ind = ind as usize;
                if solver.particles.get(ind).map_or(false, |p| p.is_motor()) {
                    solver.particles[ind].set_kind(Kind::Motor(acc));
                }
            }
            GamePacket::Spawn(pos) => {
                solver.add_particle(GROUND.with_position(pos).with_velocity(vec2(0., -0.5)));
            }
            GamePacket::Muzzle(desired_pos) => {
                let center = solver.particles[player.model.center];
                let (center_base, _, _) = solver.connections[player.model.center_connection];
                let center_base = solver.particles[center_base];
                let direction_up = center.pos - center_base.pos;

                let mut desired_pos = (desired_pos - center.pos).normalize() * 6. + center.pos;
                if (desired_pos - center.pos).dot(direction_up) < 0. {
                    desired_pos = f32::signum(direction_up.perp_dot(desired_pos - center.pos))
                        * direction_up.perp()
                        * 6.
                        + center.pos;
                }

                let muzzle = solver.particles[player.model.muzzle];
                let mut angle = (muzzle.pos - center.pos).angle_between(desired_pos - center.pos);
                angle = angle.min(0.01).max(-0.01);
                desired_pos = (muzzle.pos - center.pos)
                    .rotate(Vec2::from_angle(angle))
                    .normalize()
                    * 6.
                    + center.pos;

                player.model.pistols.iter().for_each(|pistol| {
                    let (i, _, link) = solver.connections.get_mut(*pistol).unwrap();
                    let base = solver.particles[*i];
                    *link = link.with_length(desired_pos.distance(base.pos));
                });
            }
            GamePacket::Fire(bullet) => {
                let center = &solver.particles[player.model.center];
                let muzzle_end = &solver.particles[player.model.center];
                let muzzle_dir = (muzzle_end.pos - center.pos).normalize();
                let bullet_pos = center.pos + muzzle_dir * 10.;

                match bullet {
                    0 => {
                        solver.add_particle(
                            PROJECTILE_HEAVY
                                .with_position(bullet_pos)
                                .with_velocity(muzzle_dir * 5.),
                        );
                    }
                    _ => (),
                };
            }
            _ => (),
        }
    }

    pub fn add_particle(&self, pos: Vec2) -> GamePacket {
        GamePacket::Spawn(pos)
    }

    pub fn move_player(&self, coeff: f32) -> Vec<GamePacket> {
        self.player
            .model
            .left_motors
            .iter()
            .map(|ind| GamePacket::Motor(*ind as u32, coeff * self.player.get_power()))
            .chain(
                self.player
                    .model
                    .right_motors
                    .iter()
                    .map(|ind| GamePacket::Motor(*ind as u32, -coeff * self.player.get_power())),
            )
            .collect()
    }

    pub fn move_muzzle(&self, desired_pos: Vec2) -> Vec<GamePacket> {
        vec![GamePacket::Muzzle(desired_pos)]
    }
}
