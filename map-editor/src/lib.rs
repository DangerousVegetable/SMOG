pub mod constructor {
    use std::ops::Range;

    use bevy::{
        asset::Handle,
        color::Color,
        math::{vec2, Vec2, Vec4},
        prelude::Image,
    };
    use image::{Rgba, RgbaImage};
    use rand::Rng;
    use solver::{particle::Particle, Connection, Constraint, Link, Solver, PARTICLE_RADIUS};

    use crate::map::{Map, Spawn};

    pub struct TriangularGrid<T> {
        bounds: (Vec2, Vec2),
        pub width: usize,
        pub height: usize,
        pub grid: Vec<T>,
    }

    impl<T> TriangularGrid<T>
    where
        T: Default + Clone + Copy,
    {
        const X_SHIFT: f32 = PARTICLE_RADIUS * 2.;
        const Y_SHIFT: f32 = 1.7320508075688772935274463415059 * PARTICLE_RADIUS; // sqrt(3) * radius

        pub fn new(constraint: Constraint) -> Self {
            let (bl, tr) = constraint.bounds();
            let width = ((tr.x - bl.x) / Self::X_SHIFT) as usize + 3;
            let height = ((tr.y - bl.y) / Self::Y_SHIFT) as usize + 3;
            Self {
                bounds: (bl, tr),
                width: width,
                height: height,
                grid: vec![T::default(); width * height],
            }
        }

        pub fn get(&self, (i, j): (usize, usize)) -> &T {
            let ind = i * self.height + j;
            &self.grid[ind]
        }

        pub fn get_mut(&mut self, (i, j): (usize, usize)) -> &mut T {
            let ind = i * self.height + j;
            &mut self.grid[ind]
        }

        /// Get coordinates of the cell (i, j) assuming (1, 1) is in the bottom-left corner
        pub fn get_position(&self, (i, j): (usize, usize)) -> Vec2 {
            if j % 2 == 1 {
                let (i, j) = (i as f32, j as f32);
                let x = (i - 1.) * Self::X_SHIFT + self.bounds.0.x + PARTICLE_RADIUS;
                let y = (j - 1.) * Self::Y_SHIFT + self.bounds.0.y + PARTICLE_RADIUS;
                vec2(x, y)
            } else {
                let (i, j) = (i as f32, j as f32);
                let x = i * Self::X_SHIFT + self.bounds.0.x;
                let y = (j - 1.) * Self::Y_SHIFT + self.bounds.0.y + PARTICLE_RADIUS;
                vec2(x, y)
            }
        }

        pub fn for_adjacent<F: FnMut(&T)>(&self, (i, j): (usize, usize), mut f: F) {
            if j % 2 == 1 {
                f(self.get((i, j)));

                f(self.get((i + 1, j)));
                f(self.get((i - 1, j)));

                f(self.get((i, j + 1)));
                f(self.get((i - 1, j + 1)));

                f(self.get((i, j - 1)));
                f(self.get((i - 1, j - 1)));
            } else {
                f(self.get((i, j)));

                f(self.get((i + 1, j)));
                f(self.get((i - 1, j)));

                f(self.get((i + 1, j + 1)));
                f(self.get((i, j + 1)));

                f(self.get((i + 1, j - 1)));
                f(self.get((i, j - 1)));
            }
        }

        pub fn for_each<F: FnMut(Vec2, &T)>(&self, mut f: F) {
            let (bl, tr) = self.bounds;
            for i in 1..self.width - 1 {
                for j in 1..self.height - 1 {
                    let pos = self.get_position((i, j));
                    if pos.x <= tr.x - PARTICLE_RADIUS && pos.y <= tr.y - PARTICLE_RADIUS {
                        f(pos, self.get((i, j)));
                    }
                }
            }
        }

        pub fn for_each_mut<F: FnMut(Vec2, &mut T)>(&mut self, mut f: F) {
            let (bl, tr) = self.bounds;
            for i in 1..self.width - 1 {
                for j in 1..self.height - 1 {
                    let pos = self.get_position((i, j));
                    if pos.x <= tr.x - PARTICLE_RADIUS && pos.y <= tr.y - PARTICLE_RADIUS {
                        f(pos, self.get_mut((i, j)));
                    }
                }
            }
        }
    }
    pub struct Layer {
        constraint: Constraint,
        grid: TriangularGrid<Option<(usize, Rgba<u8>)>>,
        pub base_particle: Particle,
        pub link: Option<Link>,
        pub strength: f32,
        pub particles: Option<Vec<Particle>>,
        pub connections: Option<Vec<Connection>>,
    }

    impl Layer {
        pub fn new(
            constraint: Constraint,
            base_particle: Particle,
            link: Option<Link>,
            strength: f32,
        ) -> Self {
            let grid = TriangularGrid::new(constraint);
            Self {
                constraint,
                grid,
                base_particle,
                link,
                strength,
                particles: None,
                connections: None,
            }
        }

        pub fn init_from_image(&mut self, image: Image) {
            let image: RgbaImage = image.try_into_dynamic().unwrap().to_rgba8();
            let (width, height) = (
                self.grid.bounds.1.x - self.grid.bounds.0.x,
                self.grid.bounds.1.y - self.grid.bounds.0.y,
            );
            let (scale_x, scale_y) = (image.width() as f32 / width, image.height() as f32 / height);
            let bl = self.grid.bounds.0;

            let mut ind = 0;
            self.grid.for_each_mut(|pos, v| {
                let offset_pos = pos - bl; // get position of the particle as if the bl = (0, 0)
                let (i, j) = (
                    (offset_pos.x * scale_x) as u32,
                    image.height() - (offset_pos.y * scale_y) as u32,
                );

                if let Some(pixel) = image.get_pixel_checked(i, j) {
                    if pixel.0[3] > 0 {
                        *v = Some((ind, *pixel));
                        ind += 1;
                    }
                }
            });
        }

        pub fn get_particles(&self) -> Vec<Particle> {
            let mut particles = vec![];
            self.grid.for_each(|pos, v| {
                if let Some((_ind, color)) = *v {
                    let color = color.0.map(|c| c as f32 / 255.);
                    let color = Color::srgba(color[0], color[1], color[2], color[3]).to_linear();
                    let color = Vec4::new(color.red, color.green, color.blue, color.alpha);
                    particles.push(self.base_particle.with_position(pos).with_color(color));
                }
            });
            particles
        }

        pub fn get_connections(&self) -> Vec<Connection> {
            let mut connections_num = 0;
            let Some(link) = self.link else {
                return vec![];
            };

            for i in 1..self.grid.width - 1 {
                for j in 1..self.grid.height - 1 {
                    let pos = (i, j);
                    if let Some((ind, _color)) = self.grid.get(pos) {
                        self.grid.for_adjacent(pos, |p| {
                            if let Some((p_ind, _)) = p {
                                if p_ind > ind {
                                    //connections.push((*ind, *p_ind, link));
                                    connections_num += 1;
                                }
                            }
                        })
                    }
                }
            }

            let mut connections = vec![];
            let particles = self.get_particles();
            let mut rng = rand::thread_rng();
            for _ in 0..(connections_num as f32 * self.strength) as usize {
                let i = rng.gen_range(0..particles.len());
                let j = rng.gen_range(0..particles.len());
                let dist = (particles[i].pos - particles[j].pos).length();
                if dist > 0. {
                    connections.push((i, j, link.with_length(dist)));
                }
            }

            connections
        }

        pub fn bake(&mut self) {
            self.particles = Some(self.get_particles());
            self.connections = Some(self.get_connections());
        }

        pub fn solver(&mut self) -> Solver {
            if self.particles.is_none() || self.connections.is_none() {
                self.bake();
            }
            let particles = self.particles.as_ref().unwrap();
            let connections = self.connections.as_ref().unwrap();
            Solver::new(self.constraint, particles, connections)
        }
    }
    pub struct MapConstructor {
        pub name: String,
        pub constraint: Constraint,
        pub layers: Vec<Layer>,
        pub spawns: Vec<Spawn>,
        pub textures: Vec<Handle<Image>>,

        pub particles: Option<Vec<Particle>>,
        pub connections: Option<Vec<Connection>>,
    }

    impl MapConstructor {
        pub fn new(name: String, constraint: Constraint) -> Self {
            Self {
                name,
                constraint,
                layers: vec![],
                spawns: vec![],
                textures: vec![],
                particles: None,
                connections: None
            }
        }

        pub fn add_layer(&mut self) {
            self.layers
                .push(Layer::new(self.constraint, Particle::default(), None, 1.))
        }

        pub fn bake_layers(&mut self) {
            let mut particles = vec![];
            let mut connections = vec![];
            let mut offset = 0;
            for layer in self.layers.iter_mut() {
                layer.bake();
                particles.append(&mut layer.particles.as_mut().unwrap().clone());

                let layer_connections = layer.connections.as_ref().unwrap();
                for (i, j, link) in layer_connections.iter() {
                    connections.push((*i + offset, *j + offset, *link));
                }

                offset = particles.len();
            }
            self.particles = Some(particles);
            self.connections = Some(connections);
        }

        pub fn solver(&mut self) -> Solver {
            if self.particles.is_none() || self.connections.is_none() {
                self.bake_layers();
            }
            let particles = self.particles.as_ref().unwrap();
            let connections = self.connections.as_ref().unwrap();
            Solver::new(self.constraint, particles, connections)
        }

        pub fn map(&mut self) -> Map {
            if self.particles.is_none() || self.connections.is_none() {
                self.bake_layers();
            }
            let particles = self.particles.as_ref().unwrap().clone();
            let connections = self.connections.as_ref().unwrap().clone();
            Map {
                name: self.name.clone(),
                constraint: self.constraint,
                particles,
                connections,
                spawns: self.spawns.clone(),
                textures_num: self.textures.len(),
            }
        }
    }
}

pub mod map {
    use bevy::math::Vec2;
    use solver::{particle::Particle, Connection, Constraint, Solver};

    #[derive(PartialEq, Clone)]
    pub struct Spawn {
        pub pos: Vec2,
        pub team: usize,
    }
    pub struct Map {
        pub name: String,
        pub constraint: Constraint,
        pub particles: Vec<Particle>,
        pub connections: Vec<Connection>,
        pub spawns: Vec<Spawn>,
        pub textures_num: usize,
    }

    impl Map {
        pub fn solver(&self) -> Solver {
            Solver::new(self.constraint, &self.particles, &self.connections)
        }
    }
}
