use std::ops::Range;

use bevy::math::{vec4, Vec2};
use solver::{
    chain_model, model,
    particle::{Particle, METAL, MOTOR, SPIKE},
    Connection, Link, Model, Solver,
};

pub const TANK_HP: f32 = 7.;
pub const TANK_ELASTICITY: f32 = 10.;

pub const MUZZLE_ELASTICITY: f32 = 100.;

pub const TREAD_ELASTICITY: f32 = 30.;
pub const TREAD_HP: f32 = 3.;

pub const BASE_HP: f32 = 4.;
pub const BASE_ELASTICITY: f32 = 30.;

pub const PISTOL_HP: f32 = 7.;
pub const PISTOL_ELASTICITY: f32 = 20.;

#[derive(Default, Clone)]
pub struct RawPlayerModel {
    pub particles: Vec<Particle>,
    pub connections: Vec<Connection>,
    pub left_motors: Vec<usize>,  // controlled motors
    pub right_motors: Vec<usize>, // controlled motors
    pub pistols: Vec<usize>,      // controlled connnections
    pub center: usize,            // main particle
    pub muzzle: usize,            // end of the muzzle
    pub center_connection: usize, // hp
}

#[derive(Debug, Default, Clone)]
pub struct PlayerModel {
    pub range: Range<usize>,    // size of the model (in number of particles)
    pub left_motors: Vec<usize>,  // controlled motors
    pub right_motors: Vec<usize>, // controlled motors
    pub pistols: Vec<usize>,      // controlled connnections
    pub center: usize,            // main particle
    pub muzzle: usize,            // end of the muzzle
    pub center_connection: usize, // hp
}

impl PlayerModel {
    pub fn for_each<F: FnMut(usize)>(&self, mut f: F) {
        for i in self.range.clone() {
            f(i);
        }
    }
}

#[allow(unused_mut, unused_assignments)]
impl RawPlayerModel {
    pub fn generate_tank() -> Self {
        // TODO: make it a constant
        let link = Link::Rigid {
            length: 1.,
            durability: BASE_HP,
            elasticity: BASE_ELASTICITY,
        };

        let mut left_base;
        let mut center_base;
        let mut right_base;

        let mut main;
        let mut muzzle_end;

        let mut main_connection = 0;
        let (mut pistol1, mut pistol2) = (0, 0);

        let (mut l0, mut l1, mut l2, mut l3, mut l4, mut l5) = (0, 0, 0, 0, 0, 0); // left motors
        let (mut r0, mut r1, mut r2) = (0, 0, 0); // right motors

        let mut tank = model! {
            METAL.with_color(vec4(0.5, 0.8, 0., 1.)); link => .hex:false [
                @left_base = -4,0; -3,-0.5; -3,0.5; -2,0; -1,-0.5; -1,0.5;
                0,0; @center_base = 0,1;
                1,-0.5; 1,0.5; 2,0; 3,-0.5;3,0.5; @right_base = 4,0
                ] + [0=>1,2; 1,2=>3; 3=>4,5; 4,5=>6,7; 6,7=>8,9; 8,9=>10; 10=>11,12; 11,12=>13; 0=>13]

            METAL.with_color(vec4(0.25, 0.4, 0., 1.)); link.with_elasticity(MUZZLE_ELASTICITY) => .hex:false [
                @main = 0,2; 0,3; 0,4; 0,5; 0,6; 0,7; @muzzle_end = 0,8
            ] + [0=>1; 1=>2; 2=>3; 3=>4; 4=>5; 5=>6]

            none; link.with_durability(PISTOL_HP).with_elasticity(PISTOL_ELASTICITY) => .hex:false [] + [
                .global:true left_base, right_base => .global:true main;
                @pistol1 = .global:true left_base => .global:true muzzle_end;
                @pistol2 = .global:true right_base => .global:true muzzle_end
            ]

            none; link.with_durability(TANK_HP).with_elasticity(TANK_ELASTICITY) => .hex:false [] + [
                @main_connection = .global:true center_base => .global:true main
            ]

            MOTOR.with_color(vec4(0.25, 0.25, 0.25, 1.)); link => .offset:vec2(0.,-3.), .hex:true [
                @l0 = -7.5,2; @l1 = -5.5,0; @l2 = -2,0; @l3 = 2,0; @l4 = 5.5,0; @l5 = 5.5,2;
                @r0 = -5.5,2; @r1 = -1,2; @r2 = 3.5,2
            ] + [
                0 => 1; 1 => 2; 2 => 3; 3 => 4; 4 => 5; 0 => 5; 1 => 4; 0 => 4;
                0,1 => 6; 4,5 => 8; 2,3 => 7;

                .global:true left_base => 0,1; .global:true center_base => 2,3; .global:true right_base => 4,5
            ]
        };

        let tread = chain_model! [
            METAL; link.with_elasticity(TREAD_ELASTICITY).with_durability(TREAD_HP); 2=>SPIKE;link.with_elasticity(100.) => .start:vec2(-6., -3.-SHIFT_Y.y);
            r:12, ur:3, ul:1, l:1, dl:2, l:10, ul:2, l:1, dl:1, dr:3
        ];

        tank = tank + tread;

        Self {
            particles: tank.particles,
            connections: tank.connections,
            center: main,
            muzzle: muzzle_end,
            center_connection: main_connection,
            left_motors: vec![l0, l1, l2, l3, l4, l5],
            right_motors: vec![r0, r1, r2],
            pistols: vec![pistol1, pistol2],
        }
    }

    pub fn model(self) -> Model {
        let center = self.particles[self.center].pos;
        Model {
            particles: self.particles,
            center,
            connections: self.connections,
        }
    }

    pub fn place_in_solver(self, pos: Vec2, solver: &mut Solver) -> PlayerModel {
        let particles = solver.size();
        let connections = solver.connections.len();
        let player_model = PlayerModel {
            range: particles..particles+self.particles.len(),
            left_motors: self.left_motors.iter().map(|m| *m + particles).collect(),
            right_motors: self.right_motors.iter().map(|m| *m + particles).collect(),
            pistols: self.pistols.iter().map(|m| *m + connections).collect(),
            center: self.center + particles,
            muzzle: self.muzzle + particles,
            center_connection: self.center_connection + connections,
        };

        let model = self.model();
        solver.add_model(&model, pos);
        player_model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_tank_test() {
        let tank = RawPlayerModel::generate_tank();
        println!("{}", tank.particles.len());
        println!("{}", tank.connections.len());
        assert_eq!(tank.pistols[0], 29);
        assert_eq!(tank.center_connection, 31);
    }
}
