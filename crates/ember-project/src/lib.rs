use std::fmt;
use std::path::{Component, Path, PathBuf};

use ember_scene::{Camera2D, Scene, SpriteRenderer};
use serde::{Deserialize, Serialize};

pub const PROJECT_FORMAT_VERSION: u32 = 1;
pub const PROJECT_FILE_NAME: &str = "ember.project";
pub const STARTUP_SCENE: &str = "scenes/main.ron";
pub const STARTER_SPRITE: &str = "assets/player.png";
pub const STARTER_SCRIPT: &str = "scripts/player.em";

const STARTER_PNG: &[u8] = include_bytes!("../../../assets/sprite-demo.png");
const STARTER_SCRIPT_TEXT: &str = r#"var speed: f32 = 220.0

fn on_create() {
    log("Player created")
}

fn on_update(dt: f32) {
    if input.down(Key.Right) {
        transform.move(speed * dt, 0.0)
    }
}
"#;

#[derive(Debug, Clone)]
pub struct ProjectError(String);

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ProjectError {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub format_version: u32,
    pub name: String,
    pub startup_scene: String,
}

#[derive(Debug)]
pub struct LoadedProject {
    pub project: Project,
    pub dir: PathBuf,
    pub scene: Scene,
}

pub fn validate_project_name(name: &str) -> Result<(), ProjectError> {
    if name.is_empty() {
        return Err(ProjectError("project name is empty".to_string()));
    }
    if name == "." || name == ".." {
        return Err(ProjectError(format!("project name '{name}' is reserved")));
    }
    if Path::new(name).is_absolute() {
        return Err(ProjectError(format!(
            "project name '{name}' must be relative"
        )));
    }
    for bad in ['/', '\\', ':', '*', '?', '"', '<', '>', '|'] {
        if name.contains(bad) {
            return Err(ProjectError(format!(
                "project name '{name}' contains '{bad}'"
            )));
        }
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(ProjectError(
            "project name contains control characters".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_asset_path(root: &Path, rel: &str) -> Result<PathBuf, ProjectError> {
    if rel.is_empty() {
        return Err(ProjectError("asset path is empty".to_string()));
    }
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return Err(ProjectError(format!("asset path '{rel}' must be relative")));
    }
    for component in rel_path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(ProjectError(format!(
                "asset path '{rel}' escapes the project root"
            )));
        }
    }
    let joined = root.join(rel_path);
    if joined.exists() {
        let base = root
            .canonicalize()
            .map_err(|e| ProjectError(format!("resolve '{}': {e}", root.display())))?;
        let target = joined
            .canonicalize()
            .map_err(|e| ProjectError(format!("resolve '{}': {e}", joined.display())))?;
        if !target.starts_with(&base) {
            return Err(ProjectError(format!(
                "asset path '{rel}' escapes the project root"
            )));
        }
    }
    Ok(joined)
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), ProjectError> {
    std::fs::write(path, bytes)
        .map_err(|e| ProjectError(format!("write '{}': {e}", path.display())))
}

fn starter_scene(name: &str) -> Scene {
    let mut scene = Scene::new(name);
    let player = scene.spawn("Player");
    if let Some(entity) = scene.entities.iter_mut().find(|e| e.id == player) {
        entity.sprite = Some(SpriteRenderer {
            texture: STARTER_SPRITE.to_string(),
            tint: [1.0, 1.0, 1.0, 1.0],
            sorting_order: 0,
        });
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
        return Err(ProjectError(format!(
            "directory '{}' already exists",
            dir.display()
        )));
    }
    for sub in ["scenes", "scripts", "assets"] {
        std::fs::create_dir_all(dir.join(sub))
            .map_err(|e| ProjectError(format!("create '{}': {e}", dir.join(sub).display())))?;
    }
    let project = Project {
        format_version: PROJECT_FORMAT_VERSION,
        name: name.to_string(),
        startup_scene: STARTUP_SCENE.to_string(),
    };
    let project_text = ron::ser::to_string_pretty(&project, ron::ser::PrettyConfig::new())
        .map_err(|e| ProjectError(format!("serialize project '{name}': {e}")))?;
    write_file(&dir.join(PROJECT_FILE_NAME), project_text.as_bytes())?;
    let scene_text = starter_scene(name)
        .save_to_string()
        .map_err(|e| ProjectError(format!("serialize starter scene: {e}")))?;
    write_file(&dir.join(STARTUP_SCENE), scene_text.as_bytes())?;
    write_file(&dir.join(STARTER_SPRITE), STARTER_PNG)?;
    write_file(&dir.join(STARTER_SCRIPT), STARTER_SCRIPT_TEXT.as_bytes())?;
    let readme =
        format!("# {name}\n\nCreated with `ember new`. Open this folder with the Ember editor.\n");
    write_file(&dir.join("README.md"), readme.as_bytes())?;
    Ok(())
}

pub fn load_project(dir: &Path) -> Result<LoadedProject, ProjectError> {
    let file = dir.join(PROJECT_FILE_NAME);
    let src = std::fs::read_to_string(&file)
        .map_err(|e| ProjectError(format!("read '{}': {e}", file.display())))?;
    let project: Project = ron::from_str(&src)
        .map_err(|e| ProjectError(format!("parse '{}': {e}", file.display())))?;
    if project.format_version != PROJECT_FORMAT_VERSION {
        return Err(ProjectError(format!(
            "parse '{}': unsupported project version {}, expected {PROJECT_FORMAT_VERSION}",
            file.display(),
            project.format_version
        )));
    }
    let scene_path = validate_asset_path(dir, &project.startup_scene)
        .map_err(|e| ProjectError(format!("startup scene '{}': {e}", project.startup_scene)))?;
    let scene = Scene::load_from_file(&scene_path)
        .map_err(|e| ProjectError(format!("startup scene: {e}")))?;
    for entity in &scene.entities {
        if let Some(sprite) = &entity.sprite {
            validate_asset_path(dir, &sprite.texture)
                .map_err(|e| ProjectError(format!("entity '{}': {e}", entity.name)))?;
        }
    }
    Ok(LoadedProject {
        project,
        dir: dir.to_path_buf(),
        scene,
    })
}

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
            .map_err(|e| ProjectError(format!("read '{}': {e}", scripts.display())))?;
        for entry in entries {
            let entry =
                entry.map_err(|e| ProjectError(format!("read '{}': {e}", scripts.display())))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn fresh_dir(tag: &str) -> PathBuf {
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("ember-proj-{tag}-{}-{id}", std::process::id()))
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn create_and_load_valid_project() {
        let dir = fresh_dir("valid");
        create_project(&dir, "MyGame").unwrap();
        for rel in [
            PROJECT_FILE_NAME,
            STARTUP_SCENE,
            STARTER_SPRITE,
            STARTER_SCRIPT,
            "README.md",
        ] {
            assert!(dir.join(rel).is_file(), "missing {rel}");
        }
        let loaded = load_project(&dir).unwrap();
        assert_eq!(loaded.project.name, "MyGame");
        assert_eq!(loaded.project.format_version, PROJECT_FORMAT_VERSION);
        assert_eq!(loaded.scene.entities.len(), 2);
        assert!(loaded.scene.entities.iter().any(|e| e.name == "Player"));
        cleanup(&dir);
    }

    #[test]
    fn create_twice_fails() {
        let dir = fresh_dir("twice");
        create_project(&dir, "Dup").unwrap();
        let err = create_project(&dir, "Dup").unwrap_err();
        assert!(err.to_string().contains("already exists"));
        cleanup(&dir);
    }

    #[test]
    fn bad_names_rejected() {
        for bad in ["", ".", "..", "a/b", "a\\b", "C:\\x", "/abs", "a:b", "a*b"] {
            let dir = fresh_dir("badname");
            let err = create_project(&dir, bad).unwrap_err();
            assert!(!err.to_string().is_empty(), "name '{bad}' accepted");
            assert!(!dir.exists(), "name '{bad}' created output");
        }
    }

    #[test]
    fn asset_path_accepts_inside() {
        let root = Path::new("/game");
        assert_eq!(
            validate_asset_path(root, "assets/player.png").unwrap(),
            root.join("assets/player.png")
        );
    }

    #[test]
    fn asset_path_rejects_escape() {
        let root = Path::new("/game");
        for bad in ["", "/abs.png", "../x.png", "a/../../y.png", "./x.png"] {
            assert!(
                validate_asset_path(root, bad).is_err(),
                "path '{bad}' accepted"
            );
        }
    }

    #[test]
    fn rejects_unknown_project_version() {
        let dir = fresh_dir("version");
        create_project(&dir, "Old").unwrap();
        let file = dir.join(PROJECT_FILE_NAME);
        let mut project: Project = ron::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        project.format_version = 999;
        let text = ron::ser::to_string_pretty(&project, ron::ser::PrettyConfig::new()).unwrap();
        std::fs::write(&file, text).unwrap();
        let err = load_project(&dir).unwrap_err();
        assert!(err.to_string().contains("999"));
        cleanup(&dir);
    }

    #[test]
    fn check_dir_valid() {
        let dir = fresh_dir("checkvalid");
        create_project(&dir, "Checked").unwrap();
        let report = check_dir(&dir).unwrap();
        assert_eq!(report.checked, 1);
        assert!(report.errors.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn check_dir_broken_script() {
        let dir = fresh_dir("checkbroken");
        create_project(&dir, "Broken").unwrap();
        std::fs::write(dir.join(STARTER_SCRIPT), "var speed = \n").unwrap();
        let report = check_dir(&dir).unwrap();
        assert_eq!(report.checked, 1);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].to_string().contains("player.em:1:"));
        cleanup(&dir);
    }

    #[test]
    fn check_dir_missing_project() {
        let dir = fresh_dir("checkmissing");
        let err = check_dir(&dir).unwrap_err();
        assert!(err.to_string().contains(PROJECT_FILE_NAME));
    }

    #[test]
    fn missing_project_file_mentions_path() {
        let dir = fresh_dir("missing");
        let err = load_project(&dir).unwrap_err();
        assert!(err.to_string().contains(PROJECT_FILE_NAME));
    }

    #[test]
    fn bad_sprite_ref_rejected() {
        let dir = fresh_dir("badsprite");
        create_project(&dir, "Evil").unwrap();
        let scene_path = dir.join(STARTUP_SCENE);
        let mut scene = Scene::load_from_file(&scene_path).unwrap();
        let first = scene.entities[0].id;
        let entity = scene.entities.iter_mut().find(|e| e.id == first).unwrap();
        entity.sprite = Some(SpriteRenderer {
            texture: "../evil.png".to_string(),
            tint: [1.0, 1.0, 1.0, 1.0],
            sorting_order: 0,
        });
        scene.save_to_file(&scene_path).unwrap();
        let err = load_project(&dir).unwrap_err();
        assert!(err.to_string().contains("escapes"));
        cleanup(&dir);
    }
}
