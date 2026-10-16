use std::path::{Path, PathBuf};

use crate::assets::copy_dir;
use crate::check::{check_dir, script_stems};
use crate::error::ProjectError;
use crate::model::PROJECT_FILE_NAME;
use crate::scaffold::load_project;
use crate::validate::write_file;

#[derive(Debug)]
pub struct BuildReport {
    pub scripts: usize,
}

pub fn build_project(dir: &Path) -> Result<BuildReport, ProjectError> {
    load_project(dir)?;
    let report = check_dir(dir)?;
    if !report.errors.is_empty() {
        let mut message = String::new();
        for e in &report.errors {
            message.push_str(&format!("{e}\n"));
        }
        return Err(ProjectError::new(message));
    }
    let build = dir.join("build");
    std::fs::create_dir_all(&build)
        .map_err(|e| ProjectError::new(format!("create '{}': {e}", build.display())))?;
    for stem in script_stems(dir)? {
        let src = std::fs::read_to_string(dir.join("scripts").join(format!("{stem}.em")))
            .map_err(|e| ProjectError::new(format!("read script '{stem}.em': {e}")))?;
        let chunk = ember_lang::compile::compile(
            &dir.join("scripts")
                .join(format!("{stem}.em"))
                .display()
                .to_string(),
            &src,
        )
        .map_err(|errors| {
            let mut message = String::new();
            for e in &errors {
                message.push_str(&format!("{e}\n"));
            }
            ProjectError::new(message)
        })?;
        ember_vm::verify(&chunk)
            .map_err(|e| ProjectError::new(format!("verify '{stem}.em': {e}")))?;
        let bytes = ember_vm::encode(&chunk);
        write_file(&build.join(format!("{stem}.embc")), &bytes)?;
    }
    Ok(BuildReport {
        scripts: report.checked,
    })
}

#[derive(Debug)]
pub struct PackageReport {
    pub dir: PathBuf,
    pub files: Vec<String>,
}

pub fn package_project(dir: &Path, player_exe: &Path) -> Result<PackageReport, ProjectError> {
    let loaded = load_project(dir)?;
    build_project(dir)?;
    if !player_exe.is_file() {
        return Err(ProjectError::new(format!(
            "player '{}' not found; build it first",
            player_exe.display()
        )));
    }
    let out = dir
        .join("dist")
        .join(format!("{}-windows-x86_64", loaded.project.name));
    if out.exists() {
        std::fs::remove_dir_all(&out)
            .map_err(|e| ProjectError::new(format!("clean '{}': {e}", out.display())))?;
    }
    std::fs::create_dir_all(&out)
        .map_err(|e| ProjectError::new(format!("create '{}': {e}", out.display())))?;
    let exe_name = player_exe
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "ember-player.exe".to_string());
    std::fs::copy(player_exe, out.join(&exe_name))
        .map_err(|e| ProjectError::new(format!("copy player: {e}")))?;
    write_file(
        &out.join(PROJECT_FILE_NAME),
        std::fs::read(dir.join(PROJECT_FILE_NAME))
            .map_err(|e| ProjectError::new(format!("read project: {e}")))?
            .as_slice(),
    )?;
    copy_dir(&dir.join("scenes"), &out.join("scenes"))?;
    copy_dir(&dir.join("build"), &out.join("build"))?;
    copy_dir(&dir.join("assets"), &out.join("assets"))?;
    let mut files = vec![exe_name, PROJECT_FILE_NAME.to_string()];
    let mut stack = vec![out.join("scenes"), out.join("build"), out.join("assets")];
    while let Some(top) = stack.pop() {
        let entries = std::fs::read_dir(&top)
            .map_err(|e| ProjectError::new(format!("read '{}': {e}", top.display())))?;
        for entry in entries {
            let entry =
                entry.map_err(|e| ProjectError::new(format!("read '{}': {e}", top.display())))?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(rel) = path.strip_prefix(&out) {
                files.push(rel.display().to_string());
            }
        }
    }
    files.sort();
    Ok(PackageReport { dir: out, files })
}
