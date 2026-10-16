use std::fmt;
use std::path::Path;

use ember_core::Vec2;
use serde::{Deserialize, Serialize};

pub const SCENE_FORMAT_VERSION: u32 = 3;

#[derive(Debug, Clone)]
pub struct SceneError(String);

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SceneError {}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform2D {
    pub position: Vec2,
    pub rotation_degrees: f32,
    pub scale: Vec2,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            position: Vec2::zero(),
            rotation_degrees: 0.0,
            scale: Vec2::new(1.0, 1.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpriteRenderer {
    pub texture: String,
    pub tint: [f32; 4],
    pub sorting_order: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera2D {
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collider2D {
    pub hx: f32,
    pub hy: f32,
    #[serde(default)]
    pub sensor: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimClip {
    pub name: String,
    pub frames: Vec<String>,
    pub fps: f32,
    #[serde(default)]
    pub looped: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpriteAnim {
    pub clips: Vec<AnimClip>,
    pub current: String,
    #[serde(default)]
    pub t: f32,
}

impl SpriteAnim {
    pub fn frame_at(&self, t: f32) -> Option<&str> {
        let clip = self.clips.iter().find(|c| c.name == self.current)?;
        if clip.frames.is_empty() {
            return None;
        }
        let idx = (t.max(0.0) * clip.fps.max(0.0)) as usize;
        let i = if clip.looped {
            idx % clip.frames.len()
        } else {
            idx.min(clip.frames.len() - 1)
        };
        clip.frames.get(i).map(String::as_str)
    }

    pub fn has_clip(&self, name: &str) -> bool {
        self.clips.iter().any(|c| c.name == name)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyKind {
    Static,
    Dynamic,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RigidBody2D {
    pub kind: BodyKind,
    pub velocity: Vec2,
    pub gravity_scale: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScriptValue {
    I32(i32),
    F32(f32),
    Bool(bool),
    Str(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub transform: Transform2D,
    pub sprite: Option<SpriteRenderer>,
    pub camera: Option<Camera2D>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub script_values: std::collections::HashMap<String, ScriptValue>,
    #[serde(default)]
    pub collider: Option<Collider2D>,
    #[serde(default)]
    pub rigidbody: Option<RigidBody2D>,
    #[serde(default)]
    pub anim: Option<SpriteAnim>,
}

impl Entity {
    pub fn frame_texture(&self) -> Option<&str> {
        let sprite = self.sprite.as_ref()?;
        if let Some(anim) = &self.anim
            && let Some(frame) = anim.frame_at(anim.t)
        {
            return Some(frame);
        }
        Some(sprite.texture.as_str())
    }

    pub fn texture_paths(&self) -> Vec<&str> {
        let mut out = Vec::new();
        if let Some(sprite) = &self.sprite {
            out.push(sprite.texture.as_str());
        }
        if let Some(anim) = &self.anim {
            for clip in &anim.clips {
                for frame in &clip.frames {
                    out.push(frame.as_str());
                }
            }
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub format_version: u32,
    pub name: String,
    pub entities: Vec<Entity>,
}

impl Scene {
    pub fn new(name: &str) -> Self {
        Self {
            format_version: SCENE_FORMAT_VERSION,
            name: name.to_string(),
            entities: Vec::new(),
        }
    }

    pub fn spawn(&mut self, name: &str) -> EntityId {
        let next = self.entities.iter().map(|e| e.id.0).max().unwrap_or(0) + 1;
        let id = EntityId(next);
        self.entities.push(Entity {
            id,
            name: name.to_string(),
            transform: Transform2D::default(),
            sprite: None,
            camera: None,
            script: None,
            script_values: std::collections::HashMap::new(),
            collider: None,
            rigidbody: None,
            anim: None,
        });
        id
    }

    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.entities.iter().find(|e| e.id == id)
    }

    pub fn save_to_string(&self) -> Result<String, SceneError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new())
            .map_err(|e| SceneError(format!("serialize scene '{}': {e}", self.name)))
    }

    pub fn load_from_str(src: &str, source: &str) -> Result<Self, SceneError> {
        let scene: Scene =
            ron::from_str(src).map_err(|e| SceneError(format!("parse '{source}': {e}")))?;
        if !(1..=SCENE_FORMAT_VERSION).contains(&scene.format_version) {
            return Err(SceneError(format!(
                "parse '{source}': unsupported format version {}, expected {SCENE_FORMAT_VERSION}",
                scene.format_version
            )));
        }
        let mut seen = std::collections::HashSet::new();
        for entity in &scene.entities {
            if !seen.insert(entity.id) {
                return Err(SceneError(format!(
                    "parse '{source}': duplicate entity id {}",
                    entity.id.0
                )));
            }
        }
        Ok(scene)
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), SceneError> {
        let text = self.save_to_string()?;
        std::fs::write(path, text)
            .map_err(|e| SceneError(format!("write '{}': {e}", path.display())))
    }

    pub fn load_from_file(path: &Path) -> Result<Self, SceneError> {
        let src = std::fs::read_to_string(path)
            .map_err(|e| SceneError(format!("read '{}': {e}", path.display())))?;
        Self::load_from_str(&src, &path.display().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_scene() -> Scene {
        let mut scene = Scene::new("demo");
        let player = scene.spawn("Player");
        let cam = scene.spawn("MainCamera");
        let p = scene.entities.iter_mut().find(|e| e.id == player).unwrap();
        p.transform.position = Vec2::new(10.0, -4.0);
        p.transform.rotation_degrees = 90.0;
        p.sprite = Some(SpriteRenderer {
            texture: "assets/player.png".to_string(),
            tint: [1.0, 1.0, 1.0, 1.0],
            sorting_order: 3,
        });
        let c = scene.entities.iter_mut().find(|e| e.id == cam).unwrap();
        c.camera = Some(Camera2D { height: 12.0 });
        scene
    }

    #[test]
    fn round_trip_preserves_ids_and_data() {
        let scene = sample_scene();
        let ids: Vec<u64> = scene.entities.iter().map(|e| e.id.0).collect();
        let loaded = Scene::load_from_str(&scene.save_to_string().unwrap(), "test").unwrap();
        assert_eq!(loaded, scene);
        assert_eq!(
            loaded.entities.iter().map(|e| e.id.0).collect::<Vec<_>>(),
            ids
        );
    }

    #[test]
    fn script_values_round_trip() {
        let mut scene = sample_scene();
        let first = scene.entities[0].id;
        let entity = scene.entities.iter_mut().find(|e| e.id == first).unwrap();
        entity.script = Some("scripts/player.em".to_string());
        entity
            .script_values
            .insert("speed".to_string(), ScriptValue::F32(6.0));
        entity
            .script_values
            .insert("lives".to_string(), ScriptValue::I32(3));
        let loaded = Scene::load_from_str(&scene.save_to_string().unwrap(), "test").unwrap();
        assert_eq!(loaded, scene);
    }

    #[test]
    fn old_scene_without_script_fields_loads() {
        let src = "(format_version: 1, name: \"old\", entities: [(
            id: (1), name: \"E\", transform: (position: (x: 0.0, y: 0.0),
            rotation_degrees: 0.0, scale: (x: 1.0, y: 1.0)),
            sprite: None, camera: None)])";
        let loaded = Scene::load_from_str(src, "old.ron").unwrap();
        assert!(loaded.entities[0].script.is_none());
        assert!(loaded.entities[0].script_values.is_empty());
        assert!(loaded.entities[0].collider.is_none());
        assert!(loaded.entities[0].rigidbody.is_none());
    }

    #[test]
    fn physics_components_round_trip() {
        let mut scene = Scene::new("phys");
        let id = scene.spawn("Box");
        let entity = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
        entity.collider = Some(Collider2D {
            hx: 0.5,
            hy: 0.5,
            sensor: false,
        });
        entity.rigidbody = Some(RigidBody2D {
            kind: BodyKind::Dynamic,
            velocity: Vec2::new(1.0, 0.0),
            gravity_scale: 0.0,
        });
        let loaded = Scene::load_from_str(&scene.save_to_string().unwrap(), "test").unwrap();
        assert_eq!(loaded, scene);
        let body = loaded.entities[0].rigidbody.as_ref().unwrap();
        assert_eq!(body.kind, BodyKind::Dynamic);
        assert_eq!(body.velocity, Vec2::new(1.0, 0.0));
    }

    #[test]
    fn spawn_assigns_unique_increasing_ids() {
        let mut scene = Scene::new("ids");
        let a = scene.spawn("a");
        let b = scene.spawn("b");
        assert_ne!(a, b);
        assert!(b.0 > a.0);
        assert!(scene.get(a).is_some());
    }

    #[test]
    fn file_round_trip_keeps_scene() {
        let scene = sample_scene();
        let path = std::env::temp_dir().join("ember-scene-roundtrip.ron");
        scene.save_to_file(&path).unwrap();
        let loaded = Scene::load_from_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded, scene);
    }

    #[test]
    fn rejects_unknown_version() {
        let mut scene = sample_scene();
        scene.format_version = 999;
        let err = Scene::load_from_str(&scene.save_to_string().unwrap(), "v.ron").unwrap_err();
        assert!(err.to_string().contains("999"));
        assert!(err.to_string().contains("v.ron"));
    }

    #[test]
    fn v1_scene_without_sensor_loads() {
        let src = "(format_version: 1, name: \"old\", entities: [(
            id: (1), name: \"E\", transform: (position: (x: 0.0, y: 0.0),
            rotation_degrees: 0.0, scale: (x: 1.0, y: 1.0)),
            sprite: None, camera: None, script: None, script_values: {},
            collider: Some((hx: 0.5, hy: 0.5)), rigidbody: None)])";
        let loaded = Scene::load_from_str(src, "old.ron").unwrap();
        let collider = loaded.entities[0].collider.as_ref().unwrap();
        assert!(!collider.sensor);
    }

    #[test]
    fn sensor_round_trip() {
        let mut scene = Scene::new("sens");
        let id = scene.spawn("Coin");
        scene
            .entities
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .collider = Some(Collider2D {
            hx: 0.5,
            hy: 0.5,
            sensor: true,
        });
        let loaded = Scene::load_from_str(&scene.save_to_string().unwrap(), "test").unwrap();
        assert_eq!(loaded, scene);
        assert!(loaded.entities[0].collider.as_ref().unwrap().sensor);
    }

    #[test]
    fn v2_scene_without_anim_loads() {
        let src = "(format_version: 2, name: \"old\", entities: [(
            id: (1), name: \"E\", transform: (position: (x: 0.0, y: 0.0),
            rotation_degrees: 0.0, scale: (x: 1.0, y: 1.0)),
            sprite: None, camera: None, script: None, script_values: {},
            collider: None, rigidbody: None)])";
        let loaded = Scene::load_from_str(src, "old.ron").unwrap();
        assert!(loaded.entities[0].anim.is_none());
    }

    #[test]
    fn anim_round_trip() {
        let mut scene = Scene::new("anim");
        let id = scene.spawn("Hero");
        scene.entities.iter_mut().find(|e| e.id == id).unwrap().anim = Some(SpriteAnim {
            clips: vec![
                AnimClip {
                    name: "idle".to_string(),
                    frames: vec!["i0.png".to_string()],
                    fps: 1.0,
                    looped: false,
                },
                AnimClip {
                    name: "walk".to_string(),
                    frames: vec!["w0.png".to_string(), "w1.png".to_string()],
                    fps: 8.0,
                    looped: true,
                },
            ],
            current: "walk".to_string(),
            t: 0.0,
        });
        let loaded = Scene::load_from_str(&scene.save_to_string().unwrap(), "test").unwrap();
        assert_eq!(loaded, scene);
    }

    #[test]
    fn frame_picks_by_time() {
        let anim = SpriteAnim {
            clips: vec![AnimClip {
                name: "walk".to_string(),
                frames: vec!["w0.png".to_string(), "w1.png".to_string()],
                fps: 2.0,
                looped: true,
            }],
            current: "walk".to_string(),
            t: 0.0,
        };
        assert_eq!(anim.frame_at(0.0), Some("w0.png"));
        assert_eq!(anim.frame_at(0.5), Some("w1.png"));
        assert_eq!(anim.frame_at(1.0), Some("w0.png"));
        let once = SpriteAnim {
            clips: vec![AnimClip {
                name: "hit".to_string(),
                frames: vec!["h0.png".to_string(), "h1.png".to_string()],
                fps: 2.0,
                looped: false,
            }],
            current: "hit".to_string(),
            t: 0.0,
        };
        assert_eq!(once.frame_at(99.0), Some("h1.png"));
        let missing = SpriteAnim {
            clips: vec![],
            current: "nope".to_string(),
            t: 0.0,
        };
        assert_eq!(missing.frame_at(0.0), None);
    }

    #[test]
    fn rejects_duplicate_id() {
        let mut scene = sample_scene();
        let dup = scene.entities[0].clone();
        scene.entities.push(dup);
        let err = Scene::load_from_str(&scene.save_to_string().unwrap(), "dup.ron").unwrap_err();
        assert!(err.to_string().contains("duplicate entity id"));
    }

    #[test]
    fn malformed_input_mentions_source() {
        let err = Scene::load_from_str("(((", "broken.ron").unwrap_err();
        assert!(err.to_string().contains("broken.ron"));
    }

    #[test]
    fn missing_file_mentions_path() {
        let err = Scene::load_from_file(Path::new("nope/missing.ron")).unwrap_err();
        assert!(err.to_string().contains("nope/missing.ron"));
    }
}
