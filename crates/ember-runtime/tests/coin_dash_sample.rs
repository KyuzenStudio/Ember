use std::collections::HashMap;
use std::path::PathBuf;

use ember_runtime::{AudioCmd, InputState, Runtime, ScriptDef};

fn sample_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/coin-dash")
}

fn load_sample() -> (Runtime, PathBuf) {
    let dir = sample_dir();
    let src = std::fs::read_to_string(dir.join("scenes/main.ron")).unwrap();
    let scene = ember_scene::Scene::load_from_str(&src, "scenes/main.ron").unwrap();
    let mut defs = Vec::new();
    for entity in &scene.entities {
        for entry in &entity.scripts {
            let source = std::fs::read_to_string(dir.join(&entry.path)).unwrap();
            defs.push(ScriptDef {
                entity: entity.id,
                file: entry.path.clone(),
                source,
                values: HashMap::new(),
            });
        }
    }
    let mut logs = Vec::new();
    let rt = Runtime::load(scene, defs, &mut logs).unwrap();
    (rt, dir)
}

fn pos(rt: &Runtime, name: &str) -> (f32, f32) {
    let e = rt.scene().entities.iter().find(|e| e.name == name).unwrap();
    (e.transform.position.x, e.transform.position.y)
}

#[test]
fn sample_textures_decode() {
    let (rt, dir) = load_sample();
    let mut count = 0;
    for entity in &rt.scene().entities {
        for path in entity.texture_paths() {
            ember_renderer::load_png_file(&dir.join(path))
                .unwrap_or_else(|e| panic!("{path}: {e}"));
            count += 1;
        }
    }
    assert!(count >= 5, "expected at least 5 textures, got {count}");
}

#[test]
fn sample_plays_from_start_to_win() {
    let (mut rt, _dir) = load_sample();
    assert!(
        rt.take_audio()
            .contains(&AudioCmd::Loop("assets/music.wav".to_string())),
        "scene music must queue as loop"
    );
    let mut input = InputState::new();
    input.press(3);
    let mut logs = Vec::new();
    let mut seen_coin = false;
    let mut jumped1 = false;
    let mut jumped2 = false;
    let mut phase = 0;
    let mut won = false;
    for _ in 0..1800 {
        let (x, y) = pos(&rt, "Player");
        if phase == 0 {
            if x < 1.0 {
                jumped1 = false;
                jumped2 = false;
            }
            if !jumped1 && x > 5.0 && y < 1.2 {
                input.press(4);
                jumped1 = true;
            }
            if jumped1 && !jumped2 && x > 8.0 {
                input.press(4);
                jumped2 = true;
            }
            if x > 12.5 {
                phase = 1;
                input.release(3);
                input.press(2);
            }
        } else if phase == 1 && x < 10.4 {
            phase = 2;
            input.release(2);
            input.press(3);
        }
        rt.tick(&mut input, 1.0 / 60.0, &mut logs).unwrap();
        for cmd in rt.take_audio() {
            if cmd == AudioCmd::Play("assets/coin.wav".to_string()) {
                seen_coin = true;
            }
        }
        if logs.contains(&"WIN".to_string()) {
            won = true;
            break;
        }
    }
    assert!(won, "never reached WIN: {logs:?}");
    assert!(seen_coin, "no coin sfx queued");
    assert!(logs.contains(&"score:3".to_string()), "{logs:?}");
    for _ in 0..5 {
        rt.tick(&mut input, 1.0 / 60.0, &mut logs).unwrap();
    }
    assert_eq!(rt.take_title(), Some("WIN score:3".to_string()));
    let cam = pos(&rt, "MainCamera");
    let player = pos(&rt, "Player");
    assert_eq!(cam, player, "camera must follow player");
}
