use std::collections::HashSet;

use ember_scene::{EntityId, Scene, ScriptValue};
use ember_vm::{Host, Value};

use crate::input::InputState;
use crate::runtime::AudioCmd;

pub(crate) fn script_value_to_value(value: &ScriptValue) -> Value {
    match value {
        ScriptValue::I32(v) => Value::I32(*v),
        ScriptValue::F32(v) => Value::F32(*v),
        ScriptValue::Bool(v) => Value::Bool(*v),
        ScriptValue::Str(v) => Value::Str(v.clone()),
    }
}

pub(crate) fn lang_wrap(file: &str, message: String) -> ember_lang::Error {
    ember_lang::Error {
        file: file.to_string(),
        line: 1,
        col: 1,
        message,
        span: ember_lang::Span::empty(0),
    }
}

pub(crate) struct CtxHost<'a> {
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
    pub(crate) fn new(
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

    pub(crate) fn take_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logs)
    }

    pub(crate) fn take_audio(&mut self) -> Vec<AudioCmd> {
        std::mem::take(&mut self.audio)
    }

    pub(crate) fn take_anim(&mut self) -> Vec<(EntityId, String)> {
        std::mem::take(&mut self.anim)
    }

    pub(crate) fn take_title(&mut self) -> Option<String> {
        self.title.take()
    }

    pub(crate) fn take_restart(&mut self) -> bool {
        std::mem::replace(&mut self.restart, false)
    }

    pub(crate) fn take_destroyed(&mut self) -> Vec<EntityId> {
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
