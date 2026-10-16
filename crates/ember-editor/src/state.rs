use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use eframe::egui;
use ember_core::Ty;
use ember_renderer::Offscreen;
use ember_runtime::Runtime;
use ember_scene::{EntityId, Scene, ScriptValue};

pub struct EditorApp {
    pub(crate) dir: PathBuf,
    pub(crate) project_name: String,
    pub(crate) scene: Scene,
    pub(crate) scene_rel: String,
    pub(crate) selection: Vec<EntityId>,
    pub(crate) undo: Vec<String>,
    pub(crate) redo: Vec<String>,
    pub(crate) anim_clip_name: String,
    pub(crate) anim_frame_path: String,
    pub(crate) scene_music_buf: String,
    pub(crate) status: String,
    pub(crate) console: Vec<String>,
    pub(crate) diag: Vec<String>,
    pub(crate) scripts: Vec<String>,
    pub(crate) diag_sel: Option<String>,
    pub(crate) assets: Vec<String>,
    pub(crate) import_src: String,
    pub(crate) comp_search: String,
    pub(crate) add_menu_open: bool,
    pub(crate) off: Option<Offscreen>,
    pub(crate) tex: Option<egui::TextureId>,
    pub(crate) tex_size: (u32, u32),
    pub(crate) sizes: HashMap<String, (u32, u32)>,
    pub(crate) missing: HashSet<String>,
    pub(crate) frame: u64,
    pub(crate) script_cache: HashMap<String, (std::time::SystemTime, Vec<CachedVar>)>,
    pub(crate) new_name: String,
    pub(crate) rename: String,
    pub(crate) play: Option<PlayState>,
    pub(crate) last_phys_sig: Vec<(u64, bool, bool, u8, u32, u32)>,
    pub(crate) title_restore: bool,
    pub(crate) audio: Option<ember_audio::AudioEngine>,
}

pub(crate) struct PlayState {
    pub(crate) runtime: Runtime,
    pub(crate) last: std::time::Instant,
    pub(crate) ticks: u64,
    pub(crate) sim_t: f32,
    pub(crate) last_dt: f32,
}

#[derive(Clone)]
pub(crate) struct CachedVar {
    pub(crate) name: String,
    pub(crate) ty: Ty,
    pub(crate) default: ScriptValue,
}

pub(crate) struct VarRow {
    pub(crate) name: String,
    pub(crate) ty: Ty,
    pub(crate) value: ScriptValue,
    pub(crate) default: ScriptValue,
    pub(crate) overridden: bool,
}

impl EditorApp {
    pub(crate) fn primary(&self) -> Option<EntityId> {
        self.selection.first().copied()
    }

    pub(crate) fn select_only(&mut self, id: EntityId) {
        self.selection = vec![id];
    }

    pub(crate) fn toggle_select(&mut self, id: EntityId) {
        if let Some(i) = self.selection.iter().position(|s| *s == id) {
            self.selection.remove(i);
        } else {
            self.selection.push(id);
        }
    }

    pub(crate) fn push_undo(undo: &mut Vec<String>, redo: &mut Vec<String>, scene: &Scene) {
        if let Ok(ron) = scene.save_to_string() {
            undo.push(ron);
            if undo.len() > 100 {
                undo.remove(0);
            }
            redo.clear();
        }
    }

    pub(crate) fn apply_snapshot(&mut self, ron: &str) {
        match Scene::load_from_str(ron, "undo") {
            Ok(scene) => {
                self.scene = scene;
                let valid: HashSet<EntityId> = self.scene.entities.iter().map(|e| e.id).collect();
                self.selection.retain(|id| valid.contains(id));
            }
            Err(e) => self.log(format!("error: {e}")),
        }
    }

    pub(crate) fn undo(&mut self) {
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

    pub(crate) fn redo(&mut self) {
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

    pub(crate) fn delete_selection(&mut self) {
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

    pub(crate) fn move_selection(&mut self, dx: f32, dy: f32) {
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
}
