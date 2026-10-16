use std::collections::{HashMap, HashSet};
use std::path::Path;

use ember_core::Ty;
use ember_renderer::{Offscreen, load_png_file};
use ember_scene::{Scene, ScriptValue};
use ember_vm::{Value, Vm};

use super::play::NoopHost;
use super::state::{CachedVar, EditorApp, VarRow};
use super::ui::log_line;

pub(crate) fn script_stem(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .strip_suffix(".em")
        .unwrap_or(path)
}

pub(crate) fn open_external(path: &Path) -> Result<(), String> {
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
    pub(crate) fn set_scene_music(&mut self, raw: &str) -> Result<(), String> {
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

    pub(crate) fn log(&mut self, line: String) {
        self.status = line.clone();
        log_line(&mut self.console, line);
    }

    pub(crate) fn refresh_lists(&mut self) {
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

    pub(crate) fn reload(&mut self) {
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

    pub(crate) fn save(&mut self) {
        let path = self.dir.join(&self.scene_rel);
        match self.scene.save_to_file(&path) {
            Ok(()) => {
                self.log(format!("saved '{}'", self.scene_rel));
                self.check_scripts();
            }
            Err(e) => self.log(format!("error: {e}")),
        }
    }

    pub(crate) fn check_scripts(&mut self) {
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

    pub(crate) fn ensure_textures(
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

    pub(crate) fn script_globals(source: &str) -> Result<Vec<(String, Ty)>, String> {
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

    pub(crate) fn script_defaults(
        file: &str,
        source: &str,
    ) -> Result<HashMap<String, ScriptValue>, String> {
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

    pub(crate) fn script_rows(
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
}
