use std::fmt;

use ember_core::Vec2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError(pub(crate) String);

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for DecodeError {}

#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl SpriteVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub const QUAD_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];

#[derive(Debug, Clone)]
pub struct GpuError(pub(crate) String);

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gpu error: {}", self.0)
    }
}

impl std::error::Error for GpuError {}

#[derive(Debug, Clone)]
pub struct RunError(pub(crate) String);

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{0}", self.0)
    }
}

impl std::error::Error for RunError {}

impl From<DecodeError> for RunError {
    fn from(e: DecodeError) -> Self {
        Self(e.to_string())
    }
}

impl From<GpuError> for RunError {
    fn from(e: GpuError) -> Self {
        Self(e.to_string())
    }
}

pub const MAX_SPRITES: usize = 1024;

#[derive(Clone, Debug)]
pub struct Camera {
    pub center: Vec2,
    pub height: f32,
}

impl Camera {
    pub fn ndc(&self, p: Vec2, aspect: f32) -> [f32; 2] {
        let half_h = self.height / 2.0;
        let half_w = half_h * aspect;
        [
            (p.x - self.center.x) / half_w,
            (p.y - self.center.y) / half_h,
        ]
    }
}

#[derive(Clone, Debug)]
pub struct DrawReq {
    pub texture: String,
    pub corners: [Vec2; 4],
    pub tint: [f32; 4],
}
