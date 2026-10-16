use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use eframe::egui;
use eframe::egui::containers::{CentralPanel, Panel};
use ember_core::Vec2;
use ember_renderer::{Camera, Offscreen, load_png_file, pick_top, scene_sprites};
use ember_scene::{Camera2D, EntityId, Scene, SpriteRenderer};

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
    new_name: String,
    rename: String,
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
            new_name: "Entity".to_string(),
            rename: String::new(),
        };
        app.reload();
        app
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
                ui.label(&self.status);
            });
        });
    }

    fn hierarchy(&mut self, ui: &mut egui::Ui) {
        Panel::left("hierarchy").show(ui, |ui| {
            ui.heading("Hierarchy");
            let ids: Vec<(EntityId, String)> = self
                .scene
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
                    self.script_buf = self
                        .scene
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
                    let id = self.scene.spawn(&self.new_name.clone());
                    self.selection = Some(id);
                }
            });
            if self
                .selection
                .is_some_and(|id| self.scene.entities.iter().any(|e| e.id == id))
                && ui.button("Delete").clicked()
                && let Some(id) = self.selection
            {
                self.scene.entities.retain(|e| e.id != id);
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

    fn inspector(&mut self, ui: &mut egui::Ui) {
        Panel::right("inspector").show(ui, |ui| {
            ui.heading("Inspector");
            let selection = self.selection;
            let Some(entity) =
                selection.and_then(|id| self.scene.entities.iter_mut().find(|e| e.id == id))
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
                    &self.scene,
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
            let (camera, draws) = scene_sprites(&self.scene, &self.sizes);
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
                let mut order: Vec<(i32, EntityId)> = self
                    .scene
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
}
