use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use eframe::egui::containers::{CentralPanel, Panel};
use ember_core::Vec2;
use ember_renderer::{Camera, Offscreen, pick_top, scene_sprites};
use ember_runtime::InputState as GameInput;
use ember_scene::{EntityId, Scene, ScriptValue};
use ember_vm::Value;

use super::play::physics_sig;
use super::project::open_external;
use super::state::{EditorApp, VarRow};

pub(crate) fn screen_to_world(
    click: (f32, f32),
    size: (f32, f32),
    camera: &Camera,
    aspect: f32,
) -> Vec2 {
    let half_h = camera.height / 2.0;
    let half_w = half_h * aspect;
    Vec2::new(
        camera.center.x + (click.0 / size.0 * 2.0 - 1.0) * half_w,
        camera.center.y + (1.0 - click.1 / size.1 * 2.0) * half_h,
    )
}

pub(crate) fn log_line(log: &mut Vec<String>, line: String) {
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
pub(crate) fn component_header(
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

impl EditorApp {
    pub(crate) fn toolbar(&mut self, ui: &mut egui::Ui) {
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

    pub(crate) fn hierarchy(&mut self, ui: &mut egui::Ui) {
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

    pub(crate) fn inspector(&mut self, ui: &mut egui::Ui) {
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
            super::inspector::section_transform(ui, entity);
            super::inspector::section_sprite(ui, entity);
            super::inspector::section_camera(ui, entity);
            super::inspector::section_collider(ui, entity);
            super::inspector::section_rigidbody(ui, entity);
            super::inspector::section_anim(
                ui,
                entity,
                &mut self.anim_clip_name,
                &mut self.anim_frame_path,
            );
            let mut note: Option<String> = None;
            let live = super::inspector::section_scripts(ui, entity, &rows_list);
            super::inspector::section_add_component(
                ui,
                entity,
                &mut self.add_menu_open,
                &mut self.comp_search,
                &self.scripts,
            );
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

    pub(crate) fn viewport(
        &mut self,
        ui: &mut egui::Ui,
        rs: Option<&eframe::egui_wgpu::RenderState>,
    ) {
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
