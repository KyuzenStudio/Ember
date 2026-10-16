use std::collections::{HashMap, HashSet};
use std::fmt;

use ember_lang::compile::compile;
use ember_scene::{EntityId, Scene, ScriptValue};
use ember_vm::{Host, RuntimeError, Value, Vm};

use crate::input::InputState;
use crate::physics::PhysicsWorld;

#[derive(Clone, Debug)]
pub struct ScriptDef {
    pub entity: EntityId,
    pub file: String,
    pub source: String,
    pub values: HashMap<String, ScriptValue>,
}

#[derive(Debug)]
pub struct ChunkDef {
    pub entity: EntityId,
    pub file: String,
    pub chunk: ember_vm::Chunk,
    pub values: HashMap<String, ScriptValue>,
}

#[derive(Debug)]
pub enum SpawnError {
    Compile(Vec<ember_lang::Error>),
    Runtime(RuntimeError),
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpawnError::Compile(errors) => {
                for e in errors {
                    writeln!(f, "{e}")?;
                }
                Ok(())
            }
            SpawnError::Runtime(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SpawnError {}

#[derive(Debug)]
struct Instance {
    entity: EntityId,
    vm: Vm,
    values: HashMap<String, ScriptValue>,
}

#[derive(Debug)]
pub struct Runtime {
    scene: Scene,
    instances: Vec<Instance>,
    physics: Option<PhysicsWorld>,
    pending_audio: Vec<AudioCmd>,
    pending_anim: Vec<(EntityId, String)>,
    pending_title: Option<String>,
    restart_requested: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioCmd {
    Play(String),
    Loop(String),
    Stop,
}

impl Runtime {
    pub fn load(
        scene: Scene,
        scripts: Vec<ScriptDef>,
        logs: &mut Vec<String>,
    ) -> Result<Self, SpawnError> {
        let mut compile_errors = Vec::new();
        let mut built = Vec::new();
        for def in &scripts {
            match compile(&def.file, &def.source) {
                Ok(chunk) => match Vm::new(chunk) {
                    Ok(vm) => built.push((def.entity, vm, def.values.clone())),
                    Err(e) => compile_errors.push(lang_wrap(&def.file, e.to_string())),
                },
                Err(mut errors) => compile_errors.append(&mut errors),
            }
        }
        if !compile_errors.is_empty() {
            return Err(SpawnError::Compile(compile_errors));
        }
        Self::start(
            scene,
            built
                .into_iter()
                .map(|(entity, vm, values)| Instance { entity, vm, values })
                .collect(),
            logs,
        )
    }

    pub fn load_prebuilt(
        scene: Scene,
        chunks: Vec<ChunkDef>,
        logs: &mut Vec<String>,
    ) -> Result<Self, SpawnError> {
        let mut built = Vec::new();
        for def in chunks {
            match Vm::new(def.chunk) {
                Ok(vm) => built.push((def.entity, vm, def.values)),
                Err(e) => {
                    return Err(SpawnError::Compile(vec![lang_wrap(
                        &def.file,
                        e.to_string(),
                    )]));
                }
            }
        }
        Self::start(
            scene,
            built
                .into_iter()
                .map(|(entity, vm, values)| Instance { entity, vm, values })
                .collect(),
            logs,
        )
    }

    fn start(
        scene: Scene,
        instances: Vec<Instance>,
        logs: &mut Vec<String>,
    ) -> Result<Self, SpawnError> {
        let physics =
            if scene.entities.iter().any(|e| e.rigidbody.is_some()) {
                Some(PhysicsWorld::build(&scene.entities).map_err(|message| {
                    SpawnError::Compile(vec![lang_wrap(&scene.name, message)])
                })?)
            } else {
                None
            };
        let mut runtime = Self {
            scene,
            instances,
            physics,
            pending_audio: Vec::new(),
            pending_anim: Vec::new(),
            pending_title: None,
            restart_requested: false,
        };
        let idle = InputState::new();
        let no_edge: HashSet<i32> = HashSet::new();
        for i in 0..runtime.instances.len() {
            let entity = runtime.instances[i].entity;
            {
                let mut host = CtxHost::new(&mut runtime.scene, entity, &idle, &no_edge);
                if runtime.instances[i].vm.has_function("$init") {
                    runtime.instances[i]
                        .vm
                        .call(&mut host, "$init", vec![])
                        .map_err(SpawnError::Runtime)?;
                }
                logs.extend(host.take_logs());
            }
            {
                let inst = &mut runtime.instances[i];
                for (name, script_value) in &inst.values {
                    if let Some(idx) = inst.vm.global_index(name) {
                        inst.vm
                            .set_global(idx, script_value_to_value(script_value))
                            .map_err(SpawnError::Runtime)?;
                    }
                }
            }
            {
                let mut host = CtxHost::new(&mut runtime.scene, entity, &idle, &no_edge);
                if runtime.instances[i].vm.has_function("on_create") {
                    runtime.instances[i]
                        .vm
                        .call(&mut host, "on_create", vec![])
                        .map_err(SpawnError::Runtime)?;
                }
                logs.extend(host.take_logs());
            }
        }
        Ok(runtime)
    }

    pub fn tick(
        &mut self,
        input: &mut InputState,
        dt: f32,
        logs: &mut Vec<String>,
    ) -> Result<(), RuntimeError> {
        let pressed = input.take_pressed();
        if let Some(physics) = &mut self.physics {
            physics.tick(&mut self.scene.entities, dt);
        }
        let contacts = match &mut self.physics {
            Some(physics) => physics.take_contacts(),
            None => Vec::new(),
        };
        let mut destroyed = Vec::new();
        for i in 0..self.instances.len() {
            if !self.instances[i].vm.has_function("on_update") {
                continue;
            }
            let entity = self.instances[i].entity;
            let mut host = CtxHost::new(&mut self.scene, entity, input, &pressed);
            self.instances[i]
                .vm
                .call(&mut host, "on_update", vec![Value::F32(dt)])?;
            logs.extend(host.take_logs());
            self.pending_audio.extend(host.take_audio());
            self.pending_anim.extend(host.take_anim());
            if let Some(title) = host.take_title() {
                self.pending_title = Some(title);
            }
            if host.take_restart() {
                self.restart_requested = true;
            }
            destroyed.extend(host.take_destroyed());
        }
        if !destroyed.is_empty() {
            self.apply_destroyed(&destroyed);
        }
        let mut late_destroyed = Vec::new();
        for contact in contacts {
            self.dispatch_contact(contact, logs, &mut late_destroyed)?;
        }
        if !late_destroyed.is_empty() {
            self.apply_destroyed(&late_destroyed);
        }
        Ok(())
    }

    fn apply_destroyed(&mut self, destroyed: &[EntityId]) {
        self.scene.entities.retain(|e| !destroyed.contains(&e.id));
        self.instances.retain(|i| !destroyed.contains(&i.entity));
        if self.physics.is_some()
            && let Ok(world) = PhysicsWorld::build(&self.scene.entities)
        {
            self.physics = Some(world);
        }
    }

    fn dispatch_contact(
        &mut self,
        contact: crate::physics::Contact,
        logs: &mut Vec<String>,
        destroyed: &mut Vec<EntityId>,
    ) -> Result<(), RuntimeError> {
        let lifecycle = if contact.begun {
            "on_collision_begin"
        } else {
            "on_collision_end"
        };
        let idle = InputState::new();
        let empty = HashSet::new();
        for (me, other) in [(contact.a, contact.b), (contact.b, contact.a)] {
            let Some(other_name) = self
                .scene
                .entities
                .iter()
                .find(|e| e.id == other)
                .map(|e| e.name.clone())
            else {
                continue;
            };
            if !self.scene.entities.iter().any(|e| e.id == me) {
                continue;
            }
            let idx = match self.instances.iter().position(|i| i.entity == me) {
                Some(idx) => idx,
                None => continue,
            };
            if !self.instances[idx].vm.has_function(lifecycle) {
                continue;
            }
            let mut host = CtxHost::new(&mut self.scene, me, &idle, &empty);
            self.instances[idx]
                .vm
                .call(&mut host, lifecycle, vec![Value::Str(other_name)])?;
            logs.extend(host.take_logs());
            self.pending_audio.extend(host.take_audio());
            self.pending_anim.extend(host.take_anim());
            if let Some(title) = host.take_title() {
                self.pending_title = Some(title);
            }
            if host.take_restart() {
                self.restart_requested = true;
            }
            destroyed.extend(host.take_destroyed());
        }
        Ok(())
    }

    pub fn take_audio(&mut self) -> Vec<AudioCmd> {
        std::mem::take(&mut self.pending_audio)
    }

    pub fn take_anim(&mut self) -> Vec<(EntityId, String)> {
        std::mem::take(&mut self.pending_anim)
    }

    pub fn take_title(&mut self) -> Option<String> {
        self.pending_title.take()
    }

    pub fn take_restart_requested(&mut self) -> bool {
        std::mem::replace(&mut self.restart_requested, false)
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn physics_active(&self) -> bool {
        self.physics.as_ref().is_some_and(PhysicsWorld::has_bodies)
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    pub fn rebuild_physics(&mut self) -> Result<(), String> {
        let physics = if self.scene.entities.iter().any(|e| e.rigidbody.is_some()) {
            Some(PhysicsWorld::build(&self.scene.entities)?)
        } else {
            None
        };
        self.physics = physics;
        Ok(())
    }

    pub fn set_global(&mut self, entity: EntityId, name: &str, value: Value) -> Result<(), String> {
        let inst = self
            .instances
            .iter_mut()
            .find(|i| i.entity == entity)
            .ok_or_else(|| "entity has no running script".to_string())?;
        let idx = inst
            .vm
            .global_index(name)
            .ok_or_else(|| format!("no global '{name}'"))?;
        inst.vm.set_global(idx, value).map_err(|e| e.to_string())
    }

    pub fn global(&self, entity: EntityId, name: &str) -> Option<Value> {
        self.instances
            .iter()
            .find(|i| i.entity == entity)
            .and_then(|i| {
                i.vm.global_index(name)
                    .and_then(|idx| i.vm.globals().get(idx).cloned())
            })
    }
}

fn script_value_to_value(value: &ScriptValue) -> Value {
    match value {
        ScriptValue::I32(v) => Value::I32(*v),
        ScriptValue::F32(v) => Value::F32(*v),
        ScriptValue::Bool(v) => Value::Bool(*v),
        ScriptValue::Str(v) => Value::Str(v.clone()),
    }
}

fn lang_wrap(file: &str, message: String) -> ember_lang::Error {
    ember_lang::Error {
        file: file.to_string(),
        line: 1,
        col: 1,
        message,
        span: ember_lang::Span::empty(0),
    }
}

struct CtxHost<'a> {
    scene: &'a mut Scene,
    entity: EntityId,
    input: &'a InputState,
    pressed: &'a HashSet<i32>,
    logs: Vec<String>,
    audio: Vec<AudioCmd>,
    anim: Vec<(EntityId, String)>,
    title: Option<String>,
    restart: bool,
    destroyed: Vec<EntityId>,
}

impl<'a> CtxHost<'a> {
    fn new(
        scene: &'a mut Scene,
        entity: EntityId,
        input: &'a InputState,
        pressed: &'a HashSet<i32>,
    ) -> Self {
        Self {
            scene,
            entity,
            input,
            pressed,
            logs: Vec::new(),
            audio: Vec::new(),
            anim: Vec::new(),
            title: None,
            restart: false,
            destroyed: Vec::new(),
        }
    }

    fn take_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logs)
    }

    fn take_audio(&mut self) -> Vec<AudioCmd> {
        std::mem::take(&mut self.audio)
    }

    fn take_anim(&mut self) -> Vec<(EntityId, String)> {
        std::mem::take(&mut self.anim)
    }

    fn take_title(&mut self) -> Option<String> {
        self.title.take()
    }

    fn take_restart(&mut self) -> bool {
        std::mem::replace(&mut self.restart, false)
    }

    fn take_destroyed(&mut self) -> Vec<EntityId> {
        std::mem::take(&mut self.destroyed)
    }

    fn own_pos(&self) -> (f32, f32) {
        self.scene
            .entities
            .iter()
            .find(|e| e.id == self.entity)
            .map(|e| (e.transform.position.x, e.transform.position.y))
            .unwrap_or((0.0, 0.0))
    }

    fn named_pos(&self, name: &str) -> Option<(f32, f32)> {
        self.scene
            .entities
            .iter()
            .find(|e| e.name == name)
            .map(|e| (e.transform.position.x, e.transform.position.y))
    }
}

impl<'a> Host for CtxHost<'a> {
    fn input_down(&mut self, key: i32) -> bool {
        self.input.is_down(key)
    }

    fn input_pressed(&mut self, key: i32) -> bool {
        self.pressed.contains(&key)
    }

    fn transform_move(&mut self, dx: f32, dy: f32) {
        if let Some(entity) = self.scene.entities.iter_mut().find(|e| e.id == self.entity) {
            entity.transform.position.x += dx;
            entity.transform.position.y += dy;
        }
    }

    fn transform_x(&mut self) -> f32 {
        self.own_pos().0
    }

    fn transform_y(&mut self) -> f32 {
        self.own_pos().1
    }

    fn transform_set(&mut self, x: f32, y: f32) {
        if let Some(entity) = self.scene.entities.iter_mut().find(|e| e.id == self.entity) {
            entity.transform.position.x = x;
            entity.transform.position.y = y;
        }
    }

    fn entity_x(&mut self, name: &str) -> Result<f32, String> {
        self.named_pos(name)
            .map(|p| p.0)
            .ok_or_else(|| format!("entity '{name}' not found"))
    }

    fn entity_y(&mut self, name: &str) -> Result<f32, String> {
        self.named_pos(name)
            .map(|p| p.1)
            .ok_or_else(|| format!("entity '{name}' not found"))
    }

    fn entity_destroy(&mut self, name: &str) -> Result<(), String> {
        let Some(id) = self
            .scene
            .entities
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.id)
        else {
            return Err(format!("entity '{name}' not found"));
        };
        if !self.destroyed.contains(&id) {
            self.destroyed.push(id);
        }
        Ok(())
    }

    fn audio_play(&mut self, name: &str) {
        self.audio.push(AudioCmd::Play(name.to_string()));
    }

    fn audio_loop(&mut self, name: &str) {
        self.audio.push(AudioCmd::Loop(name.to_string()));
    }

    fn audio_stop(&mut self) {
        self.audio.push(AudioCmd::Stop);
    }

    fn anim_play(&mut self, clip: &str) {
        self.anim.push((self.entity, clip.to_string()));
    }

    fn game_restart(&mut self) {
        self.restart = true;
    }

    fn game_set_title(&mut self, title: &str) {
        self.title = Some(title.to_string());
    }

    fn log(&mut self, msg: &str) {
        self.logs.push(msg.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ember_scene::{Camera2D, SpriteRenderer};

    const PLAYER: &str = "var speed: f32 = 220.0\nfn on_create() {\nlog(\"hi\")\n}\nfn on_update(dt: f32) {\nif input.down(Key.Right) {\ntransform.move(speed * dt, 0.0)\n}\n}\n";

    fn scene_with_player() -> (Scene, EntityId) {
        let mut scene = Scene::new("game");
        let id = scene.spawn("Player");
        let entity = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
        entity.sprite = Some(SpriteRenderer {
            texture: "assets/player.png".to_string(),
            tint: [1.0, 1.0, 1.0, 1.0],
            sorting_order: 0,
        });
        let cam = scene.spawn("Cam");
        scene
            .entities
            .iter_mut()
            .find(|e| e.id == cam)
            .unwrap()
            .camera = Some(Camera2D { height: 12.0 });
        (scene, id)
    }

    fn def(entity: EntityId, source: &str) -> ScriptDef {
        ScriptDef {
            entity,
            file: "player.em".to_string(),
            source: source.to_string(),
            values: HashMap::new(),
        }
    }

    fn pos(rt: &Runtime, id: EntityId) -> (f32, f32) {
        let e = rt.scene().entities.iter().find(|e| e.id == id).unwrap();
        (e.transform.position.x, e.transform.position.y)
    }

    #[test]
    fn script_moves_entity_from_input() {
        let (scene, id) = scene_with_player();
        let mut spawn_logs = Vec::new();
        let mut rt = Runtime::load(scene, vec![def(id, PLAYER)], &mut spawn_logs).unwrap();
        assert_eq!(spawn_logs, vec!["hi".to_string()]);
        let mut input = InputState::new();
        input.press(3);
        let mut logs = Vec::new();
        rt.tick(&mut input, 0.5, &mut logs).unwrap();
        assert_eq!(pos(&rt, id), (110.0, 0.0));
        assert!(logs.is_empty());
    }

    #[test]
    fn released_key_moves_nothing() {
        let (scene, id) = scene_with_player();
        let mut spawn_logs = Vec::new();
        let mut rt = Runtime::load(scene, vec![def(id, PLAYER)], &mut spawn_logs).unwrap();
        assert_eq!(spawn_logs, vec!["hi".to_string()]);
        let mut logs = Vec::new();
        rt.tick(&mut InputState::new(), 0.5, &mut logs).unwrap();
        assert_eq!(pos(&rt, id), (0.0, 0.0));
        assert!(logs.is_empty());
    }

    #[test]
    fn on_create_runs_once() {
        let (scene, id) = scene_with_player();
        let mut spawn_logs = Vec::new();
        let mut rt = Runtime::load(scene, vec![def(id, PLAYER)], &mut spawn_logs).unwrap();
        assert_eq!(spawn_logs, vec!["hi".to_string()]);
        let mut logs = Vec::new();
        rt.tick(&mut InputState::new(), 0.016, &mut logs).unwrap();
        rt.tick(&mut InputState::new(), 0.016, &mut logs).unwrap();
        assert!(logs.is_empty());
    }

    #[test]
    fn missing_on_update_is_ok() {
        let (scene, id) = scene_with_player();
        let mut rt = Runtime::load(
            scene,
            vec![def(id, "fn on_create() {\n}\n")],
            &mut Vec::new(),
        )
        .unwrap();
        let mut logs = Vec::new();
        rt.tick(&mut InputState::new(), 0.016, &mut logs).unwrap();
    }

    #[test]
    fn script_error_aborts_tick_with_location() {
        let (scene, id) = scene_with_player();
        let mut rt = Runtime::load(
            scene,
            vec![def(id, "fn on_update(dt: f32) {\nvar x: i32 = 1 / 0\n}\n")],
            &mut Vec::new(),
        )
        .unwrap();
        let mut logs = Vec::new();
        let err = rt
            .tick(&mut InputState::new(), 0.016, &mut logs)
            .unwrap_err();
        assert!(err.to_string().contains("player.em:2:"));
        assert!(err.to_string().contains("division by zero"));
    }

    #[test]
    fn two_entities_are_independent() {
        let mut scene = Scene::new("game");
        let a = scene.spawn("A");
        let b = scene.spawn("B");
        let src = "fn on_update(dt: f32) {\ntransform.move(1.0, 0.0)\n}\n";
        let mut rt = Runtime::load(scene, vec![def(a, src), def(b, src)], &mut Vec::new()).unwrap();
        let mut logs = Vec::new();
        rt.tick(&mut InputState::new(), 1.0, &mut logs).unwrap();
        assert_eq!(pos(&rt, a), (1.0, 0.0));
        assert_eq!(pos(&rt, b), (1.0, 0.0));
    }

    #[test]
    fn values_override_initializers() {
        let (scene, id) = scene_with_player();
        let mut script = def(
            id,
            "var speed: f32 = 1.0\nfn on_update(dt: f32) {\ntransform.move(speed * dt, 0.0)\n}\n",
        );
        script
            .values
            .insert("speed".to_string(), ScriptValue::F32(10.0));
        script
            .values
            .insert("gone".to_string(), ScriptValue::I32(1));
        let mut rt = Runtime::load(scene, vec![script], &mut Vec::new()).unwrap();
        assert_eq!(rt.global(id, "speed"), Some(Value::F32(10.0)));
        rt.tick(&mut InputState::new(), 0.5, &mut Vec::new())
            .unwrap();
        assert_eq!(pos(&rt, id), (5.0, 0.0));
    }

    #[test]
    fn compile_error_blocks_load() {
        let (scene, id) = scene_with_player();
        let err = Runtime::load(scene, vec![def(id, "var x = \n")], &mut Vec::new()).unwrap_err();
        assert!(err.to_string().contains("player.em:1:"));
    }

    #[test]
    fn physics_runs_inside_tick() {
        use ember_scene::{BodyKind, Collider2D, RigidBody2D};
        let mut scene = Scene::new("phys");
        let ground = scene.spawn("Ground");
        scene
            .entities
            .iter_mut()
            .find(|e| e.id == ground)
            .unwrap()
            .transform
            .position = ember_core::Vec2::new(0.0, -1.0);
        let entity = scene.entities.iter_mut().find(|e| e.id == ground).unwrap();
        entity.rigidbody = Some(RigidBody2D {
            kind: BodyKind::Static,
            velocity: ember_core::Vec2::zero(),
            gravity_scale: 1.0,
        });
        entity.collider = Some(Collider2D {
            hx: 10.0,
            hy: 1.0,
            sensor: false,
        });
        let cube = scene.spawn("Cube");
        let entity = scene.entities.iter_mut().find(|e| e.id == cube).unwrap();
        entity.transform.position = ember_core::Vec2::new(0.0, 5.0);
        entity.rigidbody = Some(RigidBody2D {
            kind: BodyKind::Dynamic,
            velocity: ember_core::Vec2::zero(),
            gravity_scale: 1.0,
        });
        entity.collider = Some(Collider2D {
            hx: 0.5,
            hy: 0.5,
            sensor: false,
        });
        let mut rt = Runtime::load(scene, vec![], &mut Vec::new()).unwrap();
        for _ in 0..180 {
            rt.tick(&mut InputState::new(), 1.0 / 60.0, &mut Vec::new())
                .unwrap();
        }
        let y = rt
            .scene()
            .entities
            .iter()
            .find(|e| e.name == "Cube")
            .unwrap()
            .transform
            .position
            .y;
        assert!((y - 0.5).abs() < 0.05, "rest y = {y}");
    }

    #[test]
    fn bad_collider_blocks_load() {
        use ember_scene::{BodyKind, Collider2D, RigidBody2D};
        let mut scene = Scene::new("bad");
        let id = scene.spawn("Bad");
        let entity = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
        entity.rigidbody = Some(RigidBody2D {
            kind: BodyKind::Dynamic,
            velocity: ember_core::Vec2::zero(),
            gravity_scale: 1.0,
        });
        entity.collider = Some(Collider2D {
            hx: 0.0,
            hy: 1.0,
            sensor: false,
        });
        let err = Runtime::load(scene, vec![], &mut Vec::new()).unwrap_err();
        assert!(err.to_string().contains("must be positive"));
    }

    #[test]
    fn rebuild_picks_up_new_bodies() {
        use ember_scene::{BodyKind, Collider2D, RigidBody2D};
        let mut scene = Scene::new("late");
        let id = scene.spawn("Late");
        let mut rt = Runtime::load(scene, vec![], &mut Vec::new()).unwrap();
        for _ in 0..10 {
            rt.tick(&mut InputState::new(), 1.0 / 60.0, &mut Vec::new())
                .unwrap();
        }
        let entity = rt
            .scene_mut()
            .entities
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap();
        entity.transform.position = ember_core::Vec2::new(0.0, 5.0);
        entity.rigidbody = Some(RigidBody2D {
            kind: BodyKind::Dynamic,
            velocity: ember_core::Vec2::zero(),
            gravity_scale: 1.0,
        });
        entity.collider = Some(Collider2D {
            hx: 0.5,
            hy: 0.5,
            sensor: false,
        });
        rt.rebuild_physics().unwrap();
        for _ in 0..60 {
            rt.tick(&mut InputState::new(), 1.0 / 60.0, &mut Vec::new())
                .unwrap();
        }
        let y = rt
            .scene()
            .entities
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .transform
            .position
            .y;
        assert!(y < 4.0, "body did not fall after rebuild: y = {y}");
    }

    fn scene_with_two(name_a: &str, name_b: &str) -> (Scene, EntityId, EntityId) {
        let mut scene = Scene::new("two");
        let a = scene.spawn(name_a);
        let b = scene.spawn(name_b);
        (scene, a, b)
    }

    #[test]
    fn pressed_fires_once_per_press() {
        let (scene, id) = scene_with_player();
        let src = "fn on_update(dt: f32) {\nif input.pressed(Key.Space) {\ntransform.move(1.0, 0.0)\n}\n}\n";
        let mut rt = Runtime::load(scene, vec![def(id, src)], &mut Vec::new()).unwrap();
        let mut input = InputState::new();
        input.press(4);
        rt.tick(&mut input, 0.5, &mut Vec::new()).unwrap();
        assert_eq!(pos(&rt, id), (1.0, 0.0));
        rt.tick(&mut input, 0.5, &mut Vec::new()).unwrap();
        assert_eq!(pos(&rt, id), (1.0, 0.0));
    }

    #[test]
    fn transform_read_write() {
        let (scene, a, b) = scene_with_two("A", "B");
        let src = "fn on_update(dt: f32) {\ntransform.set(entity.x(\"B\"), entity.y(\"B\"))\n}\n";
        let mut rt = Runtime::load(scene, vec![def(a, src)], &mut Vec::new()).unwrap();
        rt.scene_mut()
            .entities
            .iter_mut()
            .find(|e| e.id == b)
            .unwrap()
            .transform
            .position = ember_core::Vec2::new(3.0, 4.0);
        rt.tick(&mut InputState::new(), 0.5, &mut Vec::new())
            .unwrap();
        assert_eq!(pos(&rt, a), (3.0, 4.0));
    }

    #[test]
    fn unknown_entity_errors_with_name() {
        let (scene, id) = scene_with_player();
        let src = "fn on_update(dt: f32) {\ntransform.set(entity.x(\"Ghost\"), 0.0)\n}\n";
        let mut rt = Runtime::load(scene, vec![def(id, src)], &mut Vec::new()).unwrap();
        let err = rt
            .tick(&mut InputState::new(), 0.5, &mut Vec::new())
            .unwrap_err();
        assert!(err.to_string().contains("Ghost"), "{err}");
    }

    #[test]
    fn str_helpers_log_score() {
        let (scene, id) = scene_with_player();
        let src = "var score: i32 = 3\nfn on_update(dt: f32) {\nlog(str.cat(\"s:\", str.of_i(score)))\n}\n";
        let mut rt = Runtime::load(scene, vec![def(id, src)], &mut Vec::new()).unwrap();
        let mut logs = Vec::new();
        rt.tick(&mut InputState::new(), 0.5, &mut logs).unwrap();
        assert_eq!(logs, vec!["s:3".to_string()]);
    }

    #[test]
    fn destroy_removes_entity_and_instance() {
        let (scene, a, b) = scene_with_two("A", "B");
        let src_a = "var done: i32 = 0\nfn on_update(dt: f32) {\nif done == 0 {\nentity.destroy(\"B\")\ndone = 1\n}\n}\n";
        let src_b = "fn on_update(dt: f32) {\nlog(\"b alive\")\n}\n";
        let mut rt =
            Runtime::load(scene, vec![def(a, src_a), def(b, src_b)], &mut Vec::new()).unwrap();
        let mut logs = Vec::new();
        rt.tick(&mut InputState::new(), 0.5, &mut logs).unwrap();
        assert!(rt.scene().entities.iter().all(|e| e.id != b));
        rt.tick(&mut InputState::new(), 0.5, &mut logs).unwrap();
        assert_eq!(logs.iter().filter(|l| *l == "b alive").count(), 1);
    }

    #[test]
    fn destroy_missing_errors_with_name() {
        let (scene, id) = scene_with_player();
        let src = "fn on_update(dt: f32) {\nentity.destroy(\"Ghost\")\n}\n";
        let mut rt = Runtime::load(scene, vec![def(id, src)], &mut Vec::new()).unwrap();
        let err = rt
            .tick(&mut InputState::new(), 0.5, &mut Vec::new())
            .unwrap_err();
        assert!(err.to_string().contains("Ghost"), "{err}");
    }

    #[test]
    fn audio_anim_title_recorded() {
        let (scene, id) = scene_with_player();
        let src = "fn on_update(dt: f32) {\naudio.play(\"jump.wav\")\naudio.loop(\"music.ogg\")\naudio.stop()\nanim.play(\"walk\")\ngame.set_title(\"hi\")\n}\n";
        let mut rt = Runtime::load(scene, vec![def(id, src)], &mut Vec::new()).unwrap();
        rt.tick(&mut InputState::new(), 0.5, &mut Vec::new())
            .unwrap();
        assert_eq!(
            rt.take_audio(),
            vec![
                AudioCmd::Play("jump.wav".to_string()),
                AudioCmd::Loop("music.ogg".to_string()),
                AudioCmd::Stop,
            ]
        );
        assert_eq!(rt.take_anim(), vec![(id, "walk".to_string())]);
        assert_eq!(rt.take_title(), Some("hi".to_string()));
        assert_eq!(rt.take_title(), None);
    }

    #[test]
    fn restart_flag_then_rebuild_resets_state() {
        let (scene, id) = scene_with_player();
        let pristine = scene.clone();
        let src = "var n: i32 = 0\nfn on_create() {\nlog(\"born\")\n}\nfn on_update(dt: f32) {\nn = n + 1\ntransform.move(1.0, 0.0)\nif n > 1 {\ngame.restart()\n}\n}\n";
        let script = def(id, src);
        let mut spawn_logs = Vec::new();
        let mut rt = Runtime::load(scene, vec![script.clone()], &mut spawn_logs).unwrap();
        assert_eq!(spawn_logs, vec!["born".to_string()]);
        let mut logs = Vec::new();
        rt.tick(&mut InputState::new(), 0.5, &mut logs).unwrap();
        assert!(!rt.take_restart_requested());
        rt.tick(&mut InputState::new(), 0.5, &mut logs).unwrap();
        assert!(rt.take_restart_requested());
        assert!(!rt.take_restart_requested());
        let mut fresh_logs = Vec::new();
        let fresh = Runtime::load(pristine, vec![script], &mut fresh_logs).unwrap();
        assert_eq!(fresh_logs, vec!["born".to_string()]);
        assert_eq!(fresh.global(id, "n"), Some(Value::I32(0)));
        assert_eq!(pos(&fresh, id), (0.0, 0.0));
    }

    #[test]
    fn collision_begin_and_end_dispatched() {
        use ember_scene::{BodyKind, Collider2D, RigidBody2D};
        fn body(scene: &mut Scene, name: &str, y: f32, kind: BodyKind) -> EntityId {
            let id = scene.spawn(name);
            let e = scene.entities.iter_mut().find(|e| e.id == id).unwrap();
            e.transform.position = ember_core::Vec2::new(0.0, y);
            e.rigidbody = Some(RigidBody2D {
                kind,
                velocity: ember_core::Vec2::zero(),
                gravity_scale: 1.0,
            });
            e.collider = Some(Collider2D {
                hx: 0.5,
                hy: 0.5,
                sensor: false,
            });
            id
        }
        let mut scene = Scene::new("hit");
        let ground = body(&mut scene, "Ground", -1.0, BodyKind::Static);
        let cube = body(&mut scene, "Box", 5.0, BodyKind::Dynamic);
        let ground_src = "fn on_collision_begin(other: string) {\nlog(str.cat(\"g began:\", other))\n}\nfn on_collision_end(other: string) {\nlog(str.cat(\"g ended:\", other))\n}\n";
        let cube_src = "fn on_collision_begin(other: string) {\nlog(str.cat(\"c began:\", other))\n}\nfn on_collision_end(other: string) {\nlog(str.cat(\"c ended:\", other))\n}\nvar t: i32 = 0\nfn on_update(dt: f32) {\nt = t + 1\nif t > 90 {\ntransform.set(0.0, 5.0)\nt = 0\n}\n}\n";
        let mut rt = Runtime::load(
            scene,
            vec![def(ground, ground_src), def(cube, cube_src)],
            &mut Vec::new(),
        )
        .unwrap();
        let mut logs = Vec::new();
        for _ in 0..240 {
            rt.tick(&mut InputState::new(), 1.0 / 60.0, &mut logs)
                .unwrap();
        }
        assert!(
            logs.contains(&"c began:Ground".to_string()),
            "missing begin on box: {logs:?}"
        );
        assert!(
            logs.contains(&"g began:Box".to_string()),
            "missing begin on ground: {logs:?}"
        );
        assert!(
            logs.contains(&"c ended:Ground".to_string()),
            "missing end on box: {logs:?}"
        );
    }
}
