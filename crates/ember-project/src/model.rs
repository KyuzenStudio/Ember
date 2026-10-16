use std::path::PathBuf;

use ember_scene::Scene;
use serde::{Deserialize, Serialize};

pub const PROJECT_FORMAT_VERSION: u32 = 1;
pub const PROJECT_FILE_NAME: &str = "ember.project";
pub const STARTUP_SCENE: &str = "scenes/main.ron";
pub const STARTER_SPRITE: &str = "assets/player.png";
pub const STARTER_SCRIPT: &str = "scripts/player.em";

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
