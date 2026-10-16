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
    @location(1) color: vec4<f32>,
};

@vertex
fn vs(@location(0) p: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs(@location(0) uv: vec2<f32>, @location(1) color: vec4<f32>) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, uv) * color;
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

pub fn fit_quad(tex_size: Vec2, surface_size: Vec2) -> [SpriteVertex; 4] {
    let blank = [SpriteVertex {
        position: [0.0, 0.0],
        uv: [0.0, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
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
            color: [1.0, 1.0, 1.0, 1.0],
        },
        SpriteVertex {
            position: [hx, hy],
            uv: [1.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
        },
        SpriteVertex {
            position: [hx, -hy],
            uv: [1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        },
        SpriteVertex {
            position: [-hx, -hy],
            uv: [0.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
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
        let bind_layout = sprite_bind_layout(&device);
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

        let pipeline = sprite_pipeline(&device, &bind_layout, format);

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

fn sprite_bind_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
    })
}

fn sprite_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sprite shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("sprite pipeline layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
    })
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

pub fn sprite_quad(center: Vec2, size: Vec2, rotation_degrees: f32) -> [Vec2; 4] {
    let angle = rotation_degrees.to_radians();
    let (sin, cos) = angle.sin_cos();
    let hx = size.x / 2.0;
    let hy = size.y / 2.0;
    [(-hx, hy), (hx, hy), (hx, -hy), (-hx, -hy)]
        .map(|(x, y)| Vec2::new(center.x + x * cos - y * sin, center.y + x * sin + y * cos))
}

pub fn scene_sprites(
    scene: &ember_scene::Scene,
    sizes: &std::collections::HashMap<String, (u32, u32)>,
) -> (Camera, Vec<DrawReq>) {
    let camera = scene
        .entities
        .iter()
        .find_map(|e| {
            e.camera.as_ref().map(|c| Camera {
                center: e.transform.position,
                height: c.height,
            })
        })
        .unwrap_or(Camera {
            center: Vec2::zero(),
            height: 12.0,
        });
    let mut ordered: Vec<(i32, DrawReq)> = Vec::new();
    for entity in &scene.entities {
        let Some(sprite) = &entity.sprite else {
            continue;
        };
        let Some((w, h)) = sizes.get(&sprite.texture).copied() else {
            continue;
        };
        let aspect = w as f32 / h.max(1) as f32;
        let size = Vec2::new(aspect * entity.transform.scale.x, entity.transform.scale.y);
        let corners = sprite_quad(
            entity.transform.position,
            size,
            entity.transform.rotation_degrees,
        );
        ordered.push((
            sprite.sorting_order,
            DrawReq {
                texture: sprite.texture.clone(),
                corners,
                tint: sprite.tint,
            },
        ));
    }
    ordered.sort_by_key(|(order, _)| *order);
    (camera, ordered.into_iter().map(|(_, d)| d).collect())
}

pub fn pick_top(quads: &[[Vec2; 4]], point: Vec2) -> Option<usize> {
    quads.iter().rposition(|quad| {
        let signs: Vec<f32> = quad
            .iter()
            .enumerate()
            .map(|(i, corner)| {
                let next = quad[(i + 1) % 4];
                let edge = next - *corner;
                let to_point = point - *corner;
                edge.x * to_point.y - edge.y * to_point.x
            })
            .collect();
        signs.iter().all(|s| *s >= 0.0) || signs.iter().all(|s| *s <= 0.0)
    })
}

struct CachedTexture {
    key: String,
    width: u32,
    height: u32,
    bind_group: wgpu::BindGroup,
}

pub struct SceneRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: Vec<CachedTexture>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl SceneRenderer {
    pub fn new(window: Arc<Window>) -> Result<Self, GpuError> {
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
        let layout = sprite_bind_layout(&device);
        let pipeline = sprite_pipeline(&device, &layout, format);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene verts"),
            size: (size_of::<SpriteVertex>() * MAX_SPRITES * 4) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut indices = Vec::with_capacity(MAX_SPRITES * 6);
        for i in 0..MAX_SPRITES as u16 {
            indices.extend_from_slice(&[i * 4, i * 4 + 1, i * 4 + 2, i * 4, i * 4 + 2, i * 4 + 3]);
        }
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene indices"),
            size: (size_of::<u16>() * indices.len()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&indices));
        Ok(Self {
            device,
            queue,
            surface,
            config,
            pipeline,
            layout,
            sampler,
            textures: Vec::new(),
            vertex_buffer,
            index_buffer,
        })
    }

    pub fn cache_texture(&mut self, key: &str, img: &DecodedImage) -> Result<(), GpuError> {
        upload_texture(
            &self.device,
            &self.queue,
            &self.layout,
            &self.sampler,
            &mut self.textures,
            key,
            img,
        )
    }

    pub fn texture_size(&self, key: &str) -> Option<(u32, u32)> {
        find_texture(&self.textures, key).map(|t| (t.width, t.height))
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let Some((w, h)) = effective_size(width, height) else {
            return;
        };
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height.max(1) as f32
    }

    pub fn draw(&mut self, camera: &Camera, sprites: &[DrawReq]) -> Result<(), GpuError> {
        if sprites.len() > MAX_SPRITES {
            return Err(GpuError(format!("too many sprites: {}", sprites.len())));
        }
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
        let aspect = self.aspect();
        let verts = build_verts(camera, aspect, &self.textures, sprites)?;
        self.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&verts));
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        run_pass(&PassArgs {
            device: &self.device,
            queue: &self.queue,
            pipeline: &self.pipeline,
            vertex_buffer: &self.vertex_buffer,
            index_buffer: &self.index_buffer,
            textures: &self.textures,
            view: &view,
            sprites,
        })?;
        self.queue.present(frame);
        Ok(())
    }
}

fn find_texture<'a>(textures: &'a [CachedTexture], key: &str) -> Option<&'a CachedTexture> {
    textures.iter().find(|t| t.key == key)
}

fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    textures: &mut Vec<CachedTexture>,
    key: &str,
    img: &DecodedImage,
) -> Result<(), GpuError> {
    if textures.iter().any(|t| t.key == key) {
        return Ok(());
    }
    if img.width == 0 || img.height == 0 {
        return Err(GpuError(format!("texture '{key}' has zero size")));
    }
    let size = wgpu::Extent3d {
        width: img.width,
        height: img.height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sprite"),
        size,
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
        size,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sprite bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    textures.push(CachedTexture {
        key: key.to_string(),
        width: img.width,
        height: img.height,
        bind_group,
    });
    Ok(())
}

fn build_verts(
    camera: &Camera,
    aspect: f32,
    textures: &[CachedTexture],
    sprites: &[DrawReq],
) -> Result<Vec<SpriteVertex>, GpuError> {
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mut verts = Vec::with_capacity(sprites.len() * 4);
    for sprite in sprites {
        find_texture(textures, &sprite.texture)
            .ok_or_else(|| GpuError(format!("texture '{}' not cached", sprite.texture)))?;
        for (i, corner) in sprite.corners.iter().enumerate() {
            let p = camera.ndc(*corner, aspect);
            verts.push(SpriteVertex {
                position: p,
                uv: uvs[i],
                color: sprite.tint,
            });
        }
    }
    Ok(verts)
}

struct PassArgs<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    pipeline: &'a wgpu::RenderPipeline,
    vertex_buffer: &'a wgpu::Buffer,
    index_buffer: &'a wgpu::Buffer,
    textures: &'a [CachedTexture],
    view: &'a wgpu::TextureView,
    sprites: &'a [DrawReq],
}

fn run_pass(args: &PassArgs) -> Result<(), GpuError> {
    let PassArgs {
        device,
        queue,
        pipeline,
        vertex_buffer,
        index_buffer,
        textures,
        view,
        sprites,
    } = args;
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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
        pass.set_pipeline(pipeline);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        for (i, sprite) in sprites.iter().enumerate() {
            let tex = find_texture(textures, &sprite.texture)
                .ok_or_else(|| GpuError(format!("texture '{}' not cached", sprite.texture)))?;
            pass.set_bind_group(0, &tex.bind_group, &[]);
            let base = i as u32 * 6;
            pass.draw_indexed(base..base + 6, 0, 0..1);
        }
    }
    queue.submit([encoder.finish()]);
    Ok(())
}

fn make_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    })
}

fn make_buffers(device: &wgpu::Device, queue: &wgpu::Queue) -> (wgpu::Buffer, wgpu::Buffer) {
    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scene verts"),
        size: (size_of::<SpriteVertex>() * MAX_SPRITES * 4) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut indices = Vec::with_capacity(MAX_SPRITES * 6);
    for i in 0..MAX_SPRITES as u16 {
        indices.extend_from_slice(&[i * 4, i * 4 + 1, i * 4 + 2, i * 4, i * 4 + 2, i * 4 + 3]);
    }
    let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scene indices"),
        size: (size_of::<u16>() * indices.len()) as u64,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&indices));
    (vertex_buffer, index_buffer)
}

pub struct Offscreen {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: Vec<CachedTexture>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

impl Offscreen {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32) -> Self {
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let layout = sprite_bind_layout(device);
        let pipeline = sprite_pipeline(device, &layout, format);
        let sampler = make_sampler(device);
        let (vertex_buffer, index_buffer) = make_buffers(device, queue);
        let (w, h) = (width.max(1), height.max(1));
        let texture = Self::make_target(device, format, w, h);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            layout,
            sampler,
            textures: Vec::new(),
            vertex_buffer,
            index_buffer,
            texture,
            view,
            width: w,
            height: h,
            format,
        }
    }

    fn make_target(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("editor viewport"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let (w, h) = effective_size(width, height).unwrap_or((self.width, self.height));
        if w == self.width && h == self.height {
            return;
        }
        self.width = w;
        self.height = h;
        self.texture = Self::make_target(&self.device, self.format, w, h);
        self.view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
    }

    pub fn cache_texture(&mut self, key: &str, img: &DecodedImage) -> Result<(), GpuError> {
        upload_texture(
            &self.device,
            &self.queue,
            &self.layout,
            &self.sampler,
            &mut self.textures,
            key,
            img,
        )
    }

    pub fn texture_size(&self, key: &str) -> Option<(u32, u32)> {
        find_texture(&self.textures, key).map(|t| (t.width, t.height))
    }

    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height.max(1) as f32
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn render(&mut self, camera: &Camera, sprites: &[DrawReq]) -> Result<(), GpuError> {
        if sprites.len() > MAX_SPRITES {
            return Err(GpuError(format!("too many sprites: {}", sprites.len())));
        }
        let verts = build_verts(camera, self.aspect(), &self.textures, sprites)?;
        self.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&verts));
        run_pass(&PassArgs {
            device: &self.device,
            queue: &self.queue,
            pipeline: &self.pipeline,
            vertex_buffer: &self.vertex_buffer,
            index_buffer: &self.index_buffer,
            textures: &self.textures,
            view: &self.view,
            sprites,
        })
    }
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

    #[test]
    fn camera_centers_origin() {
        let cam = Camera {
            center: Vec2::zero(),
            height: 12.0,
        };
        assert_eq!(cam.ndc(Vec2::zero(), 16.0 / 9.0), [0.0, 0.0]);
        let edge = cam.ndc(Vec2::new(0.0, 6.0), 1.0);
        assert!((edge[1] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn quad_no_rotation() {
        let q = sprite_quad(Vec2::new(1.0, 2.0), Vec2::new(4.0, 2.0), 0.0);
        assert_eq!(q[0], Vec2::new(-1.0, 3.0));
        assert_eq!(q[2], Vec2::new(3.0, 1.0));
    }

    #[test]
    fn quad_right_angle() {
        let q = sprite_quad(Vec2::zero(), Vec2::new(4.0, 2.0), 90.0);
        assert!((q[0].x + 1.0).abs() < 1e-5);
        assert!((q[0].y + 2.0).abs() < 1e-5);
    }

    #[test]
    fn shader_parses() {
        naga::front::wgsl::parse_str(SHADER).expect("shader must parse");
    }

    #[test]
    fn offscreen_renders_sprite() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        }))
        .expect("headless adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("headless device");
        let mut off = Offscreen::new(&device, &queue, 64, 64);
        let bytes = include_bytes!("../../../assets/sprite-demo.png");
        let img = decode_png(bytes).unwrap();
        off.cache_texture("assets/player.png", &img).unwrap();
        let mut scene = ember_scene::Scene::new("t");
        let id = scene.spawn("Player");
        scene
            .entities
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .sprite = Some(ember_scene::SpriteRenderer {
            texture: "assets/player.png".to_string(),
            tint: [1.0, 1.0, 1.0, 1.0],
            sorting_order: 0,
        });
        let mut sizes = std::collections::HashMap::new();
        sizes.insert("assets/player.png".to_string(), (img.width, img.height));
        let (camera, draws) = scene_sprites(&scene, &sizes);
        assert_eq!(draws.len(), 1);
        off.render(&camera, &draws).unwrap();
        let pixels = readback(&device, &queue, &off);
        let center = &pixels[(32 * 64 + 32) * 4..][..4];
        let corner = &pixels[0..4];
        assert!(center[0] > 150, "center pixel dark: {center:?}");
        assert_ne!(center, corner);
    }

    fn readback(device: &wgpu::Device, queue: &wgpu::Queue, off: &Offscreen) -> Vec<u8> {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: 64 * 64 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &off.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(64 * 4),
                    rows_per_image: Some(64),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let data = slice.get_mapped_range().expect("mapped").to_vec();
        buf.unmap();
        data
    }
}
