use std::fmt;
use std::path::{Path, PathBuf};

use ember_runtime::ChunkDef;
use ember_scene::Scene;

#[derive(Debug, Clone)]
pub struct GameError(String);

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{0}", self.0)
    }
}

impl std::error::Error for GameError {}

#[derive(Debug)]
pub struct GameDef {
    pub dir: PathBuf,
    pub name: String,
    pub scene: Scene,
    pub scripts: Vec<ChunkDef>,
}

pub fn load_game(dir: &Path) -> Result<GameDef, GameError> {
    let loaded = ember_project::load_project_opts(dir, false)
        .map_err(|e| GameError(format!("load project: {e}")))?;
    let mut scripts = Vec::new();
    for entity in &loaded.scene.entities {
        for entry in &entity.scripts {
            let stem = script_stem(&entry.path).ok_or_else(|| {
                GameError(format!(
                    "entity '{}': nested scripts not supported in v0.1",
                    entity.name
                ))
            })?;
            let rel = format!("build/{stem}.embc");
            let path = dir.join(&rel);
            if !path.is_file() {
                return Err(GameError(format!(
                    "entity '{}': '{rel}' missing; run ember build first",
                    entity.name
                )));
            }
            let bytes =
                std::fs::read(&path).map_err(|e| GameError(format!("read '{rel}': {e}")))?;
            let chunk =
                ember_vm::decode(&bytes).map_err(|e| GameError(format!("decode '{rel}': {e}")))?;
            scripts.push(ChunkDef {
                entity: entity.id,
                file: rel,
                chunk,
                values: entry.values.clone(),
            });
        }
    }
    Ok(GameDef {
        dir: dir.to_path_buf(),
        name: loaded.project.name,
        scene: loaded.scene,
        scripts,
    })
}

fn script_stem(script: &str) -> Option<String> {
    let parts: Vec<&str> = script.split('/').collect();
    if parts.len() != 2 || parts[0] != "scripts" {
        return None;
    }
    let stem = parts[1].strip_suffix(".em")?;
    if stem.is_empty() || stem.contains('.') {
        return None;
    }
    Some(stem.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built_project(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ember-player-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        ember_project::create_project(&dir.join("Game"), "Game").unwrap();
        ember_project::build_project(&dir.join("Game")).unwrap();
        dir.join("Game")
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn loads_built_project() {
        let dir = built_project("ok");
        let game = load_game(&dir).unwrap();
        assert_eq!(game.name, "Game");
        assert_eq!(game.scripts.len(), 1);
        assert!(game.scripts[0].file.ends_with("player.embc"));
        cleanup(&dir);
    }

    #[test]
    fn loads_without_script_sources() {
        let dir = built_project("nosrc");
        let packed = dir.parent().unwrap().join("packed");
        for sub in ["scenes", "build", "assets"] {
            let mut stack = vec![dir.join(sub)];
            while let Some(top) = stack.pop() {
                let rel = top.strip_prefix(&dir).unwrap();
                std::fs::create_dir_all(packed.join(rel)).unwrap();
                for entry in std::fs::read_dir(&top).unwrap() {
                    let path = entry.unwrap().path();
                    if path.is_dir() {
                        stack.push(path);
                    } else {
                        std::fs::copy(&path, packed.join(path.strip_prefix(&dir).unwrap()))
                            .unwrap();
                    }
                }
            }
        }
        std::fs::copy(dir.join("ember.project"), packed.join("ember.project")).unwrap();
        let game = load_game(&packed).unwrap();
        assert_eq!(game.scripts.len(), 1);
        cleanup(&dir);
        let _ = std::fs::remove_dir_all(&packed);
    }

    #[test]
    fn missing_bytecode_errors() {
        let dir = std::env::temp_dir().join(format!("ember-player-nobuild-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        ember_project::create_project(&dir.join("Game"), "Game").unwrap();
        let err = load_game(&dir.join("Game")).unwrap_err();
        assert!(err.to_string().contains("ember build"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nested_script_rejected() {
        assert!(script_stem("scripts/player.em") == Some("player".to_string()));
        assert!(script_stem("scripts/sub/ai.em").is_none());
        assert!(script_stem("other/player.em").is_none());
    }
}
