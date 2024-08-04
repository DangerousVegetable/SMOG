use bevy::math::{vec2, Vec2, Vec4, VectorSpace};

use crate::{Constraint, PARTICLE_RADIUS};

pub const GROUND: Particle = Particle {
    mass: 1.,
    texture: 0,
    ..Particle::null()
};

pub const METAL: Particle = Particle {
    mass: 3.,
    texture: 1,
    ..Particle::null()
};

pub const MOTOR: Particle = Particle {
    mass: 3.,
    texture: 2,
    kind: Kind::Motor(0.),
    ..Particle::null()
};

pub const SPIKE: Particle = Particle {
    mass: 0.1,
    texture: 3,
    radius: PARTICLE_RADIUS / 2.,
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
    pub kind: Kind,
    pub color: Vec4,
}

impl Default for Particle {
    fn default() -> Self {
        Particle::null()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    None,
    Motor(f32), // motor with acc
}

impl Kind {
    pub fn none(&self) -> bool {
        *self == Kind::None
    }
}

impl Particle {
    const GRAVITY: Vec2 = vec2(0., -50.);
    const SLOWDOWN: f32 = 80.;

    pub const fn null() -> Self {
        Self {
            radius: crate::PARTICLE_RADIUS,
            mass: 1.,
            texture: 0,
            pos: Vec2::ZERO,
            pos_old: Vec2::ZERO,
            acc: Vec2::ZERO,
            kind: Kind::None,
            color: Vec4::ZERO,
        }
    }

    pub fn position(self, pos: Vec2) -> Self {
        Particle {
            pos,
            pos_old: pos,
            ..self
        }
    }

    pub fn kind(self, kind: Kind) -> Self {
        Particle { kind, ..self }
    }

    pub fn color(self, color: Vec4) -> Self {
        Particle { color, ..self }
    }

    pub fn velocity(self, velocity: Vec2) -> Self {
        Particle {
            pos_old: self.pos - velocity,
            ..self
        }
    }

    pub fn new(radius: f32, mass: f32, pos: Vec2, texture: u32, kind: Kind, color: Vec4) -> Self {
        Self {
            radius,
            mass,
            pos,
            pos_old: pos,
            acc: Vec2::ZERO,
            texture,
            kind,
            color
        }
    }

    pub fn update(&mut self, dt: f32) {
        let vel = self.pos - self.pos_old;
        let new_pos = self.pos + vel + (self.acc - vel * Particle::SLOWDOWN) * dt * dt;
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
        self.acc = if keep_acc { self.acc } else { Vec2::ZERO };
    }

    pub fn set_velocity(&mut self, speed: Vec2) {
        self.pos_old = self.pos - speed;
    }

    pub fn set_kind(&mut self, kind: Kind) {
        self.kind = kind;
    }

    pub fn apply_constraint(&mut self, constraint: Constraint) {
        match constraint {
            Constraint::Box(bl, tr) => {
                let new_x = self.pos.x.max(bl.x + self.radius).min(tr.x - self.radius);
                let new_y = self.pos.y.max(bl.y + self.radius).min(tr.y - self.radius);
                if (new_x, new_y) != (self.pos.x, self.pos.y) {
                    self.set_position(vec2(new_x, new_y), false);
                }
            }
        }
    }

    pub fn is_motor(&self) -> bool {
        if let Kind::Motor(_) = self.kind {
            true
        } else {
            false
        }
    }
}
