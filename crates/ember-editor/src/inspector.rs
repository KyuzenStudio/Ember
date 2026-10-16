use std::collections::HashMap;

use eframe::egui;
use ember_core::Vec2;
use ember_scene::{
    AnimClip, BodyKind, Camera2D, Collider2D, Entity, EntityId, RigidBody2D, ScriptValue,
    SpriteAnim, SpriteRenderer,
};

use super::project::script_stem;
use super::state::VarRow;
use super::ui::component_header;

pub(crate) fn section_transform(ui: &mut egui::Ui, entity: &mut Entity) {
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
}

pub(crate) fn section_sprite(ui: &mut egui::Ui, entity: &mut Entity) {
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
}

pub(crate) fn section_camera(ui: &mut egui::Ui, entity: &mut Entity) {
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
}

pub(crate) fn section_collider(ui: &mut egui::Ui, entity: &mut Entity) {
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
}

pub(crate) fn section_rigidbody(ui: &mut egui::Ui, entity: &mut Entity) {
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
}

pub(crate) fn section_anim(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    clip_name: &mut String,
    frame_path: &mut String,
) {
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
            anim_current_clip(ui, anim, frame_path);
            anim_clip_buttons(ui, anim, clip_name);
        }
        if reset_it {
            entity.anim = Some(SpriteAnim {
                clips: vec![AnimClip {
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
}

fn anim_current_clip(ui: &mut egui::Ui, anim: &mut SpriteAnim, frame_path: &mut String) {
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
            ui.text_edit_singleline(frame_path)
                .on_hover_text("assets/frame.png");
            if ui.button("+ Frame").clicked() && !frame_path.is_empty() {
                clip.frames.push(std::mem::take(frame_path));
            }
        });
    }
}

fn anim_clip_buttons(ui: &mut egui::Ui, anim: &mut SpriteAnim, clip_name: &mut String) {
    ui.horizontal(|ui| {
        ui.text_edit_singleline(clip_name)
            .on_hover_text("clip name");
        if ui.button("+ Clip").clicked() && !clip_name.is_empty() {
            let name = std::mem::take(clip_name);
            if !anim.clips.iter().any(|c| c.name == name) {
                anim.clips.push(AnimClip {
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

pub(crate) fn section_scripts(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    rows_list: &[(String, Result<Vec<VarRow>, String>)],
) -> Vec<(EntityId, String, String, ScriptValue)> {
    let mut live: Vec<(EntityId, String, String, ScriptValue)> = Vec::new();
    for (script, rows_result) in rows_list {
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
                    if let Some(entry) = entity.scripts.iter_mut().find(|s| s.path == *script) {
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
    live
}

pub(crate) fn section_add_component(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    add_menu_open: &mut bool,
    comp_search: &mut String,
    scripts: &[String],
) {
    use ember_scene::ScriptEntry;

    ui.separator();
    ui.vertical_centered(|ui| {
        if ui.button("Add Component").clicked() {
            *add_menu_open = !*add_menu_open;
        }
    });
    if *add_menu_open {
        let mut want_close = false;
        ui.group(|ui| {
            ui.text_edit_singleline(comp_search).on_hover_text("filter");
            let query = comp_search.to_lowercase();
            let matches = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
            ui.label("Components");
            if entity.sprite.is_none() && matches("sprite") && ui.button("Sprite").clicked() {
                entity.sprite = Some(SpriteRenderer {
                    texture: String::new(),
                    tint: [1.0, 1.0, 1.0, 1.0],
                    sorting_order: 0,
                });
                want_close = true;
            }
            if entity.camera.is_none() && matches("camera") && ui.button("Camera").clicked() {
                entity.camera = Some(Camera2D { height: 12.0 });
                want_close = true;
            }
            if entity.collider.is_none() && matches("collider") && ui.button("Collider").clicked() {
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
                entity.anim = Some(SpriteAnim {
                    clips: vec![AnimClip {
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
            for script in scripts {
                if entity.scripts.iter().any(|s| s.path == *script) || !matches(script) {
                    continue;
                }
                if ui.button(script).clicked() {
                    entity.scripts.push(ScriptEntry {
                        path: script.clone(),
                        values: HashMap::new(),
                    });
                    want_close = true;
                }
            }
        });
        if want_close {
            *add_menu_open = false;
        }
    }
}
