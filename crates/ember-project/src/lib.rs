mod assets;
mod build;
mod check;
mod error;
mod model;
mod scaffold;
mod validate;

pub use assets::{import_asset, list_assets};
pub use build::{BuildReport, PackageReport, build_project, package_project};
pub use check::{CheckReport, check_dir, script_stems};
pub use error::ProjectError;
pub use model::{
    LoadedProject, PROJECT_FILE_NAME, PROJECT_FORMAT_VERSION, Project, STARTER_SCRIPT,
    STARTER_SPRITE, STARTUP_SCENE,
};
pub use scaffold::{create_project, load_project, load_project_opts};
pub use validate::{validate_asset_path, validate_project_name};

#[cfg(test)]
mod tests {
    use super::*;
    use ember_scene::{Scene, SpriteRenderer};
    use std::path::{Path, PathBuf};
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
    fn build_produces_verified_embc() {
        let dir = fresh_dir("buildok");
        create_project(&dir, "Built").unwrap();
        let report = build_project(&dir).unwrap();
        assert_eq!(report.scripts, 1);
        let bytes = std::fs::read(dir.join("build").join("player.embc")).unwrap();
        let chunk = ember_vm::decode(&bytes).unwrap();
        ember_vm::verify(&chunk).unwrap();
        assert!(!dir.join("build").join("assets").exists());
        cleanup(&dir);
    }

    #[test]
    fn build_rejects_broken_script() {
        let dir = fresh_dir("buildbad");
        create_project(&dir, "Bad").unwrap();
        std::fs::write(dir.join(STARTER_SCRIPT), "var x = \n").unwrap();
        let err = build_project(&dir).unwrap_err();
        assert!(err.to_string().contains("player.em:1:"));
        cleanup(&dir);
    }

    #[test]
    fn package_copies_layout() {
        let dir = fresh_dir("pkg");
        create_project(&dir, "Packed").unwrap();
        let fake = dir.join("fake-player.exe");
        std::fs::write(&fake, b"exe").unwrap();
        let report = package_project(&dir, &fake).unwrap();
        assert!(report.dir.join("fake-player.exe").is_file());
        assert!(report.dir.join("build").join("player.embc").is_file());
        assert!(report.dir.join("assets").join("player.png").is_file());
        assert!(report.dir.join("scenes").join("main.ron").is_file());
        assert!(report.dir.join(PROJECT_FILE_NAME).is_file());
        assert!(report.files.iter().any(|f| f.contains("player.embc")));
        cleanup(&dir);
    }

    #[test]
    fn package_needs_player() {
        let dir = fresh_dir("pkgnoplayer");
        create_project(&dir, "NoPlayer").unwrap();
        let err = package_project(&dir, &dir.join("missing.exe")).unwrap_err();
        assert!(err.to_string().contains("not found"));
        cleanup(&dir);
    }

    fn copy_tree(src: &Path, dest: &Path) {
        let mut stack = vec![src.to_path_buf()];
        while let Some(top) = stack.pop() {
            let rel = top.strip_prefix(src).unwrap();
            let target = dest.join(rel);
            if top.is_dir() {
                std::fs::create_dir_all(&target).unwrap();
                for entry in std::fs::read_dir(&top).unwrap() {
                    stack.push(entry.unwrap().path());
                }
            } else {
                std::fs::copy(&top, &target).unwrap();
            }
        }
    }

    #[test]
    fn package_sample_includes_audio_without_sources() {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/coin-dash");
        let dir = fresh_dir("pkgsample");
        let proj = dir.join("CoinDash");
        copy_tree(&src, &proj);
        let fake = dir.join("fake-player.exe");
        std::fs::write(&fake, b"exe").unwrap();
        let report = package_project(&proj, &fake).unwrap();
        for wav in ["coin.wav", "jump.wav", "hit.wav", "music.wav"] {
            assert!(
                report.dir.join("assets").join(wav).is_file(),
                "{wav} missing from package"
            );
        }
        for stem in ["player.embc", "cam.embc", "enemy.embc"] {
            assert!(
                report.dir.join("build").join(stem).is_file(),
                "{stem} missing from package"
            );
        }
        assert!(!report.dir.join("scripts").exists());
        assert!(!report.dir.join("build").join("assets").exists());
        cleanup(&dir);
    }

    #[test]
    fn missing_script_source_rejected() {
        let dir = fresh_dir("noscript");
        create_project(&dir, "NoScript").unwrap();
        std::fs::remove_file(dir.join(STARTER_SCRIPT)).unwrap();
        let err = load_project(&dir).unwrap_err();
        assert!(err.to_string().contains("not found"));
        cleanup(&dir);
    }

    #[test]
    fn import_png_copies_file() {
        let dir = fresh_dir("import");
        create_project(&dir, "Imp").unwrap();
        let rel = import_asset(&dir, Path::new("../../assets/sprite-demo.png")).unwrap();
        assert_eq!(rel, "assets/sprite-demo.png");
        assert!(dir.join(&rel).is_file());
        let listed = list_assets(&dir).unwrap();
        assert!(listed.contains(&"player.png".to_string()));
        assert!(listed.contains(&"sprite-demo.png".to_string()));
        cleanup(&dir);
    }

    #[test]
    fn import_rejects_non_png() {
        let dir = fresh_dir("importbad");
        create_project(&dir, "ImpBad").unwrap();
        let fake = dir.join("note.txt");
        std::fs::write(&fake, b"hello").unwrap();
        let err = import_asset(&dir, &fake).unwrap_err();
        assert!(err.to_string().contains("must be .png, .wav, or .ogg"));
        let fake_png = dir.join("fake.png");
        std::fs::write(&fake_png, b"not png bytes").unwrap();
        let err = import_asset(&dir, &fake_png).unwrap_err();
        assert!(err.to_string().contains("not a PNG"));
        cleanup(&dir);
    }

    #[test]
    fn import_accepts_wav_and_ogg() {
        let dir = fresh_dir("importaudio");
        create_project(&dir, "ImpAudio").unwrap();
        let wav = dir.join("s.wav");
        std::fs::write(&wav, b"RIFF....WAVE").unwrap();
        assert_eq!(import_asset(&dir, &wav).unwrap(), "assets/s.wav");
        let ogg = dir.join("m.ogg");
        std::fs::write(&ogg, b"OggS....").unwrap();
        assert_eq!(import_asset(&dir, &ogg).unwrap(), "assets/m.ogg");
        assert!(dir.join("assets/s.wav").is_file());
        cleanup(&dir);
    }

    #[test]
    fn import_missing_source() {
        let dir = fresh_dir("importmissing");
        create_project(&dir, "ImpMissing").unwrap();
        let err = import_asset(&dir, Path::new("nope/gone.png")).unwrap_err();
        assert!(err.to_string().contains("not found"));
        cleanup(&dir);
    }

    #[test]
    fn import_overwrites() {
        let dir = fresh_dir("importover");
        create_project(&dir, "ImpOver").unwrap();
        let rel = import_asset(&dir, Path::new("../../assets/sprite-demo.png")).unwrap();
        let first = std::fs::read(dir.join(&rel)).unwrap();
        std::fs::write(dir.join(&rel), b"changed").unwrap();
        import_asset(&dir, Path::new("../../assets/sprite-demo.png")).unwrap();
        assert_eq!(std::fs::read(dir.join(&rel)).unwrap(), first);
        cleanup(&dir);
    }

    #[test]
    fn import_same_file_is_noop() {
        let dir = fresh_dir("importsame");
        create_project(&dir, "ImpSame").unwrap();
        let rel = import_asset(&dir, Path::new("../../assets/sprite-demo.png")).unwrap();
        let same = dir.join(&rel).canonicalize().unwrap();
        let again = import_asset(&dir, &same).unwrap();
        assert_eq!(again, rel);
        cleanup(&dir);
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
