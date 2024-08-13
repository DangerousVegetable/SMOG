use std::{borrow::{Borrow, BorrowMut}, ops::Range};

use bevy::math::Vec2;
use rand::Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

mod multithreaded;
pub mod particle;
pub mod model;
pub use model::Model;
mod utils;
use self::{
    multithreaded::UnsafeMultithreadedArray,
    utils::Grid,
};

use self::particle::{Kind, Particle};
pub const MAX: u32 = 200000;
pub const PARTICLE_RADIUS: f32 = 0.5;

pub type Connection = (usize, usize, Link);
#[derive(Clone)]
pub struct Solver {
    pub constraint: Constraint,
    pub particles: Vec<Particle>,
    pub connections: Vec<Connection>,
    pub cell_size: f32,
    grid: Grid<usize>,
}

impl Solver {
    pub fn new(constraint: Constraint, particles: &[Particle], connections: &[Connection]) -> Self {
        let cell_size = 2. * PARTICLE_RADIUS;
        let bounds = constraint.bounds();
        let width: usize = ((bounds.1.x - bounds.0.x) / cell_size) as usize + 3;
        let height: usize = ((bounds.1.y - bounds.0.y) / cell_size) as usize + 3;

        Self {
            constraint,
            particles: Vec::from(particles),
            connections: Vec::from(connections),
            cell_size,
            grid: Grid::new(width, height),
        }
    }

    fn populate_grid(&mut self) {
        self.grid.clear();
        for (i, particle) in self.particles.iter().enumerate() {
            let p = self.get_cell(particle.pos);
            self.grid.push(p, i);
        }
    }

    fn get_cell(&self, pos: Vec2) -> (usize, usize) {
        let bounds = self.constraint.bounds().0;
        (
            (((pos.x - bounds.x) / self.cell_size).max(0.) as usize + 1).min(self.grid.width - 1),
            (((pos.y - bounds.y) / self.cell_size).max(0.) as usize + 1).min(self.grid.height - 1),
        )
    }

    pub fn solve(&mut self, dt: f32) {
        // populate the grid with indexes of particles
        self.populate_grid(); // TODO: for some reason it's slow in debug mode

        self.resolve_collisions();
        self.resolve_connections();

        self.particles.par_iter_mut().for_each(|p| {
            p.apply_gravity();
            p.update(dt);
            p.apply_constraint(self.constraint);
        });
    }

    fn resolve_collisions(&mut self) {
        let even: Vec<Range<usize>> = (1..self.grid.width - 1)
            .filter(|i| i % 4 == 1)
            .map(|i| i..std::cmp::min(i + 2, self.grid.width - 1))
            .collect();
        let odd: Vec<Range<usize>> = (1..self.grid.width - 1)
            .filter(|i| i % 4 == 3)
            .map(|i| i..std::cmp::min(i + 2, self.grid.width - 1))
            .collect();

        let groups = &[even, odd];

        let particles = UnsafeMultithreadedArray::new(&mut self.particles); // create unsafe array that can be manipulated in threads
        let grid: &Grid<usize> = self.grid.borrow();

        for group in groups {
            group.par_iter().for_each(|range| {
                for col in range.clone() {
                    for row in 1..grid.height - 1 {
                        let c = (col, row);
                        for &i in grid[c].iter() {
                            for dc in -1..=1 {
                                for dr in -1..=1 {
                                    let adj = (
                                        (col as isize + dc) as usize,
                                        (row as isize + dr) as usize,
                                    );
                                    for &j in grid[adj].iter() {
                                        if i == j {
                                            continue;
                                        }
                                        Solver::resolve_collision(
                                            &mut particles.clone()[i],
                                            &mut particles.clone()[j],
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            })
        }
    }

    fn resolve_connections(&mut self) {
        for (i, j, link) in self.connections.iter_mut() {
            let (i, j) = (usize::min(*i, *j), usize::max(*i, *j));
            let (head, tail) = self.particles.split_at_mut(i + 1);
            Solver::resolve_connection(&mut head[i], &mut tail[j - i - 1], link);
        }
    }

    pub fn resolve_collision(p1: &mut Particle, p2: &mut Particle) {
        if !p1.kind.can_collide_with(&p2.kind) { return };

        let mut v = p1.pos - p2.pos;
        if v.length() < p1.radius + p2.radius && v.length() > 0.1 {
            let overlap = p1.radius + p2.radius - v.length();
            let c1 = p2.mass / (p1.mass + p2.mass);
            let c2 = 1.-c1;
            v = v.normalize_or_zero() * overlap;
            p1.set_position(p1.pos + v * c1, true);
            p2.set_position(p2.pos - v * c2, true);

            if !p1.kind.none() {
                Solver::resolve_interaction(p1, p2);
            }
            if !p2.kind.none() {
                Solver::resolve_interaction(p2, p1);
            }
        }
    }

    pub fn resolve_interaction(p1: &mut Particle, p2: &mut Particle) {
        match p1.kind.borrow_mut() {
            Kind::Motor(acc) => {
                let v = (p2.pos - p1.pos).normalize_or_zero();
                let acceleration = v.perp() * *acc;
                p2.accelerate(acceleration);
                p1.accelerate(-acceleration/2.);
            }
            Kind::Impulse(imp) => {
                if *imp < 0. { return }
                let v = (p2.pos - p1.pos).normalize_or_zero() * 0.66;
                p2.set_velocity(v);
                *imp -= 1.;
                p1.color *= 0.95;
            }
            _ => (),
        }
    }

    pub fn resolve_connection(p1: &mut Particle, p2: &mut Particle, link: &mut Link) {
        match link {
            Link::Force(force) => {
                let v = (p2.pos - p1.pos).normalize_or_zero();
                p1.accelerate(v * *force);
                p2.accelerate(-v * *force);
            }
            Link::Rigid {
                length,
                durability,
                elasticity,
            } => {
                if *durability < 0. { return };
                let mut v = p1.pos - p2.pos;
                let overlap = (*length - v.length()) / 2.;
                v = overlap * v.normalize_or_zero();
                p1.set_position(p1.pos + v, true);
                p2.set_position(p2.pos - v, true);

                let max_length = *elasticity * (*length) / 100.;
                if 2. * overlap.abs() > max_length {
                    *durability -= 2. * overlap.abs() / max_length - 1.; // substract the amount of percent max_length was exceeded
                }
            }
            _ => (),
        }
    }

    pub fn size(&self) -> usize {
        self.particles.len()
    }

    pub fn add_particle(&mut self, particle: Particle) {
        self.particles.push(particle);
    }

    pub fn add_rib(&mut self, i: usize, j: usize, length: f32) {
        self.connections.push((
            i,
            j,
            Link::Rigid {
                length,
                durability: 1.,
                elasticity: 10.,
            },
        ))
    }

    pub fn add_spring(&mut self, i: usize, j: usize, force: f32) {
        self.connections.push((i, j, Link::Force(force)))
    }

    pub fn add_model(&mut self, model: &Model, pos: Vec2) {
        let offset = pos - model.center;
        let particles_num = self.particles.len();
        self.particles.extend(
            model
                .particles
                .iter()
                .map(|p| p.with_position(p.pos + offset)),
        );
        self.connections.extend(model.connections.iter().map(|(i, j, link)| {
            (*i + particles_num, *j + particles_num, *link)
        }));
    }

    fn rnd_origin(&self) -> Vec2 {
        let bounds = self.constraint.bounds();
        rnd_in_bounds(bounds, 2. * PARTICLE_RADIUS)
    }
}

pub fn rnd_in_bounds(bounds: (Vec2, Vec2), margin: f32) -> Vec2 {
    Vec2::new(
        rand::thread_rng().gen_range(bounds.0.x + margin..bounds.1.x - margin),
        rand::thread_rng().gen_range(bounds.0.y + margin..bounds.1.y - margin),
    )
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Link {
    Force(f32), // force
    Rigid {
        length: f32,
        durability: f32,
        elasticity: f32,
    },
}

impl Link {
    pub fn with_length(&self, length: f32) -> Self {
        match self {
            Self::Force(_) => *self,
            Self::Rigid {
                length: _,
                durability,
                elasticity,
            } => Self::Rigid {
                length,
                durability: *durability,
                elasticity: *elasticity,
            },
        }
    }

    pub fn with_durabliity(&self, durability: f32) -> Self {
        match self {
            Self::Force(_) => *self,
            Self::Rigid {
                length,
                durability: _,
                elasticity,
            } => Self::Rigid {
                length: *length,
                durability,
                elasticity: *elasticity,
            },
        }
    }

    pub fn with_elasticity(&self, elasticity: f32) -> Self {
        match self {
            Self::Force(_) => *self,
            Self::Rigid {
                length,
                durability,
                elasticity: _,
            } => Self::Rigid {
                length: *length,
                durability: *durability,
                elasticity,
            },
        }
    }

    pub fn durability(&self) -> f32 {
        match self {
            Self::Rigid {
                length: _,
                durability,
                elasticity: _,
            } => *durability,
            _ => 1.,
        }
    }

    pub fn elasticity(&self) -> f32 {
        match self {
            Self::Rigid {
                length: _,
                durability: _,
                elasticity,
            } => *elasticity,
            _ => 100.,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Constraint {
    Box(Vec2, Vec2), // Rectangle, bottom-left and top-right corners
}

impl Constraint {
    pub const fn bounds(&self) -> (Vec2, Vec2) {
        match self {
            &Constraint::Box(bl, tr) => (bl, tr),
        }
    }
}
