use std::collections::HashMap;
use std::time::Instant;

use ember_runtime::{InputState, Runtime, ScriptDef};

const SCRIPT: &str = "var acc: f32 = 0.0\nfn on_update(dt: f32) {\nvar i: i32 = 0\nwhile i < 100 {\nacc = acc + dt\ntransform.move(dt, 0.0)\ni = i + 1\n}\n}\n";

fn bench_script() -> (Runtime, std::time::Duration) {
    let mut scene = ember_scene::Scene::new("perf");
    let id = scene.spawn("Mover");
    let start = Instant::now();
    let mut rt = Runtime::load(
        scene,
        vec![ScriptDef {
            entity: id,
            file: "perf.em".to_string(),
            source: SCRIPT.to_string(),
            values: HashMap::new(),
        }],
        &mut Vec::new(),
    )
    .unwrap();
    let input = InputState::new();
    for _ in 0..2000 {
        rt.tick(&input, 1.0 / 60.0, &mut Vec::new()).unwrap();
    }
    (rt, start.elapsed())
}

fn bench_physics() -> std::time::Duration {
    use ember_scene::{BodyKind, Collider2D, RigidBody2D};
    let mut scene = ember_scene::Scene::new("phys");
    for i in 0..50 {
        let id = scene.spawn(&format!("Box{i}"));
        let e = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
        e.transform.position = ember_core::Vec2::new(i as f32 * 0.1, 5.0);
        e.rigidbody = Some(RigidBody2D {
            kind: BodyKind::Dynamic,
            velocity: ember_core::Vec2::zero(),
            gravity_scale: 1.0,
        });
        e.collider = Some(Collider2D { hx: 0.5, hy: 0.5 });
    }
    let mut rt = Runtime::load(scene, vec![], &mut Vec::new()).unwrap();
    let input = InputState::new();
    let start = Instant::now();
    for _ in 0..600 {
        rt.tick(&input, 1.0 / 60.0, &mut Vec::new()).unwrap();
    }
    start.elapsed()
}

fn bench_compile() -> std::time::Duration {
    let start = Instant::now();
    for _ in 0..200 {
        ember_lang::compile::compile("perf.em", SCRIPT).unwrap();
    }
    start.elapsed()
}

fn draw_count() -> (usize, std::time::Duration) {
    let mut scene = ember_scene::Scene::new("draws");
    let mut sizes = HashMap::new();
    sizes.insert("t.png".to_string(), (64, 64));
    for i in 0..200 {
        let id = scene.spawn(&format!("S{i}"));
        let e = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
        e.sprite = Some(ember_scene::SpriteRenderer {
            texture: "t.png".to_string(),
            tint: [1.0, 1.0, 1.0, 1.0],
            sorting_order: 0,
        });
    }
    let start = Instant::now();
    let (_, draws) = ember_renderer::scene_sprites(&scene, &sizes);
    (draws.len(), start.elapsed())
}

fn gpu_frame_ms(draws_note: &str) -> String {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        flags: wgpu::InstanceFlags::empty(),
        memory_budget_thresholds: Default::default(),
        backend_options: Default::default(),
        display: None,
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: true,
        compatible_surface: None,
        apply_limit_buckets: false,
    }));
    let Ok(adapter) = adapter else {
        return format!("skipped (no adapter; {draws_note})");
    };
    let device = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("perf"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        experimental_features: Default::default(),
        memory_hints: Default::default(),
        trace: Default::default(),
    }));
    let Ok((device, queue)) = device else {
        return "skipped (no device)".to_string();
    };
    let dir = std::env::temp_dir().join("ember-perf");
    std::fs::create_dir_all(&dir).unwrap();
    for (n, rgb) in [(255u8, 0u8, 0u8), (0u8, 255u8, 0u8)].iter().enumerate() {
        let img = image::RgbImage::from_pixel(64, 64, image::Rgb([rgb.0, rgb.1, rgb.2]));
        img.save(dir.join(format!("t{n}.png"))).unwrap();
    }
    let mut off = ember_renderer::Offscreen::new(&device, &queue, 800, 600);
    for n in 0..2 {
        let key = format!("t{n}.png");
        let img = ember_renderer::load_png_file(&dir.join(&key)).expect("generated png decodes");
        off.cache_texture(&key, &img).expect("texture cached");
    }
    let mut scene = ember_scene::Scene::new("gpu");
    let mut sizes = HashMap::new();
    sizes.insert("t0.png".to_string(), (64, 64));
    sizes.insert("t1.png".to_string(), (64, 64));
    for i in 0..200 {
        let id = scene.spawn(&format!("G{i}"));
        let e = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
        e.sprite = Some(ember_scene::SpriteRenderer {
            texture: format!("t{}.png", i % 2),
            tint: [1.0, 1.0, 1.0, 1.0],
            sorting_order: 0,
        });
    }
    let (camera, draws) = ember_renderer::scene_sprites(&scene, &sizes);
    assert_eq!(draws.len(), 200);
    for _ in 0..10 {
        off.render(&camera, &draws).unwrap();
    }
    let start = Instant::now();
    let frames = 120u32;
    for _ in 0..frames {
        off.render(&camera, &draws).unwrap();
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);
    let _ = std::fs::remove_dir_all(&dir);
    format!("{ms:.3} ms/frame (200 sprites, 800x600 offscreen)")
}

#[test]
fn write_baseline() {
    let (rt, vm_total) = bench_script();
    let vm_per_tick_us = vm_total.as_secs_f64() * 1e6 / 2000.0;
    let phys_total = bench_physics();
    let phys_per_tick_us = phys_total.as_secs_f64() * 1e6 / 600.0;
    let comp_total = bench_compile();
    let comp_per_us = comp_total.as_secs_f64() * 1e6 / 200.0;
    let (draws, draws_time) = draw_count();
    assert_eq!(draws, 200);
    let gpu = gpu_frame_ms("cpu draws ok");
    let mover_x = rt
        .scene()
        .entities
        .iter()
        .find(|e| e.name == "Mover")
        .unwrap()
        .transform
        .position
        .x;
    assert!(mover_x > 0.0, "bench script did not move entity");

    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let section = format!(
        "<!-- PERF:{profile} -->\n## Profil {profile} ({} {})\n\nUji: `cargo test -p ember-runtime --test perf_baseline` (menulis ulang bagian ini).\n\n| Metrik | Nilai |\n| --- | --- |\n| VM update (2000 tick, 1 skrip, loop 100) | {vm_per_tick_us:.2} us/tick |\n| Fisika (600 tick, 50 body dinamis) | {phys_per_tick_us:.2} us/tick |\n| Kompilasi skrip kecil (200x) | {comp_per_us:.1} us/kompilasi |\n| Draw count (200 sprite, CPU `scene_sprites`) | {draws} draws, {:.1} us |\n| Frame GPU | {gpu} |\n<!-- /PERF:{profile} -->\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        draws_time.as_secs_f64() * 1e6,
    );
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/PERF_BASELINE.md");
    let start_marker = format!("<!-- PERF:{profile} -->");
    let end_marker = format!("<!-- /PERF:{profile} -->");
    let file = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| "# Baseline performa (M15)\n".to_string());
    let updated = match (file.find(&start_marker), file.find(&end_marker)) {
        (Some(s), Some(e)) => {
            format!("{}{}{}", &file[..s], section, &file[e + end_marker.len()..])
        }
        _ => format!("{file}\n{section}"),
    };
    std::fs::write(&path, updated).unwrap();
}
