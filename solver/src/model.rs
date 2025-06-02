use std::ops::Add;

use bevy::math::{vec2, Vec2};

use crate::{particle::Particle, Connection};

#[derive(Default, Debug)]
pub struct Model {
    pub center: Vec2,
    pub particles: Vec<Particle>,
    pub connections: Vec<Connection>,
}

impl Add for Model {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        let mut output = self;

        let offset = output.center - rhs.center;
        let particles_num = output.particles.len();
        output.particles.extend(
            rhs.particles
                .into_iter()
                .map(|p| p.with_position(p.pos + offset)),
        );
        output.connections.extend(
            rhs.connections
                .into_iter()
                .map(|(i, j, link)| (i + particles_num, j + particles_num, link)),
        );

        output
    }
}

pub const SHIFT_X: Vec2 = vec2(1., 0.);
pub const SHIFT_Y: Vec2 = vec2(0.5, 0.86602540378443864676372317075294);

/// Macro to create particle models.
#[macro_export]
macro_rules! model {
    ( $($p:expr $(;$l:expr)? => $(.offset:$offset:expr,)? .hex:$hex:literal [$($(@$part_var:ident =)? $x:expr, $y:expr);*] $(+ [$($(@$conn_var:ident =)? $(.global:$global_i:literal)? $($i:expr),* => $(.global:$global_j:literal)? $($j:expr),*);*] )? )* ) => {
        {
            use $crate::model::{SHIFT_X, SHIFT_Y, Model};
            use bevy::math::vec2;

            let mut particles = Vec::new();
            let mut connections = Vec::new();
            $(
                let _particles_num = particles.len();
                let mut _offset = vec2(0., 0.);
                $(
                    _offset = $offset;
                )?
                $(
                    let _ind = particles.len();
                    if $hex {
                        particles.push($p.with_position(SHIFT_X*$x as f32 + SHIFT_Y*$y as f32 + _offset));
                    }
                    else {
                        particles.push($p.with_position(vec2($x as f32, $y as f32) + _offset));
                    }
                    $(
                        $part_var = _ind;
                    )?
                )*
                $(

                    $(
                        let mut ind_i = Vec::new();
                        let mut ind_j = Vec::new();
                        let mut global_i = _particles_num;
                        let mut global_j = _particles_num;
                        $(
                            if $global_i {
                                global_i = 0;
                            }
                        )?
                        $(
                            if $global_j {
                                global_j = 0;
                            }
                        )?
                        $(
                            ind_i.push($i + global_i);
                        )*
                        $(
                            ind_j.push($j + global_j);
                        )*
                        for i in ind_i.iter() {
                            for j in ind_j.iter() {
                                let length = particles[*i].pos.distance(particles[*j].pos);
                                let _ind = connections.len();
                                connections.push((*i, *j, $l.with_length(length)));
                                $(
                                    $conn_var = _ind;
                                )?
                            }
                        }
                    )*
                )?
            )*
            
            Model {
                particles,
                connections,
                ..Default::default()
            }
        }
    };
}

/// Macro to create chained-particle models (i.e. tank treads).
#[macro_export]
macro_rules! chain_model {
    ($p:expr; $l:expr; $($step:literal=>$adj_p:expr; $adj_l:expr)? => .start:$start:expr; $($direction:ident : $num:literal),*) => {
        {
            use $crate::model::{SHIFT_X, SHIFT_Y, Model};
            use $crate::particle::Particle;
            use bevy::math::vec2;

            let mut particles: Vec<Particle> = Vec::new();
            let mut connections = Vec::new();
            
            let mut total = 0;
            let mut _step = 1;
            let mut last_ind = None;
            let mut last_pos = $start;
            
            let adj: Vec<Particle> = vec![$($adj_p)?];
            let mut _adj_l = $l;
            $(
                _step = $step;
                _adj_l = $adj_l;
            )?

            $(
                //let last_pos: Vec2 = last_ind.map_or($start, |ind: usize| particles[ind].pos);
                let direction = {
                    match stringify!($direction) {
                        "r" => {
                            SHIFT_X
                        },
                        "ur" => {
                            SHIFT_Y
                        },
                        "ul" => {
                            -SHIFT_X + SHIFT_Y
                        },
                        "l" => {
                            -SHIFT_X
                        },
                        "dl" => {
                            -SHIFT_Y
                        },
                        "dr" => {
                            SHIFT_X - SHIFT_Y
                        },
                        _ => vec2(0., 0.,)
                    }
                };

                for _ in 0..$num {
                    let _ind = particles.len();
                    particles.push($p.with_position(last_pos));
                    let _perp = direction.perp();
                    if total%_step == 0 {
                        for adj_p in adj.iter() {
                            let offset = $p.radius + adj_p.radius;
                            particles.push(adj_p.with_position(last_pos - _perp*offset));
                            connections.push((_ind, _ind+1, _adj_l.with_length(offset)));
                        }
                    }
                    last_pos += direction;
                    if let Some(ind) = last_ind {
                        connections.push((ind, _ind, $l.with_length(1.)));
                    }
                    last_ind = Some(_ind);
                    total += 1;
                }
            )*

            if let Some(ind) = last_ind {
                if ind > 0 {
                    connections.push((ind, 0, $l.with_length(1.)));
                }
            }

            Model {
                particles,
                connections,
                ..Default::default()
            }
        }
    }
}


#[allow(unused_mut)]
#[cfg(test)]
mod tests {
    use crate::{
        particle::{GROUND, METAL},
        Link,
    };

    #[test]
    fn model_test() {
        let mut main_particle;
        let v = model! {
            METAL => .hex:true [0,1]
            GROUND; Link::Rigid { length: 1., durability: 1., elasticity: 10.} => .hex:true [@main_particle = 0,0.5; 1,0; 1,1] + [0=>1,2; 1=>2]
        };
        println!("{v:?}");
        assert_eq!(1, main_particle);
        assert_eq!(4, v.particles.len());
        assert_eq!(3, v.connections.len());
    }
    #[test]
    fn chain_model_test() {
        let chain = chain_model![
            METAL; Link::Rigid { length: 1., durability: 1., elasticity: 10.}; => .start:vec2(0., 0.); 
            r:2, ur:2, ul:2, l:2, dl:2, dr: 2
        ];

        assert_eq!(chain.particles.len(), 12);
        assert_eq!(chain.connections.len(), 12);
        dbg!(chain);
    }
}
