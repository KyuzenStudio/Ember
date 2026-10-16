use std::ffi::OsString;
use std::path::Path;

use crate::Outcome;

pub(crate) fn resolve_player_exe() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("locate player: {e}"))?;
    let name = if cfg!(windows) {
        "ember-player.exe"
    } else {
        "ember-player"
    };
    let mut dir = exe.parent().map(|p| p.to_path_buf());
    for _ in 0..3 {
        let Some(parent) = dir else {
            break;
        };
        let candidate = parent.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
        dir = parent.parent().map(|p| p.to_path_buf());
    }
    Err("ember-player binary not found next to ember; build the workspace first".to_string())
}

pub(crate) fn cmd_new(args: &[OsString], cwd: &Path) -> Outcome {
    let name = args[1].to_string_lossy().into_owned();
    let dir = cwd.join(&args[1]);
    match ember_project::create_project(&dir, &name) {
        Ok(()) => Outcome {
            code: 0,
            stdout: format!("created '{name}' in '{}'\n", dir.display()),
            stderr: String::new(),
        },
        Err(e) => Outcome {
            code: 1,
            stdout: String::new(),
            stderr: format!("error: {e}\n"),
        },
    }
}

pub(crate) fn cmd_check(args: &[OsString], cwd: &Path) -> Outcome {
    let dir = if args.len() == 2 {
        cwd.join(&args[1])
    } else {
        cwd.to_path_buf()
    };
    if !dir.is_dir() {
        return Outcome {
            code: 1,
            stdout: String::new(),
            stderr: format!("error: '{}' is not a directory\n", dir.display()),
        };
    }
    match ember_project::check_dir(&dir) {
        Ok(report) if report.errors.is_empty() => Outcome {
            code: 0,
            stdout: format!("ok: {} script(s) checked\n", report.checked),
            stderr: String::new(),
        },
        Ok(report) => {
            let mut stderr = String::new();
            for e in &report.errors {
                stderr.push_str(&format!("{e}\n"));
            }
            Outcome {
                code: 1,
                stdout: String::new(),
                stderr,
            }
        }
        Err(e) => Outcome {
            code: 1,
            stdout: String::new(),
            stderr: format!("error: {e}\n"),
        },
    }
}

pub(crate) fn cmd_build(args: &[OsString], cwd: &Path) -> Outcome {
    let dir = if args.len() == 2 {
        cwd.join(&args[1])
    } else {
        cwd.to_path_buf()
    };
    match ember_project::build_project(&dir) {
        Ok(report) => Outcome {
            code: 0,
            stdout: format!(
                "built {} script(s) to '{}'\n",
                report.scripts,
                dir.join("build").display()
            ),
            stderr: String::new(),
        },
        Err(e) => Outcome {
            code: 1,
            stdout: String::new(),
            stderr: format!("error: {e}\n"),
        },
    }
}

pub(crate) fn cmd_package(args: &[OsString], cwd: &Path) -> Outcome {
    let dir = if args.len() == 2 {
        cwd.join(&args[1])
    } else {
        cwd.to_path_buf()
    };
    let player = match resolve_player_exe() {
        Ok(p) => p,
        Err(e) => {
            return Outcome {
                code: 1,
                stdout: String::new(),
                stderr: format!("error: {e}\n"),
            };
        }
    };
    match ember_project::package_project(&dir, &player) {
        Ok(report) => {
            let mut stdout = format!("packaged '{}'\n", report.dir.display());
            for file in &report.files {
                stdout.push_str(&format!("  {file}\n"));
            }
            Outcome {
                code: 0,
                stdout,
                stderr: String::new(),
            }
        }
        Err(e) => Outcome {
            code: 1,
            stdout: String::new(),
            stderr: format!("error: {e}\n"),
        },
    }
}
