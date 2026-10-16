use std::path::Path;

use crate::error::ProjectError;
use crate::validate::write_file;

const PNG_MAGIC: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

pub fn list_assets(dir: &Path) -> Result<Vec<String>, ProjectError> {
    let assets = dir.join("assets");
    let mut names: Vec<String> = Vec::new();
    if assets.is_dir() {
        let entries = std::fs::read_dir(&assets)
            .map_err(|e| ProjectError::new(format!("read '{}': {e}", assets.display())))?;
        for entry in entries {
            let entry = entry
                .map_err(|e| ProjectError::new(format!("read '{}': {e}", assets.display())))?;
            let path = entry.path();
            if path.is_file()
                && let Some(name) = path.file_name().and_then(|s| s.to_str())
            {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

pub fn import_asset(dir: &Path, src: &Path) -> Result<String, ProjectError> {
    if !src.is_file() {
        return Err(ProjectError::new(format!(
            "source '{}' not found",
            src.display()
        )));
    }
    let name = src
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .ok_or_else(|| ProjectError::new(format!("bad asset name '{}'", src.display())))?;
    if !name.to_lowercase().ends_with(".png")
        && !name.to_lowercase().ends_with(".wav")
        && !name.to_lowercase().ends_with(".ogg")
    {
        return Err(ProjectError::new(format!(
            "asset '{name}' must be .png, .wav, or .ogg"
        )));
    }
    let rel = format!("assets/{name}");
    let dst = dir.join(&rel);
    if let (Ok(src_c), Ok(dst_c)) = (src.canonicalize(), dst.canonicalize())
        && src_c == dst_c
    {
        return Ok(rel);
    }
    let bytes = std::fs::read(src)
        .map_err(|e| ProjectError::new(format!("read '{}': {e}", src.display())))?;
    if name.to_lowercase().ends_with(".png")
        && (bytes.len() < PNG_MAGIC.len() || bytes[..PNG_MAGIC.len()] != PNG_MAGIC)
    {
        return Err(ProjectError::new(format!("asset '{name}' is not a PNG")));
    }
    write_file(&dst, &bytes)?;
    Ok(rel)
}

pub(crate) fn copy_dir(src: &Path, dst: &Path) -> Result<(), ProjectError> {
    std::fs::create_dir_all(dst)
        .map_err(|e| ProjectError::new(format!("create '{}': {e}", dst.display())))?;
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rel = dir
            .strip_prefix(src)
            .map_err(|_| ProjectError::new(format!("resolve '{}'", dir.display())))?;
        let target = dst.join(rel);
        std::fs::create_dir_all(&target)
            .map_err(|e| ProjectError::new(format!("create '{}': {e}", target.display())))?;
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| ProjectError::new(format!("read '{}': {e}", dir.display())))?;
        for entry in entries {
            let entry =
                entry.map_err(|e| ProjectError::new(format!("read '{}': {e}", dir.display())))?;
            let path = entry.path();
            if path.is_symlink() {
                continue;
            } else if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                let rel = path
                    .strip_prefix(src)
                    .map_err(|_| ProjectError::new(format!("resolve '{}'", path.display())))?;
                let target = dst.join(rel);
                std::fs::copy(&path, &target)
                    .map_err(|e| ProjectError::new(format!("copy '{}': {e}", path.display())))?;
            }
        }
    }
    Ok(())
}
