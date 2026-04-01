use bytemuck::{Pod, Zeroable};
use wgpu::VertexBufferLayout;

/// Vertex for the solid-color pipeline (ellipsoid globe).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexPos {
    pub position: [f32; 3],
}

/// Vertex for the textured pipeline (glTF mesh content).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexPosTex {
    pub position: [f32; 3],
    pub texcoord: [f32; 2],
}

impl VertexPos {
    pub const LAYOUT: VertexBufferLayout<'static> = VertexBufferLayout {
        array_stride: size_of::<VertexPos>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
    };
}

impl VertexPosTex {
    pub const LAYOUT: VertexBufferLayout<'static> = VertexBufferLayout {
        array_stride: size_of::<VertexPosTex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2],
    };
}
