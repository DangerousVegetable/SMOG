

use bevy::{math::{vec2, Vec2, Vec4}, render::render_resource::{BufferAddress, VertexAttribute, VertexBufferLayout, VertexStepMode}};
use wgpu::vertex_attr_array;

use super::vertex::Vertex;
use solver::particle::Particle;

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
#[repr(C)]
pub struct Raw {
    size: f32,
    pos: Vec2,
    texture: u32, 
    color: Vec4,
}

impl Raw {
    const ATTRIBS: [VertexAttribute; 4] = vertex_attr_array![
        // size
        2 => Float32,
        // position
        3 => Float32x2,
        // texture index
        4 => Uint32,
        // color
        5 => Float32x4,
    ];

    pub fn desc() -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as BufferAddress,
            step_mode: VertexStepMode::Instance,
            attributes: Self::ATTRIBS.into(),
        }
    }
}

impl Raw {
    pub fn from_particle(particle: &Particle) -> Raw {
        Raw {
            size: particle.radius,
            pos: particle.pos,
            texture: particle.texture,
            color: particle.color,
        }
    }

    pub const fn vertices() -> [Vertex; 4] {
        [
            Vertex {
                pos: vec2(-1.0, 1.0),
                uv: vec2(0.0, 0.0),
            },
            Vertex {
                pos: vec2(-1.0, -1.0),
                uv: vec2(0.0, 1.0)
            },
            Vertex {
                pos: vec2(1.0, -1.0),
                uv: vec2(1.0, 1.0),
            },
            Vertex {
                pos: vec2(1.0, 1.0),
                uv: vec2(1.0, 0.0),
            },
        ]
    }

    pub const fn indices() -> [u32; 6] {
        // two faces: 0-1-3 and 3-1-2
        [0,1,3,3,1,2]
    }
}