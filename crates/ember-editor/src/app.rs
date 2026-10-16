#[path = "inspector.rs"]
pub(crate) mod inspector;
#[path = "play.rs"]
mod play;
#[path = "project.rs"]
mod project;
#[path = "state.rs"]
mod state;
#[path = "ui.rs"]
mod ui;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use ember_scene::Scene;

pub use self::state::EditorApp;
#[allow(unused_imports)]
pub(crate) use self::ui::screen_to_world;

impl EditorApp {
    pub fn new(dir: &Path) -> Self {
        let mut app = Self {
            dir: dir.to_path_buf(),
            project_name: String::new(),
            scene: Scene::new("untitled"),
            scene_rel: ember_project::STARTUP_SCENE.to_string(),
            selection: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            anim_clip_name: "walk".to_string(),
            anim_frame_path: String::new(),
            scene_music_buf: String::new(),
            status: String::new(),
            console: Vec::new(),
            diag: Vec::new(),
            scripts: Vec::new(),
            diag_sel: None,
            assets: Vec::new(),
            import_src: String::new(),
            comp_search: String::new(),
            add_menu_open: false,
            off: None,
            tex: None,
            tex_size: (0, 0),
            sizes: HashMap::new(),
            missing: HashSet::new(),
            frame: 0,
            script_cache: HashMap::new(),
            new_name: "Entity".to_string(),
            rename: String::new(),
            play: None,
            last_phys_sig: Vec::new(),
            title_restore: false,
            audio: None,
        };
        app.reload();
        app
    }

    pub fn playing(&self) -> bool {
        self.play.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::state::VarRow;
    use super::ui::log_line;
    use super::*;
    use ember_core::Vec2;
    use ember_renderer::{Camera, pick_top, sprite_quad};
    use ember_runtime::InputState as GameInput;
    use ember_scene::{EntityId, ScriptValue, SpriteRenderer};
    use ember_vm::Value;
    use std::path::PathBuf;

    #[test]
    fn console_caps_lines() {
        let mut log = Vec::new();
        for i in 0..600 {
            log_line(&mut log, format!("{i}"));
        }
        assert_eq!(log.len(), 500);
        assert_eq!(log[0], "100");
    }

    fn test_app(tag: &str) -> (PathBuf, EditorApp) {
        let base = std::env::temp_dir().join(format!("ember-editor-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        ember_project::create_project(&base.join("Game"), "Game").unwrap();
        let app = EditorApp::new(&base.join("Game"));
        (base, app)
    }

    fn pos_of(scene: &Scene, name: &str) -> (f32, f32) {
        let e = scene.entities.iter().find(|e| e.name == name).unwrap();
        (e.transform.position.x, e.transform.position.y)
    }

    #[test]
    fn play_runs_and_stop_preserves_edit() {
        let (base, mut app) = test_app("play");
        assert!(!app.playing());
        app.start_play();
        assert!(app.playing());
        let edit_before = app.scene.save_to_string().unwrap();
        let mut input = GameInput::new();
        input.press(3);
        app.tick_play(&mut input, 1.0 / 60.0);
        let (rx, _) = pos_of(app.play.as_ref().unwrap().runtime.scene(), "Player");
        assert!(rx > 0.0);
        assert_eq!(pos_of(&app.scene, "Player").0, 0.0);
        app.stop_play();
        assert!(!app.playing());
        assert_eq!(app.scene.save_to_string().unwrap(), edit_before);
        let _ = std::fs::remove_dir_all(&base);
    }

    fn tiny_wav() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&44u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&44100u32.to_le_bytes());
        bytes.extend_from_slice(&88200u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 8]);
        bytes
    }

    #[test]
    fn play_drains_audio_queue() {
        let (base, mut app) = test_app("audio");
        let dir = base.join("Game");
        std::fs::write(dir.join("assets").join("s.wav"), tiny_wav()).unwrap();
        std::fs::write(
            dir.join("scripts").join("player.em"),
            "fn on_update(dt: f32) {\naudio.play(\"assets/s.wav\")\n}\n",
        )
        .unwrap();
        app.start_play();
        assert!(app.playing());
        app.tick_play(&mut GameInput::new(), 1.0 / 60.0);
        let engine = app.audio.as_ref().unwrap();
        assert!(
            engine.recorded().iter().any(|r| r.contains("s.wav")),
            "{:?}",
            engine.recorded()
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn stop_silences_music() {
        let (base, mut app) = test_app("stopmusic");
        let dir = base.join("Game");
        std::fs::write(dir.join("assets").join("m.wav"), tiny_wav()).unwrap();
        std::fs::write(
            dir.join("scripts").join("player.em"),
            "fn on_create() {\naudio.loop(\"assets/m.wav\")\n}\nfn on_update(dt: f32) {\n}\n",
        )
        .unwrap();
        app.start_play();
        assert!(app.playing());
        app.tick_play(&mut GameInput::new(), 1.0 / 60.0);
        app.stop_play();
        assert!(!app.playing());
        let recorded = app.audio.as_ref().unwrap().recorded();
        assert_eq!(recorded.last().map(String::as_str), Some("stop"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn undo_redo_add_entity() {
        let (base, mut app) = test_app("undoadd");
        let before = app.scene.save_to_string().unwrap();
        EditorApp::push_undo(&mut app.undo, &mut app.redo, &app.scene);
        let id = app.scene.spawn("Extra");
        app.select_only(id);
        assert!(app.scene.entities.iter().any(|e| e.id == id));
        app.undo();
        assert_eq!(app.scene.save_to_string().unwrap(), before);
        assert!(app.selection.is_empty());
        app.redo();
        assert!(app.scene.entities.iter().any(|e| e.id == id));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn move_selection_shifts_all() {
        let (base, mut app) = test_app("undomove");
        let a = app.scene.spawn("A");
        let b = app.scene.spawn("B");
        app.selection = vec![a, b];
        EditorApp::push_undo(&mut app.undo, &mut app.redo, &app.scene);
        app.move_selection(2.0, -1.0);
        for id in [a, b] {
            let e = app.scene.entities.iter().find(|e| e.id == id).unwrap();
            assert_eq!(
                (e.transform.position.x, e.transform.position.y),
                (2.0, -1.0)
            );
        }
        app.undo();
        for id in [a, b] {
            let e = app.scene.entities.iter().find(|e| e.id == id).unwrap();
            assert_eq!((e.transform.position.x, e.transform.position.y), (0.0, 0.0));
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn delete_selection_undoable() {
        let (base, mut app) = test_app("undodel");
        let a = app.scene.spawn("A");
        let b = app.scene.spawn("B");
        app.selection = vec![a, b];
        app.delete_selection();
        assert!(!app.scene.entities.iter().any(|e| e.id == a));
        assert!(app.selection.is_empty());
        app.undo();
        assert!(app.scene.entities.iter().any(|e| e.id == a));
        assert!(app.scene.entities.iter().any(|e| e.id == b));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn toggle_select_adds_and_removes() {
        let (base, mut app) = test_app("togglesel");
        let a = app.scene.spawn("A");
        let b = app.scene.spawn("B");
        app.select_only(a);
        app.toggle_select(b);
        assert_eq!(app.selection, vec![a, b]);
        app.toggle_select(a);
        assert_eq!(app.selection, vec![b]);
        assert_eq!(app.primary(), Some(b));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scene_music_set_and_cleared() {
        let (base, mut app) = test_app("scenemusic");
        let dir = base.join("Game");
        std::fs::write(dir.join("assets").join("m.ogg"), b"OggS....").unwrap();
        assert!(app.set_scene_music("assets/m.ogg").is_ok());
        assert_eq!(app.scene.music, Some("assets/m.ogg".to_string()));
        assert!(app.set_scene_music("assets/nope.wav").is_err());
        assert!(app.set_scene_music("assets/player.png").is_err());
        assert!(app.set_scene_music("").is_ok());
        assert!(app.scene.music.is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn play_restart_rebuilds_runtime() {
        let (base, mut app) = test_app("restart");
        let dir = base.join("Game");
        std::fs::write(
            dir.join("scripts").join("player.em"),
            "var n: i32 = 0\nfn on_update(dt: f32) {\nn = n + 1\nif n > 1 {\ngame.restart()\n}\n}\n",
        )
        .unwrap();
        app.start_play();
        assert!(app.playing());
        app.tick_play(&mut GameInput::new(), 1.0 / 60.0);
        assert!(app.playing());
        app.tick_play(&mut GameInput::new(), 1.0 / 60.0);
        assert!(app.playing());
        assert!(app.console.iter().any(|l| l == "restarted"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn play_broken_script_stays_edit() {
        let (base, mut app) = test_app("broken");
        let dir = base.join("Game");
        std::fs::write(dir.join("scripts").join("player.em"), "var x = \n").unwrap();
        app.start_play();
        assert!(!app.playing());
        assert!(app.status.contains("player.em:1:"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn tick_error_stops_play() {
        let (base, mut app) = test_app("tickerr");
        let dir = base.join("Game");
        std::fs::write(
            dir.join("scripts").join("player.em"),
            "fn on_update(dt: f32) {\nvar x: i32 = 1 / 0\n}\n",
        )
        .unwrap();
        app.start_play();
        assert!(app.playing());
        app.tick_play(&mut GameInput::new(), 1.0 / 60.0);
        assert!(!app.playing());
        assert!(app.status.contains("division by zero"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn screen_center_maps_to_camera_center() {
        let cam = Camera {
            center: Vec2::new(10.0, 20.0),
            height: 12.0,
        };
        let world = screen_to_world((400.0, 300.0), (800.0, 600.0), &cam, 4.0 / 3.0);
        assert!((world.x - 10.0).abs() < 1e-4);
        assert!((world.y - 20.0).abs() < 1e-4);
    }

    #[test]
    fn screen_corner_maps_to_edge() {
        let cam = Camera {
            center: Vec2::zero(),
            height: 12.0,
        };
        let world = screen_to_world((800.0, 0.0), (800.0, 600.0), &cam, 4.0 / 3.0);
        assert!((world.x - 8.0).abs() < 1e-4);
        assert!((world.y - 6.0).abs() < 1e-4);
    }

    #[test]
    fn pick_prefers_topmost() {
        let a = sprite_quad(Vec2::zero(), Vec2::new(4.0, 4.0), 0.0);
        let b = sprite_quad(Vec2::new(3.0, 0.0), Vec2::new(4.0, 4.0), 0.0);
        assert_eq!(pick_top(&[a, b], Vec2::new(2.5, 0.0)), Some(1));
        assert_eq!(pick_top(&[a, b], Vec2::new(-1.5, 0.0)), Some(0));
        assert_eq!(pick_top(&[a, b], Vec2::new(9.0, 0.0)), None);
    }

    #[test]
    fn draw_order_matches_pick_order() {
        use ember_renderer::scene_sprites;
        let mut scene = Scene::new("t");
        let back = scene.spawn("Back");
        let front = scene.spawn("Front");
        for (id, order) in [(back, 5), (front, -2)] {
            let e = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
            e.sprite = Some(SpriteRenderer {
                texture: "a.png".to_string(),
                tint: [1.0, 1.0, 1.0, 1.0],
                sorting_order: order,
            });
        }
        let mut sizes = HashMap::new();
        sizes.insert("a.png".to_string(), (8, 8));
        let (_, draws) = scene_sprites(&scene, &sizes);
        assert_eq!(draws.len(), 2);
        let mut order: Vec<(i32, EntityId)> = scene
            .entities
            .iter()
            .filter_map(|e| e.sprite.as_ref().map(|s| (s.sorting_order, e.id)))
            .collect();
        order.sort_by_key(|(seq, _)| *seq);
        assert_eq!(order[0].1, front);
        assert_eq!(order[1].1, back);
    }

    fn script_app(tag: &str) -> (PathBuf, EditorApp) {
        let base =
            std::env::temp_dir().join(format!("ember-editor-vars-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        ember_project::create_project(&base.join("Game"), "Game").unwrap();
        let app = EditorApp::new(&base.join("Game"));
        (base, app)
    }

    fn rows_for(app: &mut EditorApp) -> Result<Vec<VarRow>, String> {
        let (script, overrides) = app
            .selection
            .first()
            .copied()
            .and_then(|id| app.scene.entities.iter().find(|e| e.id == id))
            .and_then(|e| {
                e.scripts
                    .first()
                    .map(|s| (s.path.clone(), s.values.clone()))
            })
            .unwrap();
        app.script_rows(Some(script), &overrides)
    }

    #[test]
    fn script_rows_show_defaults() {
        let (base, mut app) = script_app("defaults");
        let id = app
            .scene
            .entities
            .iter()
            .find(|e| e.name == "Player")
            .unwrap()
            .id;
        app.selection = vec![id];
        let rows = rows_for(&mut app).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "speed");
        assert_eq!(rows[0].value, ScriptValue::F32(6.0));
        assert!(!rows[0].overridden);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn script_rows_show_override() {
        let (base, mut app) = script_app("override");
        let id = app
            .scene
            .entities
            .iter()
            .find(|e| e.name == "Player")
            .unwrap()
            .id;
        app.scene
            .entities
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .scripts
            .iter_mut()
            .find(|s| s.path == "scripts/player.em")
            .unwrap()
            .values
            .insert("speed".to_string(), ScriptValue::F32(6.0));
        app.selection = vec![id];
        let rows = rows_for(&mut app).unwrap();
        assert_eq!(rows[0].value, ScriptValue::F32(6.0));
        assert!(rows[0].overridden);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn script_rows_broken_script() {
        let (base, mut app) = script_app("broken");
        let dir = base.join("Game");
        std::fs::write(dir.join("scripts").join("player.em"), "var x = \n").unwrap();
        let id = app
            .scene
            .entities
            .iter()
            .find(|e| e.name == "Player")
            .unwrap()
            .id;
        app.selection = vec![id];
        assert!(rows_for(&mut app).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn play_edits_revert_on_stop() {
        let (base, mut app) = script_app("playedit");
        app.start_play();
        assert!(app.playing());
        let id = app
            .scene
            .entities
            .iter()
            .find(|e| e.name == "Player")
            .unwrap()
            .id;
        {
            let scene = app.play.as_mut().unwrap().runtime.scene_mut();
            let entity = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
            entity.transform.position.x = 9.0;
            entity
                .scripts
                .iter_mut()
                .find(|s| s.path == "scripts/player.em")
                .unwrap()
                .values
                .insert("speed".to_string(), ScriptValue::F32(1.0));
        }
        app.play
            .as_mut()
            .unwrap()
            .runtime
            .set_global(id, "scripts/player.em", "speed", Value::F32(1.0))
            .unwrap();
        assert_eq!(
            app.play.as_ref().unwrap().runtime.global(id, "speed"),
            Some(Value::F32(1.0))
        );
        app.stop_play();
        assert!(!app.playing());
        let entity = app.scene.entities.iter().find(|e| e.id == id).unwrap();
        assert_eq!(entity.transform.position.x, 0.0);
        assert!(entity.scripts.iter().all(|s| s.values.is_empty()));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn play_runs_physics() {
        use ember_scene::{BodyKind, Collider2D, RigidBody2D};
        let (base, mut app) = script_app("playphys");
        let id = app
            .scene
            .entities
            .iter()
            .find(|e| e.name == "Player")
            .unwrap()
            .id;
        let entity = app.scene.entities.iter_mut().find(|e| e.id == id).unwrap();
        entity.rigidbody = Some(RigidBody2D {
            kind: BodyKind::Dynamic,
            velocity: Vec2::zero(),
            gravity_scale: 1.0,
        });
        entity.collider = Some(Collider2D {
            hx: 0.5,
            hy: 0.5,
            sensor: false,
        });
        app.start_play();
        assert!(app.playing());
        for _ in 0..60 {
            app.tick_play(&mut GameInput::new(), 1.0 / 60.0);
        }
        let runtime_y = app
            .play
            .as_ref()
            .unwrap()
            .runtime
            .scene()
            .entities
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .transform
            .position
            .y;
        assert!(runtime_y < -1.0, "player did not fall: y = {runtime_y}");
        app.stop_play();
        assert_eq!(
            app.scene
                .entities
                .iter()
                .find(|e| e.id == id)
                .unwrap()
                .transform
                .position
                .y,
            0.0
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
