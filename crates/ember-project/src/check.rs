use std::path::Path;

use crate::error::ProjectError;
use crate::scaffold::load_project;

#[derive(Debug)]
pub struct CheckReport {
    pub checked: usize,
    pub errors: Vec<ember_lang::Error>,
}

pub fn check_dir(dir: &Path) -> Result<CheckReport, ProjectError> {
    load_project(dir)?;
    let scripts = dir.join("scripts");
    let mut names: Vec<String> = Vec::new();
    if scripts.is_dir() {
        let entries = std::fs::read_dir(&scripts)
            .map_err(|e| ProjectError::new(format!("read '{}': {e}", scripts.display())))?;
        for entry in entries {
            let entry = entry
                .map_err(|e| ProjectError::new(format!("read '{}': {e}", scripts.display())))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "em") {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    names.sort();
    let mut errors = Vec::new();
    for name in &names {
        let path = scripts.join(name);
        match std::fs::read_to_string(&path) {
            Ok(src) => {
                if let Err(mut file_errors) =
                    ember_lang::semantic::check(&path.display().to_string(), &src)
                {
                    errors.append(&mut file_errors);
                }
            }
            Err(e) => errors.push(ember_lang::Error {
                file: path.display().to_string(),
                line: 1,
                col: 1,
                message: format!("read '{}': {e}", path.display()),
                span: ember_lang::Span::empty(0),
            }),
        }
    }
    Ok(CheckReport {
        checked: names.len(),
        errors,
    })
}

pub fn script_stems(dir: &Path) -> Result<Vec<String>, ProjectError> {
    let scripts = dir.join("scripts");
    let mut names: Vec<String> = Vec::new();
    if scripts.is_dir() {
        let entries = std::fs::read_dir(&scripts)
            .map_err(|e| ProjectError::new(format!("read '{}': {e}", scripts.display())))?;
        for entry in entries {
            let entry = entry
                .map_err(|e| ProjectError::new(format!("read '{}': {e}", scripts.display())))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "em")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}
