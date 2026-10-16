use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use eframe::egui;
use eframe::egui::containers::{CentralPanel, Panel};
use ember_core::{Ty, Vec2};
use ember_renderer::{Camera, Offscreen, load_png_file, pick_top, scene_sprites};
use ember_runtime::{InputState as GameInput, Runtime, ScriptDef};
use ember_scene::{
    BodyKind, Camera2D, Collider2D, EntityId, RigidBody2D, Scene, ScriptValue, SpriteRenderer,
};
use ember_vm::{Host, Value, Vm};

pub fn screen_to_world(click: (f32, f32), size: (f32, f32), camera: &Camera, aspect: f32) -> Vec2 {
    let half_h = camera.height / 2.0;
    let half_w = half_h * aspect;
    Vec2::new(
        camera.center.x + (click.0 / size.0 * 2.0 - 1.0) * half_w,
        camera.center.y + (1.0 - click.1 / size.1 * 2.0) * half_h,
    )
}

pub struct EditorApp {
    dir: PathBuf,
    project_name: String,
    scene: Scene,
    scene_rel: String,
    selection: Option<EntityId>,
    status: String,
    console: Vec<String>,
    diag: Vec<String>,
    scripts: Vec<String>,
    diag_sel: Option<String>,
    assets: Vec<String>,
    import_src: String,
    script_buf: String,
    off: Option<Offscreen>,
    tex: Option<egui::TextureId>,
    tex_size: (u32, u32),
    sizes: HashMap<String, (u32, u32)>,
    missing: HashSet<String>,
    frame: u64,
    script_cache: HashMap<String, (std::time::SystemTime, Vec<CachedVar>)>,
    new_name: String,
    rename: String,
    play: Option<PlayState>,
    last_phys_sig: Vec<(u64, bool, bool, u8, u32, u32)>,
}

fn physics_sig(scene: &Scene) -> Vec<(u64, bool, bool, u8, u32, u32)> {
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

struct PlayState {
    runtime: Runtime,
    last: std::time::Instant,
    ticks: u64,
    sim_t: f32,
    last_dt: f32,
}

#[derive(Clone)]
struct CachedVar {
    name: String,
    ty: Ty,
    default: ScriptValue,
}

struct VarRow {
    name: String,
    ty: Ty,
    value: ScriptValue,
    default: ScriptValue,
    overridden: bool,
}

struct NoopHost;

impl Host for NoopHost {
    fn input_down(&mut self, _key: i32) -> bool {
        false
    }

    fn transform_move(&mut self, _dx: f32, _dy: f32) {}

    fn log(&mut self, _msg: &str) {}
}

fn log_line(log: &mut Vec<String>, line: String) {
    log.push(line);
    if log.len() > 500 {
        log.drain(..log.len() - 500);
    }
}

fn open_external(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path.display().to_string()])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("open '{}': {e}", path.display()))
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("open externally is only supported on Windows".to_string())
    }
}

impl EditorApp {
    pub fn new(dir: &Path) -> Self {
        let mut app = Self {
            dir: dir.to_path_buf(),
            project_name: String::new(),
            scene: Scene::new("untitled"),
            scene_rel: ember_project::STARTUP_SCENE.to_string(),
            selection: None,
            status: String::new(),
            console: Vec::new(),
            diag: Vec::new(),
            scripts: Vec::new(),
            diag_sel: None,
            assets: Vec::new(),
            import_src: String::new(),
            script_buf: String::new(),
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
        };
        app.reload();
        app
    }

    pub fn playing(&self) -> bool {
        self.play.is_some()
    }

    fn start_play(&mut self) {
        if self.play.is_some() {
            return;
        }
        let mut defs = Vec::new();
        for entity in &self.scene.entities {
            let Some(script) = &entity.script else {
                continue;
            };
            let path = self.dir.join(script);
            match std::fs::read_to_string(&path) {
                Ok(source) => defs.push(ScriptDef {
                    entity: entity.id,
                    file: script.clone(),
                    source,
                    values: entity.script_values.clone(),
                }),
                Err(e) => {
                    self.log(format!("error: read '{}': {e}", path.display()));
                    return;
                }
            }
        }
        let mut logs = Vec::new();
        match Runtime::load(self.scene.clone(), defs, &mut logs) {
            Ok(runtime) => {
                for line in logs {
                    self.log(line);
                }
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
                self.selection = None;
                self.log("playing".to_string());
            }
            Err(e) => self.log(format!("error: {e}")),
        }
    }

    fn stop_play(&mut self) {
        if self.play.take().is_some() {
            self.selection = None;
            self.log("stopped".to_string());
        }
    }

    fn tick_play(&mut self, input: &GameInput, dt: f32) {
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
            }
            Out::Failed(message) => {
                self.play = None;
                self.selection = None;
                self.log(format!("error: {message}"));
            }
        }
    }

    fn log(&mut self, line: String) {
        self.status = line.clone();
        log_line(&mut self.console, line);
    }

    fn refresh_lists(&mut self) {
        self.scripts = ember_project::script_stems(&self.dir)
            .unwrap_or_default()
            .iter()
            .map(|s| format!("scripts/{s}.em"))
            .collect();
        if self
            .diag_sel
            .as_ref()
            .is_none_or(|s| !self.scripts.contains(s))
        {
            self.diag_sel = self.scripts.first().cloned();
        }
        self.assets = ember_project::list_assets(&self.dir).unwrap_or_default();
    }

    fn reload(&mut self) {
        self.stop_play();
        match ember_project::load_project(&self.dir) {
            Ok(loaded) => {
                self.project_name = loaded.project.name.clone();
                self.scene_rel = loaded.project.startup_scene.clone();
                self.scene = loaded.scene;
                self.selection = None;
                self.refresh_lists();
                self.log(format!("opened '{}'", self.project_name));
            }
            Err(e) => {
                self.log(format!("error: {e}"));
            }
        }
    }

    fn save(&mut self) {
        let path = self.dir.join(&self.scene_rel);
        match self.scene.save_to_file(&path) {
            Ok(()) => {
                self.log(format!("saved '{}'", self.scene_rel));
                self.check_scripts();
            }
            Err(e) => self.log(format!("error: {e}")),
        }
    }

    fn check_scripts(&mut self) {
        match ember_project::check_dir(&self.dir) {
            Ok(report) => {
                self.diag = report.errors.iter().map(|e| e.to_string()).collect();
                if report.errors.is_empty() {
                    self.log(format!("check ok: {} script(s)", report.checked));
                } else {
                    self.log(format!("check: {} error(s)", report.errors.len()));
                }
            }
            Err(e) => self.log(format!("error: {e}")),
        }
    }

    fn ensure_textures(
        scene: &Scene,
        dir: &Path,
        sizes: &mut HashMap<String, (u32, u32)>,
        missing: &mut HashSet<String>,
        retry_missing: bool,
        status: &mut String,
        off: &mut Offscreen,
    ) -> Vec<String> {
        let mut events = Vec::new();
        for entity in &scene.entities {
            let Some(sprite) = &entity.sprite else {
                continue;
            };
            if sizes.contains_key(&sprite.texture) {
                continue;
            }
            if missing.contains(&sprite.texture) && !retry_missing {
                continue;
            }
            match load_png_file(&dir.join(&sprite.texture)) {
                Ok(img) => {
                    sizes.insert(sprite.texture.clone(), (img.width, img.height));
                    missing.remove(&sprite.texture);
                    if let Err(e) = off.cache_texture(&sprite.texture, &img) {
                        *status = format!("error: {e}");
                    } else {
                        events.push(format!("loaded '{}'", sprite.texture));
                    }
                }
                Err(e) => {
                    *status = format!("error: {e}");
                    missing.insert(sprite.texture.clone());
                }
            }
        }
        events
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(format!("Ember — {}", self.project_name));
                if ui.button("Save").clicked() {
                    self.save();
                }
                if ui.button("Reload").clicked() {
                    self.reload();
                }
                if !self.playing() {
                    if ui.button("Play").clicked() {
                        self.start_play();
                    }
                } else {
                    if let Some(play) = &self.play {
                        ui.label(format!(
                            "PLAY t={:.1}s n={} dt={:.1}ms phys={}",
                            play.sim_t,
                            play.ticks,
                            play.last_dt * 1000.0,
                            play.runtime.physics_active()
                        ));
                    }
                    if ui.button("Stop").clicked() {
                        self.stop_play();
                    }
                }
                ui.label(&self.status);
            });
        });
    }

    fn hierarchy(&mut self, ui: &mut egui::Ui) {
        Panel::left("hierarchy").show(ui, |ui| {
            ui.heading("Hierarchy");
            let ids: Vec<(EntityId, String)> = match &self.play {
                Some(p) => p.runtime.scene(),
                None => &self.scene,
            }
            .entities
            .iter()
            .map(|e| (e.id, e.name.clone()))
            .collect();
            for (id, name) in ids {
                let selected = self.selection == Some(id);
                if ui
                    .selectable_label(selected, format!("{name} ({})", id.0))
                    .clicked()
                {
                    self.selection = Some(id);
                    self.rename = name;
                    self.script_buf = match &self.play {
                        Some(p) => p.runtime.scene(),
                        None => &self.scene,
                    }
                    .entities
                    .iter()
                    .find(|e| e.id == id)
                    .and_then(|e| e.script.clone())
                    .unwrap_or_default();
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.new_name);
                if ui.button("+ Entity").clicked() && !self.new_name.is_empty() {
                    let name = self.new_name.clone();
                    let scene: &mut Scene = match &mut self.play {
                        Some(p) => p.runtime.scene_mut(),
                        None => &mut self.scene,
                    };
                    let id = scene.spawn(&name);
                    self.selection = Some(id);
                }
            });
            if self.selection.is_some_and(|id| {
                match &self.play {
                    Some(p) => p.runtime.scene(),
                    None => &self.scene,
                }
                .entities
                .iter()
                .any(|e| e.id == id)
            }) && ui.button("Delete").clicked()
                && let Some(id) = self.selection
            {
                let scene: &mut Scene = match &mut self.play {
                    Some(p) => p.runtime.scene_mut(),
                    None => &mut self.scene,
                };
                scene.entities.retain(|e| e.id != id);
                self.selection = None;
            }
            ui.separator();
            ui.heading("Assets");
            for name in &self.assets {
                ui.label(format!("assets/{name}"));
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.import_src)
                        .hint_text("PNG source path, absolute or from terminal dir"),
                );
                if ui.button("Import").clicked() {
                    let src = PathBuf::from(self.import_src.clone());
                    match ember_project::import_asset(&self.dir, &src) {
                        Ok(rel) => {
                            self.log(format!("imported '{rel}'"));
                            self.assets = ember_project::list_assets(&self.dir).unwrap_or_default();
                        }
                        Err(e) => self.log(format!("error: {e}")),
                    }
                }
            });
        });
    }

    fn script_globals(source: &str) -> Result<Vec<(String, Ty)>, String> {
        let program = ember_lang::parse("script", source).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for item in &program.items {
            if let ember_lang::ast::Item::GlobalVar(g) = item
                && let Some(ty) = Ty::from_name(&g.ty.name)
                && matches!(ty, Ty::I32 | Ty::F32 | Ty::Bool | Ty::Str)
            {
                out.push((g.name.clone(), ty));
            }
        }
        Ok(out)
    }

    fn script_defaults(file: &str, source: &str) -> Result<HashMap<String, ScriptValue>, String> {
        let chunk = ember_lang::compile::compile(file, source).map_err(|es| {
            es.iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        let mut vm = Vm::new(chunk).map_err(|e| e.to_string())?;
        let mut host = NoopHost;
        vm.call(&mut host, "$init", vec![])
            .map_err(|e| e.to_string())?;
        let mut out = HashMap::new();
        for (name, ty) in Self::script_globals(source)? {
            let value = vm
                .global_index(&name)
                .and_then(|idx| vm.globals().get(idx).cloned())
                .and_then(|v| match (v, ty) {
                    (Value::I32(x), Ty::I32) => Some(ScriptValue::I32(x)),
                    (Value::F32(x), Ty::F32) => Some(ScriptValue::F32(x)),
                    (Value::Bool(x), Ty::Bool) => Some(ScriptValue::Bool(x)),
                    (Value::Str(x), Ty::Str) => Some(ScriptValue::Str(x)),
                    _ => None,
                });
            if let Some(value) = value {
                out.insert(name, value);
            }
        }
        Ok(out)
    }

    fn script_rows(
        &mut self,
        script: Option<String>,
        overrides: &HashMap<String, ScriptValue>,
    ) -> Result<Vec<VarRow>, String> {
        let Some(script) = script else {
            return Ok(Vec::new());
        };
        let path = self.dir.join(&script);
        let mtime = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .map_err(|e| format!("read '{}': {e}", path.display()))?;
        let cached = match self.script_cache.get(&script) {
            Some((known, vars)) if *known == mtime => vars.clone(),
            _ => {
                let source = std::fs::read_to_string(&path)
                    .map_err(|e| format!("read '{}': {e}", path.display()))?;
                let globals = Self::script_globals(&source)?;
                let defaults = Self::script_defaults(&script, &source)?;
                let vars: Vec<CachedVar> = globals
                    .into_iter()
                    .filter_map(|(name, ty)| {
                        defaults.get(&name).map(|default| CachedVar {
                            name,
                            ty,
                            default: default.clone(),
                        })
                    })
                    .collect();
                self.script_cache
                    .insert(script.clone(), (mtime, vars.clone()));
                vars
            }
        };
        Ok(cached
            .into_iter()
            .map(|v| {
                let overridden = overrides.get(&v.name);
                VarRow {
                    value: overridden.cloned().unwrap_or(v.default.clone()),
                    name: v.name,
                    ty: v.ty,
                    default: v.default,
                    overridden: overridden.is_some(),
                }
            })
            .collect())
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        Panel::right("inspector").show(ui, |ui| {
            ui.heading("Inspector");
            let playing = self.play.is_some();
            if playing {
                ui.label("PLAYING — edits are temporary");
            }
            let selection = self.selection;
            let linked: Option<(String, HashMap<String, ScriptValue>)> = {
                let scene: &Scene = match &self.play {
                    Some(p) => p.runtime.scene(),
                    None => &self.scene,
                };
                selection
                    .and_then(|id| scene.entities.iter().find(|e| e.id == id))
                    .and_then(|e| e.script.clone().map(|s| (s, e.script_values.clone())))
            };
            let rows_result = match linked {
                Some((script, overrides)) => self.script_rows(Some(script), &overrides),
                None => Ok(Vec::new()),
            };
            let scene: &mut Scene = match &mut self.play {
                Some(p) => p.runtime.scene_mut(),
                None => &mut self.scene,
            };
            let Some(entity) =
                selection.and_then(|id| scene.entities.iter_mut().find(|e| e.id == id))
            else {
                ui.label("nothing selected");
                return;
            };
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut entity.name);
            });
            ui.label("Transform");
            ui.horizontal(|ui| {
                ui.label("pos");
                ui.add(egui::DragValue::new(&mut entity.transform.position.x).speed(1.0));
                ui.add(egui::DragValue::new(&mut entity.transform.position.y).speed(1.0));
            });
            ui.horizontal(|ui| {
                ui.label("rot");
                ui.add(egui::DragValue::new(&mut entity.transform.rotation_degrees).speed(1.0));
            });
            ui.horizontal(|ui| {
                ui.label("scale");
                ui.add(egui::DragValue::new(&mut entity.transform.scale.x).speed(0.1));
                ui.add(egui::DragValue::new(&mut entity.transform.scale.y).speed(0.1));
            });
            if let Some(sprite) = &mut entity.sprite {
                ui.label("Sprite");
                ui.horizontal(|ui| {
                    ui.label("texture");
                    ui.text_edit_singleline(&mut sprite.texture);
                });
                ui.horizontal(|ui| {
                    ui.label("order");
                    ui.add(egui::DragValue::new(&mut sprite.sorting_order));
                });
                ui.horizontal(|ui| {
                    ui.label("tint");
                    for channel in &mut sprite.tint {
                        ui.add(egui::DragValue::new(channel).speed(0.05).range(0.0..=1.0));
                    }
                });
            } else if ui.button("+ Sprite").clicked() {
                entity.sprite = Some(SpriteRenderer {
                    texture: String::new(),
                    tint: [1.0, 1.0, 1.0, 1.0],
                    sorting_order: 0,
                });
            }
            if let Some(camera) = &mut entity.camera {
                ui.label("Camera");
                ui.horizontal(|ui| {
                    ui.label("height");
                    ui.add(
                        egui::DragValue::new(&mut camera.height)
                            .speed(0.5)
                            .range(0.1..=1000.0),
                    );
                });
            } else if ui.button("+ Camera").clicked() {
                entity.camera = Some(Camera2D { height: 12.0 });
            }
            if let Some(collider) = &mut entity.collider {
                ui.label("Collider");
                ui.horizontal(|ui| {
                    ui.label("half");
                    ui.add(
                        egui::DragValue::new(&mut collider.hx)
                            .speed(0.1)
                            .range(0.01..=1000.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut collider.hy)
                            .speed(0.1)
                            .range(0.01..=1000.0),
                    );
                });
            } else if ui.button("+ Collider").clicked() {
                entity.collider = Some(Collider2D { hx: 0.5, hy: 0.5 });
            }
            if let Some(body) = &mut entity.rigidbody {
                ui.label("RigidBody");
                ui.horizontal(|ui| {
                    ui.label("kind");
                    egui::ComboBox::from_label("")
                        .selected_text(format!("{:?}", body.kind))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut body.kind, BodyKind::Static, "Static");
                            ui.selectable_value(&mut body.kind, BodyKind::Dynamic, "Dynamic");
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("vel");
                    ui.add(egui::DragValue::new(&mut body.velocity.x).speed(0.5));
                    ui.add(egui::DragValue::new(&mut body.velocity.y).speed(0.5));
                });
                ui.horizontal(|ui| {
                    ui.label("grav");
                    ui.add(
                        egui::DragValue::new(&mut body.gravity_scale)
                            .speed(0.1)
                            .range(0.0..=10.0),
                    );
                });
            } else if ui.button("+ RigidBody").clicked() {
                entity.rigidbody = Some(RigidBody2D {
                    kind: BodyKind::Dynamic,
                    velocity: Vec2::zero(),
                    gravity_scale: 1.0,
                });
            }
            ui.label("Script");
            if let Some(current) = entity.script.clone() {
                ui.label(format!("linked: {current}"));
            } else {
                ui.label("linked: none");
            }
            let mut note: Option<String> = None;
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.script_buf);
                if ui.button("Set").clicked() {
                    let buf = self.script_buf.clone();
                    match ember_project::validate_asset_path(&self.dir, &buf) {
                        Ok(path) if buf.ends_with(".em") && path.is_file() => {
                            entity.script = Some(buf.clone());
                            self.script_buf = buf.clone();
                            note = Some(format!("linked '{buf}'"));
                        }
                        Ok(_) => {
                            note = Some(format!(
                                "error: script '{buf}' must be an existing .em file"
                            ))
                        }
                        Err(e) => note = Some(format!("error: {e}")),
                    }
                }
                if ui.button("Clear").clicked() {
                    entity.script = None;
                    note = Some("script cleared".to_string());
                }
            });
            match rows_result {
                Ok(rows) => {
                    let mut edits: HashMap<String, ScriptValue> = HashMap::new();
                    let mut reverts: Vec<String> = Vec::new();
                    for row in &rows {
                        ui.horizontal(|ui| {
                            ui.label(format!("{} ({})", row.name, row.ty));
                            let mut value = row.value.clone();
                            let changed = match &mut value {
                                ScriptValue::I32(v) => ui.add(egui::DragValue::new(v)).changed(),
                                ScriptValue::F32(v) => {
                                    ui.add(egui::DragValue::new(v).speed(0.1)).changed()
                                }
                                ScriptValue::Bool(v) => ui.checkbox(v, "").changed(),
                                ScriptValue::Str(v) => ui.text_edit_singleline(v).changed(),
                            };
                            if changed {
                                edits.insert(row.name.clone(), value);
                            }
                            if row.overridden && ui.button("Revert").clicked() {
                                reverts.push(row.name.clone());
                            }
                        });
                    }
                    for (name, value) in &edits {
                        entity.script_values.insert(name.clone(), value.clone());
                    }
                    for name in &reverts {
                        entity.script_values.remove(name);
                    }
                    let live: Vec<(String, ScriptValue)> = rows
                        .iter()
                        .map(|row| {
                            let value = if reverts.contains(&row.name) {
                                row.default.clone()
                            } else {
                                entity
                                    .script_values
                                    .get(&row.name)
                                    .cloned()
                                    .unwrap_or(row.value.clone())
                            };
                            (row.name.clone(), value)
                        })
                        .collect();
                    let entity_id = entity.id;
                    if playing && let Some(play) = &mut self.play {
                        for (name, value) in &live {
                            let runtime_value = match value {
                                ScriptValue::I32(v) => Value::I32(*v),
                                ScriptValue::F32(v) => Value::F32(*v),
                                ScriptValue::Bool(v) => Value::Bool(*v),
                                ScriptValue::Str(v) => Value::Str(v.clone()),
                            };
                            if let Err(e) = play.runtime.set_global(entity_id, name, runtime_value)
                            {
                                note = Some(format!("error: {e}"));
                            }
                        }
                    }
                }
                Err(message) => {
                    ui.label(format!("script: {message}"));
                }
            }
            if let Some(message) = note {
                self.log(message);
            }
        });
    }

    fn viewport(&mut self, ui: &mut egui::Ui, rs: Option<&eframe::egui_wgpu::RenderState>) {
        CentralPanel::default().show(ui, |ui| {
            let Some(rs) = rs else {
                ui.label("error: no wgpu device");
                return;
            };
            let avail = ui.available_size();
            let scale = ui.ctx().pixels_per_point();
            let (w, h) = (
                (avail.x * scale).max(1.0) as u32,
                (avail.y * scale).max(1.0) as u32,
            );
            if self.off.is_none() {
                self.off = Some(Offscreen::new(&rs.device, &rs.queue, w, h));
                self.tex_size = (0, 0);
            }
            let poll = self.frame.is_multiple_of(30);
            if poll {
                let listed = ember_project::list_assets(&self.dir).unwrap_or_default();
                for name in &listed {
                    if !self.assets.contains(name) {
                        self.log(format!("detected 'assets/{name}'"));
                    }
                }
                self.assets = listed;
            }
            let events = {
                let Some(off) = &mut self.off else {
                    return;
                };
                off.resize(w, h);
                Self::ensure_textures(
                    match &self.play {
                        Some(p) => p.runtime.scene(),
                        None => &self.scene,
                    },
                    &self.dir,
                    &mut self.sizes,
                    &mut self.missing,
                    poll,
                    &mut self.status,
                    off,
                )
            };
            for event in events {
                self.log(event);
            }
            let Some(off) = &mut self.off else {
                return;
            };
            let (camera, draws) = scene_sprites(
                match &self.play {
                    Some(p) => p.runtime.scene(),
                    None => &self.scene,
                },
                &self.sizes,
            );
            if let Err(e) = off.render(&camera, &draws) {
                self.status = format!("error: {e}");
                return;
            }
            if self.tex_size != (w, h) {
                if let Some(old) = self.tex.take() {
                    rs.renderer.write().free_texture(&old);
                }
                self.tex = Some(rs.renderer.write().register_native_texture(
                    &rs.device,
                    off.view(),
                    eframe::egui_wgpu::wgpu::FilterMode::Linear,
                ));
                self.tex_size = (w, h);
            }
            let Some(tex) = self.tex else {
                return;
            };
            let (rect, resp) = ui.allocate_exact_size(avail, egui::Sense::click());
            ui.painter().image(
                tex,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            if resp.clicked()
                && let Some(pos) = resp.interact_pointer_pos()
            {
                let local = (pos.x - rect.min.x, pos.y - rect.min.y);
                let world =
                    screen_to_world(local, (rect.width(), rect.height()), &camera, off.aspect());
                let quads: Vec<[Vec2; 4]> = draws.iter().map(|d| d.corners).collect();
                let mut order: Vec<(i32, EntityId)> = match &self.play {
                    Some(p) => p.runtime.scene(),
                    None => &self.scene,
                }
                .entities
                .iter()
                .filter_map(|e| e.sprite.as_ref().map(|s| (s.sorting_order, e.id)))
                .collect();
                order.sort_by_key(|(seq, _)| *seq);
                if order.len() == quads.len()
                    && let Some(hit) = pick_top(&quads, world)
                    && let Some((_, id)) = order.get(hit)
                {
                    self.selection = Some(*id);
                }
            }
        });
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        ui.ctx().request_repaint();
        self.frame = self.frame.wrapping_add(1);
        if self.play.is_some() {
            let mut input = GameInput::new();
            ui.ctx().input(|i| {
                if i.key_down(egui::Key::ArrowUp) {
                    input.press(0);
                }
                if i.key_down(egui::Key::ArrowDown) {
                    input.press(1);
                }
                if i.key_down(egui::Key::ArrowLeft) {
                    input.press(2);
                }
                if i.key_down(egui::Key::ArrowRight) {
                    input.press(3);
                }
                if i.key_down(egui::Key::Space) {
                    input.press(4);
                }
            });
            let escape = ui.ctx().input(|i| i.key_down(egui::Key::Escape));
            if escape {
                self.stop_play();
            } else {
                let sig = match &self.play {
                    Some(p) => physics_sig(p.runtime.scene()),
                    None => Vec::new(),
                };
                if sig != self.last_phys_sig {
                    self.last_phys_sig = sig;
                    let rebuilt = match &mut self.play {
                        Some(play) => play.runtime.rebuild_physics(),
                        None => Ok(()),
                    };
                    if let Err(message) = rebuilt {
                        self.log(format!("error: {message}"));
                        self.stop_play();
                    }
                }
                let dt = match &mut self.play {
                    Some(play) => {
                        let now = std::time::Instant::now();
                        let dt = now.duration_since(play.last).as_secs_f32().min(0.1);
                        play.last = now;
                        dt
                    }
                    None => 0.0,
                };
                self.tick_play(&input, dt);
            }
        }
        self.toolbar(ui);
        self.hierarchy(ui);
        self.inspector(ui);
        if frame.wgpu_render_state().is_some() {
            let rs = frame.wgpu_render_state();
            self.viewport(ui, rs);
        } else {
            CentralPanel::default().show(ui, |ui| {
                ui.label("error: no wgpu device");
            });
        }
        let status = self.status.clone();
        Panel::bottom("diagnostics").show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Scripts");
                        if ui.button("Check").clicked() {
                            self.check_scripts();
                            self.refresh_lists();
                        }
                    });
                    for script in self.scripts.clone() {
                        let selected = self.diag_sel.as_ref() == Some(&script);
                        ui.horizontal(|ui| {
                            if ui.selectable_label(selected, &script).clicked() {
                                self.diag_sel = Some(script.clone());
                            }
                            if ui.button("Open").clicked() {
                                let path = self.dir.join(&script);
                                match open_external(&path) {
                                    Ok(()) => self.log(format!("opened '{script}'")),
                                    Err(e) => self.log(format!("error: {e}")),
                                }
                            }
                        });
                    }
                    ui.heading("Diagnostics");
                    if self.diag.is_empty() {
                        ui.label("no errors");
                    }
                    for line in self.diag.clone() {
                        ui.label(line);
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Console");
                        if ui.button("Clear").clicked() {
                            self.console.clear();
                        }
                    });
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for line in self.console.clone() {
                            ui.label(line);
                        }
                    });
                });
            });
            ui.label(status);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ember_renderer::sprite_quad;

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
        app.tick_play(&input, 1.0 / 60.0);
        let (rx, _) = pos_of(app.play.as_ref().unwrap().runtime.scene(), "Player");
        assert!(rx > 0.0);
        assert_eq!(pos_of(&app.scene, "Player").0, 0.0);
        app.stop_play();
        assert!(!app.playing());
        assert_eq!(app.scene.save_to_string().unwrap(), edit_before);
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
        app.tick_play(&GameInput::new(), 1.0 / 60.0);
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
            .and_then(|id| app.scene.entities.iter().find(|e| e.id == id))
            .and_then(|e| e.script.clone().map(|s| (s, e.script_values.clone())))
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
        app.selection = Some(id);
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
            .script_values
            .insert("speed".to_string(), ScriptValue::F32(6.0));
        app.selection = Some(id);
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
        app.selection = Some(id);
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
                .script_values
                .insert("speed".to_string(), ScriptValue::F32(1.0));
        }
        app.play
            .as_mut()
            .unwrap()
            .runtime
            .set_global(id, "speed", Value::F32(1.0))
            .unwrap();
        assert_eq!(
            app.play.as_ref().unwrap().runtime.global(id, "speed"),
            Some(Value::F32(1.0))
        );
        app.stop_play();
        assert!(!app.playing());
        let entity = app.scene.entities.iter().find(|e| e.id == id).unwrap();
        assert_eq!(entity.transform.position.x, 0.0);
        assert!(entity.script_values.is_empty());
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
        entity.collider = Some(Collider2D { hx: 0.5, hy: 0.5 });
        app.start_play();
        assert!(app.playing());
        for _ in 0..60 {
            app.tick_play(&GameInput::new(), 1.0 / 60.0);
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
