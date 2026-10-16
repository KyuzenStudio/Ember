use std::path::{Component, Path, PathBuf};

use crate::error::ProjectError;

pub fn validate_project_name(name: &str) -> Result<(), ProjectError> {
    if name.is_empty() {
        return Err(ProjectError::new("project name is empty".to_string()));
    }
    if name == "." || name == ".." {
        return Err(ProjectError::new(format!(
            "project name '{name}' is reserved"
        )));
    }
    if Path::new(name).is_absolute() {
        return Err(ProjectError::new(format!(
            "project name '{name}' must be relative"
        )));
    }
    for bad in ['/', '\\', ':', '*', '?', '"', '<', '>', '|'] {
        if name.contains(bad) {
            return Err(ProjectError::new(format!(
                "project name '{name}' contains '{bad}'"
            )));
        }
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(ProjectError::new(
            "project name contains control characters".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_asset_path(root: &Path, rel: &str) -> Result<PathBuf, ProjectError> {
    if rel.is_empty() {
        return Err(ProjectError::new("asset path is empty".to_string()));
    }
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return Err(ProjectError::new(format!(
            "asset path '{rel}' must be relative"
        )));
    }
    for component in rel_path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(ProjectError::new(format!(
                "asset path '{rel}' escapes the project root"
            )));
        }
    }
    let joined = root.join(rel_path);
    if joined.exists() {
        let base = root
            .canonicalize()
            .map_err(|e| ProjectError::new(format!("resolve '{}': {e}", root.display())))?;
        let target = joined
            .canonicalize()
            .map_err(|e| ProjectError::new(format!("resolve '{}': {e}", joined.display())))?;
        if !target.starts_with(&base) {
            return Err(ProjectError::new(format!(
                "asset path '{rel}' escapes the project root"
            )));
        }
    }
    Ok(joined)
}

pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<(), ProjectError> {
    std::fs::write(path, bytes)
        .map_err(|e| ProjectError::new(format!("write '{}': {e}", path.display())))
}
