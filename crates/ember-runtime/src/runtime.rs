use std::collections::HashMap;
use std::fmt;

use ember_lang::compile::compile;
use ember_scene::{EntityId, Scene, ScriptValue};
use ember_vm::{Host, RuntimeError, Value, Vm};

use crate::input::InputState;

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
        let mut runtime = Self { scene, instances };
        let idle = InputState::new();
        for i in 0..runtime.instances.len() {
            let entity = runtime.instances[i].entity;
            {
                let mut host = CtxHost::new(&mut runtime.scene, entity, &idle);
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
                let mut host = CtxHost::new(&mut runtime.scene, entity, &idle);
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
        input: &InputState,
        dt: f32,
        logs: &mut Vec<String>,
    ) -> Result<(), RuntimeError> {
        for i in 0..self.instances.len() {
            if !self.instances[i].vm.has_function("on_update") {
                continue;
            }
            let entity = self.instances[i].entity;
            let mut host = CtxHost::new(&mut self.scene, entity, input);
            self.instances[i]
                .vm
                .call(&mut host, "on_update", vec![Value::F32(dt)])?;
            logs.extend(host.take_logs());
        }
        Ok(())
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
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
    logs: Vec<String>,
}

impl<'a> CtxHost<'a> {
    fn new(scene: &'a mut Scene, entity: EntityId, input: &'a InputState) -> Self {
        Self {
            scene,
            entity,
            input,
            logs: Vec::new(),
        }
    }

    fn take_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logs)
    }
}

impl<'a> Host for CtxHost<'a> {
    fn input_down(&mut self, key: i32) -> bool {
        self.input.is_down(key)
    }

    fn transform_move(&mut self, dx: f32, dy: f32) {
        if let Some(entity) = self.scene.entities.iter_mut().find(|e| e.id == self.entity) {
            entity.transform.position.x += dx;
            entity.transform.position.y += dy;
        }
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
        rt.tick(&input, 0.5, &mut logs).unwrap();
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
        rt.tick(&InputState::new(), 0.5, &mut logs).unwrap();
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
        rt.tick(&InputState::new(), 0.016, &mut logs).unwrap();
        rt.tick(&InputState::new(), 0.016, &mut logs).unwrap();
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
        rt.tick(&InputState::new(), 0.016, &mut logs).unwrap();
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
        let err = rt.tick(&InputState::new(), 0.016, &mut logs).unwrap_err();
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
        rt.tick(&InputState::new(), 1.0, &mut logs).unwrap();
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
        rt.tick(&InputState::new(), 0.5, &mut Vec::new()).unwrap();
        assert_eq!(pos(&rt, id), (5.0, 0.0));
    }

    #[test]
    fn compile_error_blocks_load() {
        let (scene, id) = scene_with_player();
        let err = Runtime::load(scene, vec![def(id, "var x = \n")], &mut Vec::new()).unwrap_err();
        assert!(err.to_string().contains("player.em:1:"));
    }
}
