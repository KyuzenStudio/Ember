mod backend;
mod frame;
mod image;
mod sprite;
mod types;

pub use backend::run_sprite_window;
pub use frame::{Offscreen, SceneRenderer};
pub(crate) use image::effective_size;
pub use image::{decode_png, load_png_file};
pub use sprite::{fit_quad, pick_top, scene_sprites, sprite_quad};
pub use types::{
    Camera, DecodeError, DecodedImage, DrawReq, GpuError, MAX_SPRITES, QUAD_INDICES, RunError,
    SpriteVertex,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::SHADER;
    use ember_core::Vec2;
    use std::path::Path;

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
    fn draws_use_anim_frame() {
        use ember_scene::{AnimClip, SpriteAnim, SpriteRenderer};
        let mut scene = ember_scene::Scene::new("anim");
        let id = scene.spawn("Hero");
        let e = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
        e.sprite = Some(SpriteRenderer {
            texture: "idle.png".to_string(),
            tint: [1.0, 1.0, 1.0, 1.0],
            sorting_order: 0,
        });
        e.anim = Some(SpriteAnim {
            clips: vec![AnimClip {
                name: "walk".to_string(),
                frames: vec!["w0.png".to_string(), "w1.png".to_string()],
                fps: 2.0,
                looped: true,
            }],
            current: "walk".to_string(),
            t: 0.6,
        });
        let mut sizes = std::collections::HashMap::new();
        sizes.insert("idle.png".to_string(), (8, 8));
        sizes.insert("w0.png".to_string(), (8, 8));
        sizes.insert("w1.png".to_string(), (8, 8));
        let (_, draws) = scene_sprites(&scene, &sizes);
        assert_eq!(draws.len(), 1);
        assert_eq!(draws[0].texture, "w1.png");
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
