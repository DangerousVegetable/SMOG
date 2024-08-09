use bevy::{math::Vec2, render::render_resource::VertexBufferLayout};
use wgpu::{vertex_attr_array, BufferAddress, VertexAttribute, VertexStepMode};

#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub pos: Vec2,
    pub uv: Vec2,
}

impl Vertex {
    const ATTRIBS: [VertexAttribute; 2] = vertex_attr_array![
        // position
        0 => Float32x2,
        // uv
        1 => Float32x2,
    ];

    pub fn desc() -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: Self::ATTRIBS.into(),
        }
    }
}