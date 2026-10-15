use std::fmt;
use std::path::Path;
use std::sync::Arc;

use ember_core::Vec2;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const SHADER: &str = "
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@location(0) p: vec2<f32>, @location(1) uv: vec2<f32>) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, uv);
}
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError(String);

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

pub fn decode_png(bytes: &[u8]) -> Result<DecodedImage, DecodeError> {
    if bytes.is_empty() {
        return Err(DecodeError("empty PNG input".to_string()));
    }
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| DecodeError(format!("invalid PNG ({e})")))?;
    let rgba = img.to_rgba8();
    Ok(DecodedImage {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

pub fn load_png_file(path: &Path) -> Result<DecodedImage, DecodeError> {
    let bytes = std::fs::read(path)
        .map_err(|e| DecodeError(format!("cannot read '{}': {e}", path.display())))?;
    decode_png(&bytes).map_err(|e| DecodeError(format!("'{}': {e}", path.display())))
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

impl SpriteVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub const QUAD_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];

pub fn fit_quad(tex_size: Vec2, surface_size: Vec2) -> [SpriteVertex; 4] {
    let blank = [SpriteVertex {
        position: [0.0, 0.0],
        uv: [0.0, 0.0],
    }; 4];
    if tex_size.x <= 0.0 || tex_size.y <= 0.0 || surface_size.x <= 0.0 || surface_size.y <= 0.0 {
        return blank;
    }
    let scale = (surface_size.x / tex_size.x).min(surface_size.y / tex_size.y) * 0.9;
    let hx = tex_size.x * scale / surface_size.x;
    let hy = tex_size.y * scale / surface_size.y;
    [
        SpriteVertex {
            position: [-hx, hy],
            uv: [0.0, 0.0],
        },
        SpriteVertex {
            position: [hx, hy],
            uv: [1.0, 0.0],
        },
        SpriteVertex {
            position: [hx, -hy],
            uv: [1.0, 1.0],
        },
        SpriteVertex {
            position: [-hx, -hy],
            uv: [0.0, 1.0],
        },
    ]
}

pub fn effective_size(width: u32, height: u32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 {
        None
    } else {
        Some((width, height))
    }
}

#[derive(Debug, Clone)]
pub struct GpuError(String);

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gpu error: {}", self.0)
    }
}

impl std::error::Error for GpuError {}

#[derive(Debug, Clone)]
pub struct RunError(String);

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

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    tex_size: Vec2,
}

impl Gpu {
    fn new(window: Arc<Window>, img: &DecodedImage) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| GpuError(format!("create surface: {e}")))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| GpuError(format!("no usable adapter: {e}")))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|e| GpuError(format!("request device: {e}")))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| GpuError("surface reports no texture formats".to_string()))?;

        let inner = window.inner_size();
        let (width, height) = effective_size(inner.width, inner.height).unwrap_or((800, 600));
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let tex_size = wgpu::Extent3d {
            width: img.width,
            height: img.height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sprite"),
            size: tex_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * img.width),
                rows_per_image: Some(img.height),
            },
            tex_size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite bind layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite bind group"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite pipeline layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[Some(SpriteVertex::layout())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        let tex_size_vec = Vec2::new(img.width as f32, img.height as f32);
        let surf_size = Vec2::new(width as f32, height as f32);
        let verts = fit_quad(tex_size_vec, surf_size);
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite verts"),
            size: (size_of::<SpriteVertex>() * 4) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&verts));
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite indices"),
            size: (size_of::<u16>() * QUAD_INDICES.len()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&QUAD_INDICES));

        Ok(Self {
            device,
            queue,
            surface,
            config,
            pipeline,
            bind_group,
            vertex_buffer,
            index_buffer,
            tex_size: tex_size_vec,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        let Some((w, h)) = effective_size(width, height) else {
            return;
        };
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        let verts = fit_quad(self.tex_size, Vec2::new(w as f32, h as f32));
        self.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&verts));
    }

    fn render(&mut self) -> Result<(), GpuError> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.surface.configure(&self.device, &self.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(()),
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sprite pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.07,
                            g: 0.07,
                            b: 0.09,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..QUAD_INDICES.len() as u32, 0, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        Ok(())
    }
}

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    image: Option<DecodedImage>,
    error: Option<String>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Ember Sprite")
            .with_inner_size(LogicalSize::new(800.0, 600.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(format!("create window: {e}"));
                event_loop.exit();
                return;
            }
        };
        let Some(img) = self.image.take() else {
            self.error = Some("sprite image missing".to_string());
            event_loop.exit();
            return;
        };
        match Gpu::new(window.clone(), &img) {
            Ok(gpu) => {
                self.window = Some(window);
                self.gpu = Some(gpu);
            }
            Err(e) => {
                self.error = Some(e.to_string());
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = &self.window else {
            return;
        };
        if window_id != window.id() {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &mut self.gpu
                    && let Err(e) = gpu.render()
                {
                    self.error = Some(e.to_string());
                    event_loop.exit();
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        state: winit::event::ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

pub fn run_sprite_window(png_path: &Path) -> Result<(), RunError> {
    let img = load_png_file(png_path).map_err(RunError::from)?;
    if img.width == 0 || img.height == 0 {
        return Err(RunError("sprite has zero size".to_string()));
    }
    let event_loop = EventLoop::new().map_err(|e| RunError(format!("create event loop: {e}")))?;
    let mut app = App {
        window: None,
        gpu: None,
        image: Some(img),
        error: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| RunError(format!("run event loop: {e}")))?;
    if let Some(msg) = app.error {
        return Err(RunError(msg));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_png_bytes() -> Vec<u8> {
        let mut raw = vec![0u8; 8 * (1 + 8 * 4)];
        for y in 0..8 {
            for x in 0..8 {
                let at = y * (1 + 8 * 4) + 1 + x * 4;
                raw[at..at + 4].copy_from_slice(&[200, 40, 40, 255]);
            }
        }
        let mut out = Vec::new();
        out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&8u32.to_be_bytes());
        ihdr.extend_from_slice(&8u32.to_be_bytes());
        ihdr.push(8);
        ihdr.push(6);
        ihdr.push(0);
        ihdr.push(0);
        ihdr.push(0);
        push_chunk(&mut out, b"IHDR", &ihdr);
        push_chunk(&mut out, b"IDAT", &zlib_store(&raw));
        push_chunk(&mut out, b"IEND", &[]);
        out
    }

    fn push_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(tag);
        out.extend_from_slice(data);
        out.extend_from_slice(&crc32(&[tag.as_slice(), data].concat()).to_be_bytes());
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

    fn zlib_store(raw: &[u8]) -> Vec<u8> {
        let mut out = vec![0x78, 0x01];
        let mut chunks = raw.chunks(65535);
        let first = chunks.next().unwrap_or(&[]);
        let mut rest: Vec<&[u8]> = chunks.collect();
        let last = if rest.is_empty() {
            first
        } else {
            let l = rest.pop().unwrap_or(&[]);
            write_stored_block(&mut out, first, false);
            for c in rest {
                write_stored_block(&mut out, c, false);
            }
            l
        };
        write_stored_block(&mut out, last, true);
        let adler = adler32(raw);
        out.extend_from_slice(&adler);
        out
    }

    fn write_stored_block(out: &mut Vec<u8>, data: &[u8], last: bool) {
        out.push(if last { 1 } else { 0 });
        out.extend_from_slice(&(data.len() as u16).to_le_bytes());
        out.extend_from_slice(&(!(data.len() as u16)).to_le_bytes());
        out.extend_from_slice(data);
    }

    fn adler32(data: &[u8]) -> [u8; 4] {
        let mut a = 1u32;
        let mut b = 0u32;
        for &byte in data {
            a = (a + byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        ((b << 16) | a).to_be_bytes()
    }

    #[test]
    fn decode_valid_png() {
        let img = decode_png(&solid_png_bytes()).unwrap();
        assert_eq!((img.width, img.height), (8, 8));
        assert_eq!(img.rgba.len(), 8 * 8 * 4);
        assert_eq!(&img.rgba[0..4], &[200, 40, 40, 255]);
    }

    #[test]
    fn demo_asset_decodes() {
        let bytes = include_bytes!("../../../assets/sprite-demo.png");
        let img = decode_png(bytes).unwrap();
        assert_eq!((img.width, img.height), (128, 128));
        assert_eq!(img.rgba.len(), 128 * 128 * 4);
    }

    #[test]
    fn decode_rejects_empty() {
        assert!(decode_png(&[]).is_err());
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode_png(b"not a png").is_err());
    }

    #[test]
    fn load_missing_file_mentions_path() {
        let err = load_png_file(Path::new("nope/missing.png")).unwrap_err();
        assert!(err.to_string().contains("nope/missing.png"));
    }

    #[test]
    fn fit_quad_preserves_aspect() {
        let q = fit_quad(Vec2::new(4.0, 2.0), Vec2::new(800.0, 600.0));
        let w = q[1].position[0] - q[0].position[0];
        let h = q[0].position[1] - q[2].position[1];
        let pixel_ratio = (w * 800.0) / (h * 600.0);
        assert!((pixel_ratio - 2.0).abs() < 1e-5);
        for v in &q {
            assert!(v.position[0].abs() <= 1.0);
            assert!(v.position[1].abs() <= 1.0);
        }
    }

    #[test]
    fn fit_quad_square_surface_fills() {
        let q = fit_quad(Vec2::new(64.0, 64.0), Vec2::new(600.0, 600.0));
        assert!((q[1].position[0] - 0.9).abs() < 1e-5);
        assert!((q[0].position[1] - 0.9).abs() < 1e-5);
    }

    #[test]
    fn fit_quad_degenerate_is_blank() {
        let q = fit_quad(Vec2::new(0.0, 8.0), Vec2::new(800.0, 600.0));
        assert!(q.iter().all(|v| v.position == [0.0, 0.0]));
    }

    #[test]
    fn effective_size_rejects_zero() {
        assert_eq!(effective_size(0, 600), None);
        assert_eq!(effective_size(800, 0), None);
        assert_eq!(effective_size(800, 600), Some((800, 600)));
    }
}
