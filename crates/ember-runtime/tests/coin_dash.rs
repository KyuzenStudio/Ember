use std::collections::HashMap;

use ember_core::Vec2;
use ember_runtime::{InputState, Runtime, ScriptDef};
use ember_scene::{BodyKind, Collider2D, RigidBody2D};

const PLAYER: &str = "var score: i32 = 0\nvar lives: i32 = 3\nvar won: i32 = 0\nfn on_update(dt: f32) {\nif input.down(Key.Right) {\ntransform.move(6.0 * dt, 0.0)\n}\nif input.down(Key.Left) {\ntransform.move(-6.0 * dt, 0.0)\n}\nif input.pressed(Key.Space) {\ntransform.move(0.0, 2.2)\n}\nif won == 1 {\ngame.set_title(str.cat(\"WIN score:\", str.of_i(score)))\n}\n}\nfn on_collision_begin(other: string) {\nif other == \"Coin1\" {\nentity.destroy(\"Coin1\")\nscore = score + 1\naudio.play(\"coin.wav\")\nlog(str.cat(\"score:\", str.of_i(score)))\n}\nif other == \"Coin2\" {\nentity.destroy(\"Coin2\")\nscore = score + 1\naudio.play(\"coin.wav\")\nlog(str.cat(\"score:\", str.of_i(score)))\n}\nif other == \"Enemy\" {\nlives = lives - 1\ntransform.set(0.0, 0.5)\naudio.play(\"hit.wav\")\nlog(str.cat(\"lives:\", str.of_i(lives)))\n}\nif score == 2 {\nwon = 1\naudio.stop()\nlog(\"WIN\")\n}\n}\n";

const CAM: &str =
    "fn on_update(dt: f32) {\ntransform.set(entity.x(\"Player\"), entity.y(\"Player\"))\n}\n";

const ENEMY: &str = "var dir: f32 = 1.0\nfn on_update(dt: f32) {\ntransform.move(dir * 3.0 * dt, 0.0)\nif transform.x() > 9.0 {\ndir = -1.0\n}\nif transform.x() < 7.0 {\ndir = 1.0\n}\n}\n";

fn solid(
    scene: &mut ember_scene::Scene,
    name: &str,
    x: f32,
    y: f32,
    kind: BodyKind,
    sensor: bool,
) -> ember_scene::EntityId {
    let id = scene.spawn(name);
    let e = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
    e.transform.position = Vec2::new(x, y);
    e.rigidbody = Some(RigidBody2D {
        kind,
        velocity: Vec2::zero(),
        gravity_scale: 1.0,
    });
    e.collider = Some(Collider2D {
        hx: 0.5,
        hy: 0.5,
        sensor,
    });
    id
}

fn coin_dash() -> (Runtime, ember_scene::EntityId) {
    let mut scene = ember_scene::Scene::new("dash");
    let ground = scene.spawn("Ground");
    let g = scene.entities.iter_mut().find(|e| e.id == ground).unwrap();
    g.transform.position = Vec2::new(5.0, -1.0);
    g.rigidbody = Some(RigidBody2D {
        kind: BodyKind::Static,
        velocity: Vec2::zero(),
        gravity_scale: 1.0,
    });
    g.collider = Some(Collider2D {
        hx: 12.0,
        hy: 1.0,
        sensor: false,
    });
    let player = solid(&mut scene, "Player", 0.0, 2.0, BodyKind::Dynamic, false);
    solid(&mut scene, "Coin1", 2.0, 0.5, BodyKind::Static, true);
    solid(&mut scene, "Coin2", 4.0, 0.5, BodyKind::Static, true);
    let enemy = solid(&mut scene, "Enemy", 8.0, 0.5, BodyKind::Static, false);
    let cam = scene.spawn("Cam");
    let defs = vec![
        ScriptDef {
            entity: player,
            file: "player.em".to_string(),
            source: PLAYER.to_string(),
            values: HashMap::new(),
        },
        ScriptDef {
            entity: cam,
            file: "cam.em".to_string(),
            source: CAM.to_string(),
            values: HashMap::new(),
        },
        ScriptDef {
            entity: enemy,
            file: "enemy.em".to_string(),
            source: ENEMY.to_string(),
            values: HashMap::new(),
        },
    ];
    let rt = Runtime::load(scene, defs, &mut Vec::new()).unwrap();
    (rt, player)
}

fn y_of(rt: &Runtime, name: &str) -> f32 {
    rt.scene()
        .entities
        .iter()
        .find(|e| e.name == name)
        .unwrap()
        .transform
        .position
        .y
}

fn x_of(rt: &Runtime, name: &str) -> f32 {
    rt.scene()
        .entities
        .iter()
        .find(|e| e.name == name)
        .unwrap()
        .transform
        .position
        .x
}

#[test]
fn jump_rises_and_lands() {
    let (mut rt, _) = coin_dash();
    let mut input = InputState::new();
    for _ in 0..60 {
        rt.tick(&mut input, 1.0 / 60.0, &mut Vec::new()).unwrap();
    }
    let rest = y_of(&rt, "Player");
    assert!((rest - 0.5).abs() < 0.1, "not resting: {rest}");
    input.press(4);
    rt.tick(&mut input, 1.0 / 60.0, &mut Vec::new()).unwrap();
    let apex = y_of(&rt, "Player");
    assert!(apex > rest + 1.5, "no jump: {apex}");
    for _ in 0..120 {
        rt.tick(&mut input, 1.0 / 60.0, &mut Vec::new()).unwrap();
    }
    let landed = y_of(&rt, "Player");
    assert!((landed - 0.5).abs() < 0.1, "did not land: {landed}");
}

#[test]
fn coins_score_and_win() {
    let (mut rt, _) = coin_dash();
    let mut input = InputState::new();
    input.press(3);
    let mut logs = Vec::new();
    for _ in 0..120 {
        rt.tick(&mut input, 1.0 / 60.0, &mut logs).unwrap();
    }
    assert!(rt.scene().entities.iter().all(|e| e.name != "Coin1"));
    assert!(rt.scene().entities.iter().all(|e| e.name != "Coin2"));
    assert!(logs.contains(&"score:1".to_string()), "{logs:?}");
    assert!(logs.contains(&"score:2".to_string()), "{logs:?}");
    assert!(logs.contains(&"WIN".to_string()), "{logs:?}");
    assert_eq!(rt.take_title(), Some("WIN score:2".to_string()));
    assert_eq!(x_of(&rt, "Cam"), x_of(&rt, "Player"));
    assert_eq!(y_of(&rt, "Cam"), y_of(&rt, "Player"));
}

#[test]
fn enemy_hit_loses_life_and_respawns() {
    let (mut rt, _) = coin_dash();
    let mut input = InputState::new();
    input.press(3);
    let mut logs = Vec::new();
    for _ in 0..400 {
        rt.tick(&mut input, 1.0 / 60.0, &mut logs).unwrap();
    }
    assert!(logs.contains(&"lives:2".to_string()), "{logs:?}");
}

#[test]
fn enemy_patrols_between_bounds() {
    let (mut rt, _) = coin_dash();
    let mut input = InputState::new();
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    for _ in 0..600 {
        rt.tick(&mut input, 1.0 / 60.0, &mut Vec::new()).unwrap();
        let x = x_of(&rt, "Enemy");
        min = min.min(x);
        max = max.max(x);
    }
    assert!(min < 7.5, "never went left: {min}");
    assert!(max > 8.5, "never went right: {max}");
}
