use std::{
    borrow::Borrow,
    f32::consts::PI,
    ops::{Index, IndexMut, Range},
    time::Instant,
};

use bevy::math::{vec2, Vec2};
use rand::Rng;
use rayon::prelude::*;

pub mod particle;
mod multithreaded;
use self::{
    multithreaded::{UnsafeMultithreadedArray},
    particle::{MOTOR, SPIKE},
};

use self::particle::{Kind, Particle, METAL, GROUND};
pub const MAX: u32 = 200000;
pub const PARTICLE_SIZE: f32 = 0.5;

pub type Connection = (usize, usize, Link);
#[derive(Clone)]
pub struct Solver {
    pub constraint: Constraint,
    pub particles: Vec<Particle>,
    pub connections: Vec<Connection>,
    pub cell_size: f32,
    pub grid: Grid<usize>,
}

impl Solver {
    pub fn new(
        constraint: Constraint,
        cell_size: f32,
        particles: &[Particle],
        connections: &[Connection],
    ) -> Self {
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
        //let time = Instant::now();
        self.populate_grid(); // TODO: for some reason it's slow in debug mode
                              //let elapsed = Instant::now() - time;
                              //println!("populate time: {}", 8.*elapsed.as_nanos() as f32 / 1000000.);

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
        self.connections
            .retain(|&(i, j, _)| i < self.particles.len() && j < self.particles.len());
        for &(i, j, link) in self.connections.iter() {
            let (i, j) = (std::cmp::min(i, j), std::cmp::max(i, j));
            let (head, tail) = self.particles.split_at_mut(i + 1);
            Solver::resolve_connection(&mut head[i], &mut tail[j - i - 1], link);
        }
    }

    pub fn resolve_collision(p1: &mut Particle, p2: &mut Particle) {
        let mut v = p1.pos - p2.pos;
        if v.length() < p1.radius + p2.radius {
            let overlap = (p1.radius + p2.radius - v.length());
            let c1 = p2.mass / (p1.mass + p2.mass);
            let c2 = p1.mass / (p1.mass + p2.mass);
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
        match p1.kind {
            //Kind::None => (),
            Kind::Motor(acc) => {
                let v = (p2.pos - p1.pos).normalize();
                let acceleration = v.perp() * acc;
                p2.accelerate(acceleration);
            }
            _ => (),
        }
    }

    pub fn resolve_connection(p1: &mut Particle, p2: &mut Particle, link: Link) {
        match link {
            Link::Force(force) => {
                let v = (p2.pos - p1.pos).normalize_or_zero();
                p1.accelerate(v * force);
                p2.accelerate(-v * force);
            }
            Link::Rigid(length) => {
                let mut v = p1.pos - p2.pos;
                let overlap = (length - v.length()) / 2.;
                v = overlap * v.normalize();
                p1.set_position(p1.pos + v, true);
                p2.set_position(p2.pos - v, true);
            }
        }
    }

    pub fn change_number(&mut self, number: usize) {
        if number < self.particles.len() {
            self.particles.truncate(number);
        } else {
            while self.particles.len() < number {
                let left = number - self.particles.len();
                //if left >= 20 {
                //    self.add_ring(0.3, 20);
                //}
                //else if left >= 4 {
                //    self.add_square(1.0);
                //}
                //else if left >= 3 {
                //    self.add_triangle(1.0);
                //}
                //else {self.add_particle(SAND.place(self.rnd_origin()));}
                let mut bounds = self.constraint.bounds();
                bounds.0.y = bounds.1.y * 0.8;
                let pos = rnd_in_bounds(bounds, 2. * PARTICLE_SIZE);
                if self.particles.len() % 10 == 0 {
                    self.add_particle(GROUND.position(pos));
                } else {
                    self.add_particle(GROUND.position(pos));
                }
            }
        }
    }

    pub fn size(&self) -> usize {
        self.particles.len()
    }

    pub fn add_particle(&mut self, particle: Particle) {
        self.particles.push(particle);
    }

    pub fn add_rib(&mut self, i: usize, j: usize, length: f32) {
        self.connections.push((i, j, Link::Rigid(length)))
    }

    pub fn add_spring(&mut self, i: usize, j: usize, force: f32) {
        self.connections.push((i, j, Link::Force(force)))
    }

    pub fn add_triangle(&mut self, length: f32) {
        self.add_ring(length, 3);
    }

    pub fn add_square(&mut self, length: f32) {
        let ind = self.particles.len();
        self.add_ring(length, 4);
        self.add_rib(ind, ind + 2, length * f32::sqrt(2.));
        self.add_rib(ind + 1, ind + 3, length * f32::sqrt(2.));
    }

    pub fn add_ring(&mut self, length: f32, number: usize) {
        let angle = PI / (number as f32);
        let radius = 0.5 * length / f32::atan(angle);
        let center = rnd_in_bounds(self.constraint.bounds(), radius);
        let ind = self.particles.len();
        for i in 0..number {
            let alpha = 2. * PI * (i as f32) / (number as f32);
            let pos = center + vec2(f32::cos(alpha), f32::sin(alpha)) * radius;
            self.add_particle(GROUND.position(pos));
            self.add_rib(ind + i, ind + ((i + 1) % number), length);
        }
    }

    pub fn add_tread(&mut self, pos: Vec2, power: f32, len: usize) {
        let ind = self.particles.len();
        let len =  len as isize;

        let mut tread_indexes = vec![];
        let mut tread_ind = self.particles.len();
        // lower tread
        for i in -len..=len {
            self.add_particle(
                METAL.position(pos + vec2(2. * PARTICLE_SIZE * i as f32, -f32::sqrt(3.) / 2.)),
            );
            tread_indexes.push(tread_ind);
            tread_ind += 1;
        }
        self.add_particle(METAL.position(pos + vec2(2. * PARTICLE_SIZE * (len as f32 + 0.5), 0.)));
        tread_indexes.push(tread_ind);
        tread_ind += 1;

        // upper tread
        for i in (-len..=len).rev() {
            self.add_particle(
                METAL.position(pos + vec2(2. * PARTICLE_SIZE * i as f32, f32::sqrt(3.) / 2.)),
            );
            tread_indexes.push(tread_ind);
            tread_ind += 1;
        }
        self.add_particle(METAL.position(pos + vec2(-2. * PARTICLE_SIZE * (len as f32 + 0.5), 0.)));
        tread_ind += 1;

        let total = 2 + 2*(2*len+1) as usize;
        for i in 0..total {
            self.add_rib(ind + i, ind + ((i + 1) % total), 2. * PARTICLE_SIZE);
        }

        // spikes
        let mut spike_ind = self.particles.len();
        for i in (-len..=len).step_by(2) {
            self.add_particle(
                SPIKE.position(pos + vec2(2. * PARTICLE_SIZE * i as f32, -f32::sqrt(3.) / 2. - SPIKE.radius - METAL.radius)),
            );
            self.add_rib(tread_indexes[(i+len) as usize], spike_ind, SPIKE.radius + METAL.radius);
            spike_ind += 1;
        }

        for i in (-len..=len).rev().step_by(2) {
            self.add_particle(
                SPIKE.position(pos + vec2(2. * PARTICLE_SIZE * i as f32, f32::sqrt(3.) / 2. + SPIKE.radius + METAL.radius)),
            );
            self.add_rib(tread_indexes[(-i+len + 2*len+2) as usize], spike_ind, SPIKE.radius + METAL.radius);
            spike_ind += 1;
        }

        // motors
        let ind  = self.particles.len();
        self.add_particle(
            MOTOR
                .position(pos + vec2(-2. * PARTICLE_SIZE * (len as f32 - 0.5), 0.))
                .kind(Kind::Motor(power)),
        );
        self.add_particle(
            MOTOR
            .position(pos)
            .kind(Kind::Motor(power)),
        );
        self.add_particle(
            MOTOR
                .position(pos + vec2(2. * PARTICLE_SIZE * (len as f32 - 0.5), 0.))
                .kind(Kind::Motor(power)),
        );
        self.add_rib(ind, ind + 1, (2*len - 1) as f32 / 2.);
        self.add_rib(ind+1, ind + 2, (2*len - 1) as f32 / 2.);
        self.add_rib(ind, ind + 2, (2*len - 1) as f32);
    }

    fn rnd_origin(&self) -> Vec2 {
        let bounds = self.constraint.bounds();
        rnd_in_bounds(bounds, 2. * PARTICLE_SIZE)
    }
}

pub fn rnd_in_bounds(bounds: (Vec2, Vec2), margin: f32) -> Vec2 {
    Vec2::new(
        rand::thread_rng().gen_range(bounds.0.x + margin..bounds.1.x - margin),
        rand::thread_rng().gen_range(bounds.0.y + margin..bounds.1.y - margin),
    )
}

#[derive(Clone, Copy)]
pub enum Link {
    Force(f32), // force
    Rigid(f32), // constant length
}

const CELL_MAX: usize = 4;

#[derive(Default, Clone)]
pub struct GridCell<T>
where
    T: Clone + Copy + Default,
{
    pub len: usize,
    pub elements: [T; CELL_MAX],
}

impl<T> GridCell<T>
where
    T: Clone + Copy + Default,
{
    pub fn push(&mut self, elem: T) {
        if self.len < CELL_MAX {
            self.elements[self.len] = elem;
            self.len += 1;
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn iter(&self) -> std::slice::Iter<T> {
        self.elements[0..self.len].iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<T> {
        self.elements[0..self.len].iter_mut()
    }
}

#[derive(Clone)]
pub struct Grid<T>
where
    T: Clone + Copy + Default,
{
    pub width: usize,
    pub height: usize,
    grid: Vec<GridCell<T>>,
}

impl<T> Index<(usize, usize)> for Grid<T>
where
    T: Clone + Copy + Default,
{
    type Output = GridCell<T>;
    fn index(&self, (i, j): (usize, usize)) -> &Self::Output {
        let ind = i * self.height + j;
        &self.grid[ind]
    }
}

impl<T> IndexMut<(usize, usize)> for Grid<T>
where
    T: Clone + Copy + Default,
{
    fn index_mut(&mut self, (i, j): (usize, usize)) -> &mut Self::Output {
        let ind = i * self.height + j;
        &mut self.grid[ind]
    }
}

impl<T> Grid<T>
where
    T: Clone + Copy + Default,
{
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            grid: vec![GridCell::<T>::default(); width * height],
        }
    }

    pub fn clear(&mut self) {
        for cell in self.grid.iter_mut() {
            cell.clear()
        }
    }

    pub fn push(&mut self, ind: (usize, usize), value: T) {
        self[ind].push(value);
    }
}

#[derive(Clone, Copy)]
pub enum Constraint {
    Box(Vec2, Vec2), // Rectangle, bottom-left and top-right corners
    #[deprecated]
    Cup(Vec2, Vec2), // U-shape, bottom-left and top-right corners
}

impl Constraint {
    pub const fn bounds(&self) -> (Vec2, Vec2) {
        match self {
            &Constraint::Box(bl, tr) => (bl, tr),
            &Constraint::Cup(bl, tr) => (bl, tr),
        }
    }
}
