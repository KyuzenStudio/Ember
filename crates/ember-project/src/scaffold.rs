use std::path::Path;

use ember_scene::{Camera2D, Scene, ScriptEntry, SpriteRenderer};

use crate::error::ProjectError;
use crate::model::{
    LoadedProject, PROJECT_FILE_NAME, PROJECT_FORMAT_VERSION, Project, STARTER_SCRIPT,
    STARTER_SPRITE, STARTUP_SCENE,
};
use crate::validate::{validate_asset_path, validate_project_name, write_file};

const STARTER_PNG: &[u8] = include_bytes!("../../../assets/sprite-demo.png");
const STARTER_SCRIPT_TEXT: &str = r#"var speed: f32 = 6.0

fn on_create() {
    log("Player created")
}

fn on_update(dt: f32) {
    if input.down(Key.Right) {
        transform.move(speed * dt, 0.0)
    }
}
"#;

fn starter_scene(name: &str) -> Scene {
    let mut scene = Scene::new(name);
    let player = scene.spawn("Player");
    if let Some(entity) = scene.entities.iter_mut().find(|e| e.id == player) {
        entity.sprite = Some(SpriteRenderer {
            texture: STARTER_SPRITE.to_string(),
            tint: [1.0, 1.0, 1.0, 1.0],
            sorting_order: 0,
        });
        entity.scripts = vec![ScriptEntry {
            path: STARTER_SCRIPT.to_string(),
            values: std::collections::HashMap::new(),
        }];
    }
    let camera = scene.spawn("MainCamera");
    if let Some(entity) = scene.entities.iter_mut().find(|e| e.id == camera) {
        entity.camera = Some(Camera2D { height: 12.0 });
    }
    scene
}

pub fn create_project(dir: &Path, name: &str) -> Result<(), ProjectError> {
    validate_project_name(name)?;
    if dir.exists() {
        return Err(ProjectError::new(format!(
            "directory '{}' already exists",
            dir.display()
        )));
    }
    for sub in ["scenes", "scripts", "assets"] {
        std::fs::create_dir_all(dir.join(sub))
            .map_err(|e| ProjectError::new(format!("create '{}': {e}", dir.join(sub).display())))?;
    }
    let project = Project {
        format_version: PROJECT_FORMAT_VERSION,
        name: name.to_string(),
        startup_scene: STARTUP_SCENE.to_string(),
    };
    let project_text = ron::ser::to_string_pretty(&project, ron::ser::PrettyConfig::new())
        .map_err(|e| ProjectError::new(format!("serialize project '{name}': {e}")))?;
    write_file(&dir.join(PROJECT_FILE_NAME), project_text.as_bytes())?;
    let scene_text = starter_scene(name)
        .save_to_string()
        .map_err(|e| ProjectError::new(format!("serialize starter scene: {e}")))?;
    write_file(&dir.join(STARTUP_SCENE), scene_text.as_bytes())?;
    write_file(&dir.join(STARTER_SPRITE), STARTER_PNG)?;
    write_file(&dir.join(STARTER_SCRIPT), STARTER_SCRIPT_TEXT.as_bytes())?;
    let readme =
        format!("# {name}\n\nCreated with `ember new`. Open this folder with the Ember editor.\n");
    write_file(&dir.join("README.md"), readme.as_bytes())?;
    Ok(())
}

pub fn load_project(dir: &Path) -> Result<LoadedProject, ProjectError> {
    load_project_opts(dir, true)
}

pub fn load_project_opts(
    dir: &Path,
    check_script_sources: bool,
) -> Result<LoadedProject, ProjectError> {
    let file = dir.join(PROJECT_FILE_NAME);
    let src = std::fs::read_to_string(&file)
        .map_err(|e| ProjectError::new(format!("read '{}': {e}", file.display())))?;
    let project: Project = ron::from_str(&src)
        .map_err(|e| ProjectError::new(format!("parse '{}': {e}", file.display())))?;
    if project.format_version != PROJECT_FORMAT_VERSION {
        return Err(ProjectError::new(format!(
            "parse '{}': unsupported project version {}, expected {PROJECT_FORMAT_VERSION}",
            file.display(),
            project.format_version
        )));
    }
    let scene_path = validate_asset_path(dir, &project.startup_scene).map_err(|e| {
        ProjectError::new(format!("startup scene '{}': {e}", project.startup_scene))
    })?;
    let scene = Scene::load_from_file(&scene_path)
        .map_err(|e| ProjectError::new(format!("startup scene: {e}")))?;
    for entity in &scene.entities {
        if let Some(sprite) = &entity.sprite {
            validate_asset_path(dir, &sprite.texture)
                .map_err(|e| ProjectError::new(format!("entity '{}': {e}", entity.name)))?;
        }
        for entry in &entity.scripts {
            let path = validate_asset_path(dir, &entry.path)
                .map_err(|e| ProjectError::new(format!("entity '{}': {e}", entity.name)))?;
            if check_script_sources && !path.is_file() {
                return Err(ProjectError::new(format!(
                    "entity '{}': script '{}' not found",
                    entity.name, entry.path
                )));
            }
        }
    }
    Ok(LoadedProject {
        project,
        dir: dir.to_path_buf(),
        scene,
    })
}
