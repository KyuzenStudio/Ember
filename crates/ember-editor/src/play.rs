use ember_runtime::{AudioCmd, InputState as GameInput, Runtime, ScriptDef};
use ember_scene::{BodyKind, Scene};
use ember_vm::Host;

use super::state::{EditorApp, PlayState};

pub(crate) fn physics_sig(scene: &Scene) -> Vec<(u64, bool, bool, u8, u32, u32)> {
    let mut sig: Vec<(u64, bool, bool, u8, u32, u32)> = scene
        .entities
        .iter()
        .map(|e| {
            let kind = match &e.rigidbody {
                Some(b) => match b.kind {
                    BodyKind::Static => 1,
                    BodyKind::Dynamic => 2,
                },
                None => 0,
            };
            let (hx, hy) = match &e.collider {
                Some(c) => (c.hx.to_bits(), c.hy.to_bits()),
                None => (0, 0),
            };
            (
                e.id.0,
                e.collider.is_some(),
                e.rigidbody.is_some(),
                kind,
                hx,
                hy,
            )
        })
        .collect();
    sig.sort();
    sig
}

pub(crate) struct NoopHost;

impl Host for NoopHost {
    fn input_down(&mut self, _key: i32) -> bool {
        false
    }

    fn input_pressed(&mut self, _key: i32) -> bool {
        false
    }

    fn transform_move(&mut self, _dx: f32, _dy: f32) {}

    fn transform_x(&mut self) -> f32 {
        0.0
    }

    fn transform_y(&mut self) -> f32 {
        0.0
    }

    fn transform_set(&mut self, _x: f32, _y: f32) {}

    fn entity_x(&mut self, name: &str) -> Result<f32, String> {
        Err(format!("entity '{name}' not found"))
    }

    fn entity_y(&mut self, name: &str) -> Result<f32, String> {
        Err(format!("entity '{name}' not found"))
    }

    fn entity_destroy(&mut self, _name: &str) -> Result<(), String> {
        Ok(())
    }

    fn audio_play(&mut self, _name: &str) {}

    fn audio_loop(&mut self, _name: &str) {}

    fn audio_stop(&mut self) {}

    fn anim_play(&mut self, _clip: &str) {}

    fn game_restart(&mut self) {}

    fn game_set_title(&mut self, _title: &str) {}

    fn log(&mut self, _msg: &str) {}
}

impl EditorApp {
    pub(crate) fn build_play_runtime(&mut self) -> Result<Runtime, String> {
        let mut defs = Vec::new();
        for entity in &self.scene.entities {
            for entry in &entity.scripts {
                let path = self.dir.join(&entry.path);
                match std::fs::read_to_string(&path) {
                    Ok(source) => defs.push(ScriptDef {
                        entity: entity.id,
                        file: entry.path.clone(),
                        source,
                        values: entry.values.clone(),
                    }),
                    Err(e) => {
                        return Err(format!("read '{}': {e}", path.display()));
                    }
                }
            }
        }
        let mut logs = Vec::new();
        match Runtime::load(self.scene.clone(), defs, &mut logs) {
            Ok(runtime) => {
                for line in logs {
                    self.log(line);
                }
                Ok(runtime)
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub(crate) fn start_play(&mut self) {
        if self.play.is_some() {
            return;
        }
        match self.build_play_runtime() {
            Ok(runtime) => {
                self.play = Some(PlayState {
                    runtime,
                    last: std::time::Instant::now(),
                    ticks: 0,
                    sim_t: 0.0,
                    last_dt: 0.0,
                });
                if let Some(play) = &self.play {
                    self.last_phys_sig = physics_sig(play.runtime.scene());
                }
                if self.audio.is_none() {
                    self.audio = Some(ember_audio::AudioEngine::open());
                }
                self.selection.clear();
                self.log("playing".to_string());
            }
            Err(message) => self.log(format!("error: {message}")),
        }
    }

    pub(crate) fn stop_play(&mut self) {
        if self.play.take().is_some() {
            if let Some(engine) = &mut self.audio {
                engine.stop();
            }
            self.selection.clear();
            self.log("stopped".to_string());
        }
    }

    pub(crate) fn tick_play(&mut self, input: &mut GameInput, dt: f32) {
        enum Out {
            Logs(Vec<String>),
            Failed(String),
        }
        let out = match &mut self.play {
            None => return,
            Some(play) => {
                play.last = std::time::Instant::now();
                play.ticks += 1;
                play.sim_t += dt;
                play.last_dt = dt;
                let mut logs = Vec::new();
                match play.runtime.tick(input, dt, &mut logs) {
                    Ok(()) => Out::Logs(logs),
                    Err(e) => Out::Failed(e.to_string()),
                }
            }
        };
        match out {
            Out::Logs(logs) => {
                for line in logs {
                    self.log(line);
                }
                let restart = match &mut self.play {
                    Some(play) => play.runtime.take_restart_requested(),
                    None => false,
                };
                if restart {
                    match self.build_play_runtime() {
                        Ok(runtime) => {
                            if let Some(play) = &mut self.play {
                                play.runtime = runtime;
                                play.ticks = 0;
                                play.sim_t = 0.0;
                                play.last_dt = 0.0;
                                self.last_phys_sig = physics_sig(play.runtime.scene());
                            }
                            self.log("restarted".to_string());
                        }
                        Err(message) => {
                            self.log(format!("error: {message}"));
                            self.stop_play();
                        }
                    }
                }
                let cmds = match &mut self.play {
                    Some(play) => play.runtime.take_audio(),
                    None => Vec::new(),
                };
                if !cmds.is_empty() {
                    if self.audio.is_none() {
                        self.audio = Some(ember_audio::AudioEngine::open());
                    }
                    let dir = self.dir.clone();
                    let mut failures = Vec::new();
                    if let Some(engine) = &mut self.audio {
                        for cmd in cmds {
                            let result = match &cmd {
                                AudioCmd::Play(path) => engine.play(&dir.join(path)),
                                AudioCmd::Loop(path) => engine.play_loop(&dir.join(path)),
                                AudioCmd::Stop => {
                                    engine.stop();
                                    Ok(())
                                }
                            };
                            if let Err(e) = result {
                                failures.push(format!("error: {e}"));
                            }
                        }
                    }
                    for failure in failures {
                        self.log(failure);
                    }
                }
            }
            Out::Failed(message) => {
                self.play = None;
                self.selection.clear();
                self.log(format!("error: {message}"));
            }
        }
    }
}
