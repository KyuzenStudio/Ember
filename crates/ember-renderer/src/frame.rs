use std::sync::Arc;

use winit::window::Window;

use crate::backend::{make_buffers, make_sampler, sprite_bind_layout, sprite_pipeline};
use crate::effective_size;
use crate::sprite::build_verts;
use crate::types::{Camera, DecodedImage, DrawReq, GpuError, MAX_SPRITES, SpriteVertex};

pub(crate) struct CachedTexture {
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

pub(crate) fn find_texture<'a>(
    textures: &'a [CachedTexture],
    key: &str,
) -> Option<&'a CachedTexture> {
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

pub struct Offscreen {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: Vec<CachedTexture>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    pub(crate) texture: wgpu::Texture,
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
