use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use eframe::egui;
use eframe::egui::containers::{CentralPanel, Panel};
use ember_core::{Ty, Vec2};
use ember_renderer::{Camera, Offscreen, load_png_file, pick_top, scene_sprites};
use ember_runtime::{AudioCmd, InputState as GameInput, Runtime, ScriptDef};
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
    selection: Vec<EntityId>,
    undo: Vec<String>,
    redo: Vec<String>,
    anim_clip_name: String,
    anim_frame_path: String,
    scene_music_buf: String,
    status: String,
    console: Vec<String>,
    diag: Vec<String>,
    scripts: Vec<String>,
    diag_sel: Option<String>,
    assets: Vec<String>,
    import_src: String,
    comp_search: String,
    add_menu_open: bool,
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
    title_restore: bool,
    audio: Option<ember_audio::AudioEngine>,
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

fn log_line(log: &mut Vec<String>, line: String) {
    log.push(line);
    if log.len() > 500 {
        log.drain(..log.len() - 500);
    }
}

fn chevron_button(ui: &mut egui::Ui, open: bool) -> egui::Response {
    let size = egui::vec2(16.0, 16.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let c = rect.center();
    let s = 4.0;
    let points = if open {
        vec![
            egui::pos2(c.x - s, c.y - s * 0.6),
            egui::pos2(c.x + s, c.y - s * 0.6),
            egui::pos2(c.x, c.y + s * 0.6),
        ]
    } else {
        vec![
            egui::pos2(c.x - s * 0.6, c.y - s),
            egui::pos2(c.x - s * 0.6, c.y + s),
            egui::pos2(c.x + s * 0.6, c.y),
        ]
    };
    ui.painter().add(egui::Shape::convex_polygon(
        points,
        ui.visuals().text_color(),
        egui::Stroke::NONE,
    ));
    resp.on_hover_text(if open { "hide" } else { "show" })
}
fn script_stem(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .strip_suffix(".em")
        .unwrap_or(path)
}

fn component_header(
    ui: &mut egui::Ui,
    salt: (&str, u64),
    title: &str,
    menu: impl FnOnce(&mut egui::Ui),
) -> bool {
    use egui::collapsing_header::CollapsingState;
    let id = ui.make_persistent_id(salt);
    let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, true);
    let open = state.is_open();
    ui.horizontal(|ui| {
        if chevron_button(ui, open).clicked() {
            state.toggle(ui);
        }
        ui.label(title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let resp = ui.menu_button("", menu).response;
            let c = resp.rect.center();
            let color = ui.visuals().text_color();
            for dy in [-4.0, 0.0, 4.0] {
                ui.painter()
                    .circle_filled(egui::pos2(c.x, c.y + dy), 1.8, color);
            }
        });
    });
    state.store(ui.ctx());
    state.is_open()
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

    fn primary(&self) -> Option<EntityId> {
        self.selection.first().copied()
    }

    fn select_only(&mut self, id: EntityId) {
        self.selection = vec![id];
    }

    fn toggle_select(&mut self, id: EntityId) {
        if let Some(i) = self.selection.iter().position(|s| *s == id) {
            self.selection.remove(i);
        } else {
            self.selection.push(id);
        }
    }

    fn push_undo(undo: &mut Vec<String>, redo: &mut Vec<String>, scene: &Scene) {
        if let Ok(ron) = scene.save_to_string() {
            undo.push(ron);
            if undo.len() > 100 {
                undo.remove(0);
            }
            redo.clear();
        }
    }

    fn apply_snapshot(&mut self, ron: &str) {
        match Scene::load_from_str(ron, "undo") {
            Ok(scene) => {
                self.scene = scene;
                let valid: HashSet<EntityId> = self.scene.entities.iter().map(|e| e.id).collect();
                self.selection.retain(|id| valid.contains(id));
            }
            Err(e) => self.log(format!("error: {e}")),
        }
    }

    fn undo(&mut self) {
        if self.play.is_some() {
            return;
        }
        let Ok(cur) = self.scene.save_to_string() else {
            return;
        };
        if let Some(ron) = self.undo.pop() {
            self.redo.push(cur);
            self.apply_snapshot(&ron);
        }
    }

    fn redo(&mut self) {
        if self.play.is_some() {
            return;
        }
        let Ok(cur) = self.scene.save_to_string() else {
            return;
        };
        if let Some(ron) = self.redo.pop() {
            self.undo.push(cur);
            self.apply_snapshot(&ron);
        }
    }

    fn delete_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        if self.play.is_none() {
            Self::push_undo(&mut self.undo, &mut self.redo, &self.scene);
        }
        let ids = std::mem::take(&mut self.selection);
        let scene: &mut Scene = match &mut self.play {
            Some(p) => p.runtime.scene_mut(),
            None => &mut self.scene,
        };
        scene.entities.retain(|e| !ids.contains(&e.id));
    }

    fn move_selection(&mut self, dx: f32, dy: f32) {
        if self.selection.is_empty() || (dx == 0.0 && dy == 0.0) {
            return;
        }
        let ids = self.selection.clone();
        let scene: &mut Scene = match &mut self.play {
            Some(p) => p.runtime.scene_mut(),
            None => &mut self.scene,
        };
        for entity in scene.entities.iter_mut().filter(|e| ids.contains(&e.id)) {
            entity.transform.position.x += dx;
            entity.transform.position.y += dy;
        }
    }

    fn build_play_runtime(&mut self) -> Result<Runtime, String> {
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

    fn start_play(&mut self) {
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

    fn stop_play(&mut self) {
        if self.play.take().is_some() {
            if let Some(engine) = &mut self.audio {
                engine.stop();
            }
            self.selection.clear();
            self.log("stopped".to_string());
        }
    }

    fn tick_play(&mut self, input: &mut GameInput, dt: f32) {
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

    fn set_scene_music(&mut self, raw: &str) -> Result<(), String> {
        let path = raw.trim();
        if path.is_empty() {
            self.scene.music = None;
            return Ok(());
        }
        let lower = path.to_lowercase();
        if !(lower.ends_with(".wav") || lower.ends_with(".ogg")) {
            return Err(format!("music '{path}' must be .wav or .ogg"));
        }
        if !self.dir.join(path).is_file() {
            return Err(format!("music '{path}' not found in project"));
        }
        self.scene.music = Some(path.to_string());
        Ok(())
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
                self.selection.clear();
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
            for path in entity.texture_paths() {
                if sizes.contains_key(path) {
                    continue;
                }
                if missing.contains(path) && !retry_missing {
                    continue;
                }
                match load_png_file(&dir.join(path)) {
                    Ok(img) => {
                        sizes.insert(path.to_string(), (img.width, img.height));
                        missing.remove(path);
                        if let Err(e) = off.cache_texture(path, &img) {
                            *status = format!("error: {e}");
                        } else {
                            events.push(format!("loaded '{path}'"));
                        }
                    }
                    Err(e) => {
                        *status = format!("error: {e}");
                        missing.insert(path.to_string());
                    }
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
                let can_history = !self.playing();
                if ui
                    .add_enabled(
                        can_history && !self.undo.is_empty(),
                        egui::Button::new("Undo"),
                    )
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(
                        can_history && !self.redo.is_empty(),
                        egui::Button::new("Redo"),
                    )
                    .clicked()
                {
                    self.redo();
                }
                if !self.playing() {
                    if ui.button("Play").clicked() {
                        self.start_play();
                    }
                } else {
                    // if let Some(play) = &self.play {
                    //     ui.label(format!(
                    //         "PLAY t={:.1}s n={} dt={:.1}ms phys={}",
                    //         play.sim_t,
                    //         play.ticks,
                    //         play.last_dt * 1000.0,
                    //         play.runtime.physics_active()
                    //     ));
                    // }
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
                let selected = self.selection.contains(&id);
                if ui
                    .selectable_label(selected, format!("{name} ({})", id.0))
                    .clicked()
                {
                    let ctrl = ui.ctx().input(|i| i.modifiers.ctrl);
                    if ctrl {
                        self.toggle_select(id);
                    } else {
                        self.select_only(id);
                    }
                    self.rename = name;
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.new_name);
                if ui.button("+ Entity").clicked() && !self.new_name.is_empty() {
                    let name = self.new_name.clone();
                    if self.play.is_none() {
                        Self::push_undo(&mut self.undo, &mut self.redo, &self.scene);
                    }
                    let scene: &mut Scene = match &mut self.play {
                        Some(p) => p.runtime.scene_mut(),
                        None => &mut self.scene,
                    };
                    let id = scene.spawn(&name);
                    self.select_only(id);
                }
            });
            if !self.selection.is_empty() && ui.button("Delete").clicked() {
                self.delete_selection();
            }
            ui.separator();
            ui.heading("Assets");
            for name in &self.assets {
                ui.label(format!("assets/{name}"));
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.import_src)
                        .hint_text("Asset source path (.png/.wav/.ogg)"),
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
            if self.selection.len() > 1 {
                ui.label(format!("{} selected (editing first)", self.selection.len()));
            }
            let selection = self.primary();
            let linked: Vec<(String, HashMap<String, ScriptValue>)> = {
                let scene: &Scene = match &self.play {
                    Some(p) => p.runtime.scene(),
                    None => &self.scene,
                };
                selection
                    .and_then(|id| scene.entities.iter().find(|e| e.id == id))
                    .map(|e| {
                        e.scripts
                            .iter()
                            .map(|s| (s.path.clone(), s.values.clone()))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let rows_list: Vec<(String, Result<Vec<VarRow>, String>)> = linked
                .into_iter()
                .map(|(script, overrides)| {
                    let rows = self.script_rows(Some(script.clone()), &overrides);
                    (script, rows)
                })
                .collect();
            let scene: &mut Scene = match &mut self.play {
                Some(p) => p.runtime.scene_mut(),
                None => &mut self.scene,
            };
            let Some(entity) =
                selection.and_then(|id| scene.entities.iter_mut().find(|e| e.id == id))
            else {
                ui.label("nothing selected");
                ui.separator();
                ui.label("Scene music (loop on Play)");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.scene_music_buf);
                    if ui.button("Set").clicked() {
                        let buf = self.scene_music_buf.clone();
                        match self.set_scene_music(&buf) {
                            Ok(()) => self.log(format!("music '{buf}'")),
                            Err(e) => self.log(format!("error: {e}")),
                        }
                    }
                    if ui.button("Clear").clicked() {
                        self.scene.music = None;
                        self.scene_music_buf.clear();
                    }
                });
                if let Some(music) = &self.scene.music {
                    ui.label(format!("playing: {music}"));
                }
                return;
            };
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut entity.name);
            });
            let mut reset_transform = false;
            if component_header(ui, ("transform", entity.id.0), "Transform", |ui| {
                if ui.button("Reset").clicked() {
                    reset_transform = true;
                }
            }) {
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
            }
            if reset_transform {
                entity.transform.position = Vec2::zero();
                entity.transform.rotation_degrees = 0.0;
                entity.transform.scale = Vec2::new(1.0, 1.0);
            }
            if entity.sprite.is_some() {
                let mut drop_it = false;
                let mut reset_it = false;
                if component_header(ui, ("sprite", entity.id.0), "Sprite", |ui| {
                    if ui.button("Reset").clicked() {
                        reset_it = true;
                    }
                    if ui.button("Remove").clicked() {
                        drop_it = true;
                    }
                }) && let Some(sprite) = entity.sprite.as_mut()
                {
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
                }
                if reset_it {
                    entity.sprite = Some(SpriteRenderer {
                        texture: String::new(),
                        tint: [1.0, 1.0, 1.0, 1.0],
                        sorting_order: 0,
                    });
                }
                if drop_it {
                    entity.sprite = None;
                }
            }
            if entity.camera.is_some() {
                let mut drop_it = false;
                let mut reset_it = false;
                if component_header(ui, ("camera", entity.id.0), "Camera", |ui| {
                    if ui.button("Reset").clicked() {
                        reset_it = true;
                    }
                    if ui.button("Remove").clicked() {
                        drop_it = true;
                    }
                }) && let Some(camera) = entity.camera.as_mut()
                {
                    ui.horizontal(|ui| {
                        ui.label("height");
                        ui.add(
                            egui::DragValue::new(&mut camera.height)
                                .speed(0.5)
                                .range(0.1..=1000.0),
                        );
                    });
                }
                if reset_it {
                    entity.camera = Some(Camera2D { height: 12.0 });
                }
                if drop_it {
                    entity.camera = None;
                }
            }
            if entity.collider.is_some() {
                let mut drop_it = false;
                let mut reset_it = false;
                if component_header(ui, ("collider", entity.id.0), "Collider", |ui| {
                    if ui.button("Reset").clicked() {
                        reset_it = true;
                    }
                    if ui.button("Remove").clicked() {
                        drop_it = true;
                    }
                }) && let Some(collider) = entity.collider.as_mut()
                {
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
                }
                if reset_it {
                    entity.collider = Some(Collider2D {
                        hx: 0.5,
                        hy: 0.5,
                        sensor: false,
                    });
                }
                if drop_it {
                    entity.collider = None;
                }
            }
            if entity.rigidbody.is_some() {
                let mut drop_it = false;
                let mut reset_it = false;
                if component_header(ui, ("rigidbody", entity.id.0), "RigidBody", |ui| {
                    if ui.button("Reset").clicked() {
                        reset_it = true;
                    }
                    if ui.button("Remove").clicked() {
                        drop_it = true;
                    }
                }) && let Some(body) = entity.rigidbody.as_mut()
                {
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
                }
                if reset_it {
                    entity.rigidbody = Some(RigidBody2D {
                        kind: BodyKind::Dynamic,
                        velocity: Vec2::zero(),
                        gravity_scale: 1.0,
                    });
                }
                if drop_it {
                    entity.rigidbody = None;
                }
            }
            if entity.anim.is_some() {
                let mut drop_it = false;
                let mut reset_it = false;
                if component_header(ui, ("anim", entity.id.0), "Anim", |ui| {
                    if ui.button("Reset").clicked() {
                        reset_it = true;
                    }
                    if ui.button("Remove").clicked() {
                        drop_it = true;
                    }
                }) && let Some(anim) = entity.anim.as_mut()
                {
                    ui.horizontal(|ui| {
                        ui.label("clip");
                        let current = anim.current.clone();
                        egui::ComboBox::from_label("")
                            .selected_text(current.clone())
                            .show_ui(ui, |ui| {
                                for clip in &anim.clips {
                                    ui.selectable_value(
                                        &mut anim.current,
                                        clip.name.clone(),
                                        clip.name.clone(),
                                    );
                                }
                            });
                    });
                    if let Some(clip) = anim
                        .clips
                        .iter_mut()
                        .find(|c| c.name == anim.current.clone())
                    {
                        ui.horizontal(|ui| {
                            ui.label("fps");
                            ui.add(
                                egui::DragValue::new(&mut clip.fps)
                                    .speed(0.5)
                                    .range(0.0..=60.0),
                            );
                            ui.checkbox(&mut clip.looped, "loop");
                        });
                        let mut drop_frame: Option<usize> = None;
                        for (i, frame) in clip.frames.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(format!("f{i}"));
                                ui.label(frame.clone());
                                if ui.button("x").clicked() {
                                    drop_frame = Some(i);
                                }
                            });
                        }
                        if let Some(i) = drop_frame
                            && i < clip.frames.len()
                        {
                            clip.frames.remove(i);
                        }
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.anim_frame_path)
                                .on_hover_text("assets/frame.png");
                            if ui.button("+ Frame").clicked() && !self.anim_frame_path.is_empty() {
                                clip.frames.push(std::mem::take(&mut self.anim_frame_path));
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.anim_clip_name)
                            .on_hover_text("clip name");
                        if ui.button("+ Clip").clicked() && !self.anim_clip_name.is_empty() {
                            let name = std::mem::take(&mut self.anim_clip_name);
                            if !anim.clips.iter().any(|c| c.name == name) {
                                anim.clips.push(ember_scene::AnimClip {
                                    name: name.clone(),
                                    frames: Vec::new(),
                                    fps: 8.0,
                                    looped: true,
                                });
                            }
                            anim.current = name;
                        }
                        if ui.button("- Clip").clicked() {
                            let cur = anim.current.clone();
                            anim.clips.retain(|c| c.name != cur);
                            anim.current = anim
                                .clips
                                .first()
                                .map(|c| c.name.clone())
                                .unwrap_or_default();
                        }
                    });
                }
                if reset_it {
                    entity.anim = Some(ember_scene::SpriteAnim {
                        clips: vec![ember_scene::AnimClip {
                            name: "idle".to_string(),
                            frames: Vec::new(),
                            fps: 8.0,
                            looped: false,
                        }],
                        current: "idle".to_string(),
                        t: 0.0,
                    });
                }
                if drop_it {
                    entity.anim = None;
                }
            }
            let mut note: Option<String> = None;
            let mut live: Vec<(EntityId, String, String, ScriptValue)> = Vec::new();
            for (script, rows_result) in &rows_list {
                let title = format!("{} (Script)", script_stem(script));
                let mut drop_it = false;
                if component_header(ui, (script.as_str(), entity.id.0), &title, |ui| {
                    if ui.button("Unlink").clicked() {
                        drop_it = true;
                    }
                }) {
                    match rows_result {
                        Ok(rows) => {
                            let mut edits: HashMap<String, ScriptValue> = HashMap::new();
                            let mut reverts: Vec<String> = Vec::new();
                            for row in rows {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{} ({})", row.name, row.ty));
                                    let mut value = row.value.clone();
                                    let changed = match &mut value {
                                        ScriptValue::I32(v) => {
                                            ui.add(egui::DragValue::new(v)).changed()
                                        }
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
                            if let Some(entry) =
                                entity.scripts.iter_mut().find(|s| s.path == *script)
                            {
                                for (name, value) in &edits {
                                    entry.values.insert(name.clone(), value.clone());
                                }
                                for name in &reverts {
                                    entry.values.remove(name);
                                }
                            }
                            let live_rows: Vec<(String, ScriptValue)> = rows
                                .iter()
                                .map(|row| {
                                    let value = if reverts.contains(&row.name) {
                                        row.default.clone()
                                    } else {
                                        entity
                                            .scripts
                                            .iter()
                                            .find(|s| s.path == *script)
                                            .and_then(|s| s.values.get(&row.name).cloned())
                                            .unwrap_or(row.value.clone())
                                    };
                                    (row.name.clone(), value)
                                })
                                .collect();
                            let entity_id = entity.id;
                            for (name, value) in live_rows {
                                live.push((entity_id, script.clone(), name, value));
                            }
                        }
                        Err(message) => {
                            ui.label(format!("script: {message}"));
                        }
                    }
                }
                if drop_it {
                    entity.scripts.retain(|s| s.path != *script);
                }
            }
            ui.separator();
            ui.vertical_centered(|ui| {
                if ui.button("Add Component").clicked() {
                    self.add_menu_open = !self.add_menu_open;
                }
            });
            if self.add_menu_open {
                let mut want_close = false;
                ui.group(|ui| {
                    ui.text_edit_singleline(&mut self.comp_search)
                        .on_hover_text("filter");
                    let query = self.comp_search.to_lowercase();
                    let matches =
                        |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
                    ui.label("Components");
                    if entity.sprite.is_none() && matches("sprite") && ui.button("Sprite").clicked()
                    {
                        entity.sprite = Some(SpriteRenderer {
                            texture: String::new(),
                            tint: [1.0, 1.0, 1.0, 1.0],
                            sorting_order: 0,
                        });
                        want_close = true;
                    }
                    if entity.camera.is_none() && matches("camera") && ui.button("Camera").clicked()
                    {
                        entity.camera = Some(Camera2D { height: 12.0 });
                        want_close = true;
                    }
                    if entity.collider.is_none()
                        && matches("collider")
                        && ui.button("Collider").clicked()
                    {
                        entity.collider = Some(Collider2D {
                            hx: 0.5,
                            hy: 0.5,
                            sensor: false,
                        });
                        want_close = true;
                    }
                    if entity.rigidbody.is_none()
                        && matches("rigidbody")
                        && ui.button("RigidBody").clicked()
                    {
                        entity.rigidbody = Some(RigidBody2D {
                            kind: BodyKind::Dynamic,
                            velocity: Vec2::zero(),
                            gravity_scale: 1.0,
                        });
                        want_close = true;
                    }
                    if entity.anim.is_none() && matches("anim") && ui.button("Anim").clicked() {
                        entity.anim = Some(ember_scene::SpriteAnim {
                            clips: vec![ember_scene::AnimClip {
                                name: "idle".to_string(),
                                frames: Vec::new(),
                                fps: 8.0,
                                looped: false,
                            }],
                            current: "idle".to_string(),
                            t: 0.0,
                        });
                        want_close = true;
                    }
                    ui.separator();
                    ui.label("Scripts");
                    for script in self.scripts.clone() {
                        if entity.scripts.iter().any(|s| s.path == script) || !matches(&script) {
                            continue;
                        }
                        if ui.button(&script).clicked() {
                            entity.scripts.push(ember_scene::ScriptEntry {
                                path: script.clone(),
                                values: HashMap::new(),
                            });
                            want_close = true;
                        }
                    }
                });
                if want_close {
                    self.add_menu_open = false;
                }
            }
            if playing && let Some(play) = &mut self.play {
                for (entity_id, file, name, value) in &live {
                    let runtime_value = match value {
                        ScriptValue::I32(v) => Value::I32(*v),
                        ScriptValue::F32(v) => Value::F32(*v),
                        ScriptValue::Bool(v) => Value::Bool(*v),
                        ScriptValue::Str(v) => Value::Str(v.clone()),
                    };
                    if let Err(e) = play
                        .runtime
                        .set_global(*entity_id, file, name, runtime_value)
                    {
                        note = Some(format!("error: {e}"));
                    }
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
            let (rect, resp) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
            let aspect = off.aspect();
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
                let world = screen_to_world(local, (rect.width(), rect.height()), &camera, aspect);
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
                    let ctrl = ui.ctx().input(|i| i.modifiers.ctrl);
                    if ctrl {
                        self.toggle_select(*id);
                    } else {
                        self.select_only(*id);
                    }
                }
            }
            if resp.drag_started() && !self.selection.is_empty() && self.play.is_none() {
                Self::push_undo(&mut self.undo, &mut self.redo, &self.scene);
            }
            if resp.dragged()
                && !self.selection.is_empty()
                && let Some(cur) = resp.interact_pointer_pos()
            {
                let size = (rect.width(), rect.height());
                let prev = egui::pos2(cur.x - resp.drag_delta()[0], cur.y - resp.drag_delta()[1]);
                let a = screen_to_world(
                    (cur.x - rect.min.x, cur.y - rect.min.y),
                    size,
                    &camera,
                    aspect,
                );
                let b = screen_to_world(
                    (prev.x - rect.min.x, prev.y - rect.min.y),
                    size,
                    &camera,
                    aspect,
                );
                self.move_selection(a.x - b.x, a.y - b.y);
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
                for (key, bit) in [
                    (egui::Key::ArrowUp, 0),
                    (egui::Key::ArrowDown, 1),
                    (egui::Key::ArrowLeft, 2),
                    (egui::Key::ArrowRight, 3),
                    (egui::Key::Space, 4),
                ] {
                    if i.key_down(key) {
                        input.press_down(bit);
                    }
                    if i.key_pressed(key) {
                        input.press(bit);
                    }
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
                self.tick_play(&mut input, dt);
                let title = match &mut self.play {
                    Some(play) => play.runtime.take_title(),
                    None => None,
                };
                if let Some(title) = title {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Title(title));
                    self.title_restore = true;
                }
            }
        }
        if self.title_restore && self.play.is_none() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title("Ember".to_string()));
            self.title_restore = false;
        }
        if self.play.is_none() {
            let (undo_key, redo_key) = ui.ctx().input(|i| {
                (
                    i.key_pressed(egui::Key::Z) && i.modifiers.ctrl && !i.modifiers.shift,
                    (i.key_pressed(egui::Key::Y) && i.modifiers.ctrl)
                        || (i.key_pressed(egui::Key::Z) && i.modifiers.ctrl && i.modifiers.shift),
                )
            });
            if undo_key {
                self.undo();
            } else if redo_key {
                self.redo();
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
