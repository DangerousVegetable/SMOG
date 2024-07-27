use bevy::math::{vec2, Vec2};

use crate::solver::{self, Constraint};

pub const SAND : Particle = Particle {
    radius: solver::PARTICLE_SIZE,
    mass: 1.,
    texture: 0,
    ..Particle::null()
};

pub const METAL : Particle = Particle {
    radius: solver::PARTICLE_SIZE,
    mass: 10.,
    texture: 1,
    ..Particle::null()
};

#[derive(Debug, Clone, Copy)]
pub struct Particle {
    pub radius: f32,
    pub mass: f32,
    pub pos: Vec2,
    pub pos_old: Vec2,
    pub acc: Vec2,
    pub texture: u32,
}

impl Default for Particle {
    fn default() -> Self {
        Particle::null()
    }
}

impl Particle {
    const GRAVITY : Vec2 = vec2(0., -50.);
    const SLOWDOWN : f32 = 40.;

    pub const fn null() -> Self {
        Self {
            radius: solver::PARTICLE_SIZE,
            mass: 1.,
            texture: 0,
            pos: Vec2::ZERO,
            pos_old: Vec2::ZERO,
            acc: Vec2::ZERO,
        }
    }

    pub fn place(&self, pos: Vec2) -> Self {
        Particle { 
            pos, 
            pos_old: pos, 
            ..*self}
    }

    pub fn new(radius: f32, mass: f32, pos: Vec2, texture: u32) -> Self {
        Self {
            radius,
            mass,
            pos,
            pos_old: pos,
            acc: Vec2::ZERO,
            texture,
        }
    }

    pub fn update(&mut self, dt: f32) {
        let vel = self.pos - self.pos_old;
        let new_pos = self.pos + vel + (self.acc - vel*Particle::SLOWDOWN)*dt*dt;
        self.pos_old = self.pos;
        self.pos = new_pos;
        self.acc = Vec2::ZERO;
    }

    pub fn apply_gravity(&mut self) {
        self.accelerate(Particle::GRAVITY);
    }

    pub fn accelerate(&mut self, acceleration: Vec2) {
        self.acc += acceleration;
    }

    pub fn set_position(&mut self, pos: Vec2, keep_acc: bool) {
        self.pos = pos;
        self.acc = if keep_acc {self.acc} else {Vec2::ZERO};
    }

    pub fn apply_constraint(&mut self, constraint: Constraint) {
        match constraint {
            Constraint::Cup(bl, tr) => {
                let new_x = self.pos.x.max(bl.x + self.radius).min(tr.x - self.radius);
                let new_y = self.pos.y.max(bl.y + self.radius);
                if (new_x, new_y) != (self.pos.x, self.pos.y) {
                    self.set_position(vec2(new_x, new_y), false);
                }
            },
            Constraint::Box(bl, tr) => {
                let new_x = self.pos.x.max(bl.x + self.radius).min(tr.x - self.radius);
                let new_y = self.pos.y.max(bl.y + self.radius).min(tr.y - self.radius);
                if (new_x, new_y) != (self.pos.x, self.pos.y) {
                    self.set_position(vec2(new_x, new_y), false);
                }
            },
        }
    }
}

