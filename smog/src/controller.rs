use bevy::{color::Color, math::{vec2, vec4, Vec2, Vec4}, utils::HashMap};

use map_editor::map::Spawn;
use model::{PlayerModel, PISTOL_HP, TANK_HP};
use packet_tools::game_packets::{GamePacket, IndexedGamePacket};

use solver::{
    particle::{Kind, GROUND, PROJECTILE_HEAVY, PROJECTILE_IMPULSE, PROJECTILE_STICKY},
    Solver,
};

pub mod model;

#[derive(Clone, Default)]
pub struct Player {
    pub id: u8,
    pub team: usize,
    pub _name: String,
    pub model: PlayerModel,
    pub gear: usize,
    pub projectile: u8,

    // timers
    pub reload_timer: TickTimer,
    pub dash_timer: TickTimer,

    // utils
    pub thrust: (f32, f32),
    pub aim: Option<Vec2>,
}

impl Player {
    const BASE_POWER: f32 = 16.;
    const GEAR_POWER: f32 = 2.;
    const MAX_GEAR: usize = 5;

    pub fn new(id: u8, team: usize, name: String, model: PlayerModel) -> Self {
        Self {
            id,
            team,
            _name: name,
            model,
            gear: 0,
            projectile: 0,
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
    pub tick: u128,
    pub player: Player,
    pub players: Vec<Player>,
}

impl Controller {
    pub fn new(id: u8, name: String, model: PlayerModel, players: Vec<(u8, String, PlayerModel)>, spawns: &Vec<Spawn>) -> Self {
        Self {
            tick: 0,
            player: Player::new(id, spawns[id as usize].team, name, model),
            players: players.into_iter().map(|p| Player::new(p.0, spawns[p.0 as usize].team, p.1, p.2)).collect(),
        }
    }

    pub fn get_player(&self, id: u8) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }

    pub fn get_player_mut(&mut self, id: u8) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    pub fn get_player_pos(&self, player: &Player, solver: &Solver) -> Vec2 {
        solver.particles[player.model.center].pos
    }

    pub fn get_player_hp(&self, player: &Player, solver: &Solver) -> f32 {
        solver.connections[player.model.center_connection].2.durability() / TANK_HP
    }

    pub fn get_winners(&self, solver: &Solver) -> Option<(usize, Vec<&Player>)> {
        let mut team_num = HashMap::<usize, Vec<&Player>>::new();
        for p in self.players.iter() {
            if Self::player_alive(p, solver) {
                let v = team_num.entry(p.team).or_insert(vec![]);
                v.push(p);
            }
        }

        let team = if team_num.keys().len() == 1 {
            Some(*team_num.keys().next().unwrap())
        } else { None };

        team.map(|team| {
            let players = team_num.remove(&team).unwrap();
            (team, players)
        })
    }

    pub fn player_alive(player: &Player, solver: &Solver) -> bool {
        solver.connections[player.model.center_connection].2.durability() > 0.
    }

    fn update_timers(&mut self) {
        self.tick += 1;
        self.player.reload_timer.update();
        self.player.dash_timer.update();
    }

    fn update_player_colors(&self, solver: &mut Solver) {
        for player in self.players.iter() {
            let hp = self.get_player_hp(player, solver);
            let center = &mut solver.particles[player.model.center];
            center.color = get_color(hp);

            for pistol in &player.model.pistols {
                let (pistol_base, _, link) = solver.connections[*pistol];
                let pistol_base = &mut solver.particles[pistol_base];
                let hp = link.durability() / PISTOL_HP;
                pistol_base.color = get_color(hp);
            }
        }
    }

    fn update_players(&self, solver: &mut Solver) {
        for player in &self.players {
            if !Self::player_alive(player, solver) { continue; }

            if self.tick % 8 == 0 {
            let left_motor = player.model.right_motors.first().unwrap();
            let right_motor = player.model.right_motors.last().unwrap();

            let center = solver.particles[player.model.center];
            let (center_base, _, _) = solver.connections[player.model.center_connection];
            let center_base = solver.particles[center_base];
            let direction_up = center.pos - center_base.pos;

            // thrust
            if player.thrust.0 != 0. || player.thrust.1 != 0. {
                solver.particles[*left_motor].set_velocity(player.thrust.0*direction_up);
                solver.particles[*right_motor].set_velocity(player.thrust.1*direction_up);
            }

            // aim
                let Some(desired_pos) = player.aim else { continue };
                let mut desired_pos = (desired_pos - center.pos).normalize() * 6. + center.pos;
                if (desired_pos - center.pos).dot(direction_up) < -0.1 {
                    desired_pos = f32::signum(direction_up.perp_dot(desired_pos - center.pos))
                        * direction_up.perp()
                        * 6.
                        + center.pos;
                }
    
                let muzzle = solver.particles[player.model.muzzle];
                let mut angle = (muzzle.pos - center.pos).angle_between(desired_pos - center.pos);
                angle = angle.min(0.04).max(-0.04);
                desired_pos = (muzzle.pos - center.pos)
                    .rotate(Vec2::from_angle(angle))
                    .normalize()
                    * 6.
                    + center.pos;
    
                player.model.pistols.iter().for_each(|pistol| {
                    let (i, _, link) = &mut solver.connections[*pistol];
                    let base = solver.particles[*i];
                    *link = link.with_length(desired_pos.distance(base.pos));
                });
            }
        }
    }

    pub fn handle_packets(&mut self, solver: &mut Solver, packets: &Vec<IndexedGamePacket>) {
        self.update_timers();
        self.update_player_colors(solver);
        self.update_players(solver);

        for packet in packets {
            self.handle_packet(solver, packet);
        }
    }

    pub fn handle_packet(&mut self, solver: &mut Solver, packet: &IndexedGamePacket) {
        let Some(player) = self.get_player_mut(packet.id) else {
            return;
        };
        // check if player's tank is active
        if !Self::player_alive(player, solver) { return; }

        let center = solver.particles[player.model.center];

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
            GamePacket::Dash(coeff) => {
                let vel = (center.velocity() * coeff).clamp_length(0.05, 0.1);
                for p in &mut solver.particles[player.model.range.clone()] {
                    p.set_velocity(coeff*vel);
                }
            }
            GamePacket::Thrust(left, right) => {
                player.thrust = (left, right);
            }
            GamePacket::Muzzle(desired_pos) => {
                player.aim = Some(desired_pos);
            }
            GamePacket::ResetMuzzle => {
                player.aim = None;
            }
            GamePacket::Fire(bullet) => {
                let center = &solver.particles[player.model.center];
                let muzzle_end = &solver.particles[player.model.muzzle];
                let muzzle_dir = (muzzle_end.pos - center.pos).normalize();
                let bullet_pos = center.pos + muzzle_dir * 10.;

                let Some((projectile, force)) = (match bullet {
                    0 => Some((PROJECTILE_HEAVY, 0.6)),
                    1 => Some((PROJECTILE_IMPULSE, 0.25)),
                    2 => Some((PROJECTILE_STICKY, 0.1)),
                    _ => None,
                }) else { return };

                solver.add_particle(
                    projectile
                    .with_position(bullet_pos)
                    .with_velocity(muzzle_dir * force));

                let imp = force * muzzle_dir.length() * projectile.mass;
                let muzzle_end = &mut solver.particles[player.model.muzzle];
                let recoil = imp / muzzle_end.mass / 100.;
                player.model.for_each(|i| {
                    solver.particles[i].add_velocity(-recoil * muzzle_dir);
                });
            }
            GamePacket::None => ()
        }
    }

    pub fn add_particle(&self, pos: Vec2) -> Vec<GamePacket> {
        vec![GamePacket::Spawn(pos)]
    }

    pub fn move_tank(&self, coeff: f32) -> Vec<GamePacket> {
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

    pub fn reset_muzzle(&self) -> Vec<GamePacket> {
        vec![GamePacket::ResetMuzzle]
    }

    pub fn fire(&mut self) -> Vec<GamePacket> {
        if self.player.reload_timer.not_ready() { return vec![] };

        let reload_ticks = match self.player.projectile {
            0 => 400,
            1 => 1500,
            2 => 16,
            _ => 0,
        };

        self.player.reload_timer.set(reload_ticks);
        vec![GamePacket::Fire(self.player.projectile)]
    }

    pub fn rotate_tank(&self, force: f32) -> Vec<GamePacket> {
        let (left, right) = (force, -force);

        vec![GamePacket::Thrust(left, right)]
    }

    pub fn dash(&mut self) -> Vec<GamePacket> {
        self.player.dash_timer.map_or(vec![], 4800, || {
            vec![GamePacket::Dash(2.)]
        })
    }
}

fn get_color(a: f32) -> Vec4 {
    let a = a.max(0.);
    let color = Color::hsl(a*120., 1., if a == 0. {0.} else {0.7}).to_linear();
    vec4(color.red, color.green, color.blue, 1.)
}

#[derive(Clone, Default)]
pub struct TickTimer {
    pub tick: isize,
    last: isize,
}

impl TickTimer {
    pub fn new() -> Self {
        Self {
            tick: 0,
            last: 0,
        }
    }

    pub fn set(&mut self, ticks: isize) {
        self.tick = ticks;
        self.last = ticks;
    }

    pub fn update(&mut self) {
        self.tick -= 1;
    }

    pub fn ready(&self) -> bool {
        self.tick <= 0
    }

    pub fn not_ready(&self) -> bool {
        self.tick > 0
    }

    pub fn map_or<U, F: FnOnce() -> U>(&mut self, default: U, ticks: isize, f: F) -> U {
        if self.ready() {
            self.set(ticks);
            f()
        } else {
            default
        }
    }

    pub fn progress(&self) -> f32 {
        if self.last <= 0 {
            return 1.
        }
        let elapsed = self.last - self.tick;
        (elapsed as f32 / self.last as f32).clamp(0., 1.)
    }
}
