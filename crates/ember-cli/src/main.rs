use std::ffi::OsString;
use std::path::Path;

struct Outcome {
    code: i32,
    stdout: String,
    stderr: String,
}

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("error: current directory: {e}");
            return 1;
        }
    };
    let outcome = dispatch(&args, &cwd);
    if !outcome.stdout.is_empty() {
        print!("{}", outcome.stdout);
    }
    if !outcome.stderr.is_empty() {
        eprint!("{}", outcome.stderr);
    }
    outcome.code
}

fn version() -> String {
    format!("ember {}", env!("CARGO_PKG_VERSION"))
}

fn resolve_player_exe() -> Result<std::path::PathBuf, String> {
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

fn usage() -> String {
    "Usage: ember [--version|--help]\n       ember new <name>\n       ember check [path]\n       ember build [path]\n       ember package [path]\n\nCommands:\n  new <name>      Create a new project in './<name>'\n  check [path]    Check scripts in a project without opening a window\n  build [path]    Validate and compile scripts to build/*.embc\n  package [path]  Bundle a standalone Windows game folder\n\nOptions:\n  --version, -V  Print version and exit\n  --help, -h     Print this help and exit\n"
        .to_string()
}

fn dispatch(args: &[OsString], cwd: &Path) -> Outcome {
    if args.is_empty() {
        return Outcome {
            code: 2,
            stdout: String::new(),
            stderr: usage(),
        };
    }
    let first = args[0].to_string_lossy();
    match first.as_ref() {
        "--version" | "-V" if args.len() == 1 => Outcome {
            code: 0,
            stdout: format!("{}\n", version()),
            stderr: String::new(),
        },
        "--help" | "-h" | "help" if args.len() == 1 => Outcome {
            code: 0,
            stdout: usage(),
            stderr: String::new(),
        },
        "new" if args.len() == 2 => {
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
        "new" => Outcome {
            code: 2,
            stdout: String::new(),
            stderr: "error: usage: ember new <name>\n".to_string(),
        },
        "check" if args.len() <= 2 => {
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
        "check" => Outcome {
            code: 2,
            stdout: String::new(),
            stderr: "error: usage: ember check [path]\n".to_string(),
        },
        "build" if args.len() <= 2 => {
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
        "build" => Outcome {
            code: 2,
            stdout: String::new(),
            stderr: "error: usage: ember build [path]\n".to_string(),
        },
        "package" if args.len() <= 2 => {
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
        "package" => Outcome {
            code: 2,
            stdout: String::new(),
            stderr: "error: usage: ember package [path]\n".to_string(),
        },
        _ => Outcome {
            code: 2,
            stdout: String::new(),
            stderr: format!("error: unknown argument '{first}'. Try 'ember --help'.\n"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(s: &str) -> OsString {
        OsString::from(s)
    }

    fn cwd() -> std::path::PathBuf {
        Path::new("/tmp").to_path_buf()
    }

    #[test]
    fn version_has_ember_prefix() {
        assert!(version().starts_with("ember "));
    }

    #[test]
    fn usage_mentions_new() {
        let u = usage();
        assert!(u.contains("new <name>"));
        assert!(u.contains("check [path]"));
        assert!(u.contains("--version"));
        assert!(u.contains("--help"));
    }

    #[test]
    fn dispatch_version_exits_zero() {
        let o = dispatch(&[arg("--version")], &cwd());
        assert_eq!(o.code, 0);
        assert!(o.stdout.starts_with("ember "));
        assert!(o.stderr.is_empty());
    }

    #[test]
    fn dispatch_help_exits_zero() {
        let o = dispatch(&[arg("--help")], &cwd());
        assert_eq!(o.code, 0);
        assert!(o.stdout.contains("Usage: ember"));
    }

    #[test]
    fn dispatch_no_args_exits_two() {
        let o = dispatch(&[], &cwd());
        assert_eq!(o.code, 2);
        assert!(!o.stderr.is_empty());
    }

    #[test]
    fn dispatch_unknown_exits_two() {
        let o = dispatch(&[arg("run")], &cwd());
        assert_eq!(o.code, 2);
        assert!(o.stdout.is_empty());
        assert!(o.stderr.contains("unknown argument"));
    }

    #[test]
    fn dispatch_two_flags_exits_two() {
        let o = dispatch(&[arg("--version"), arg("--help")], &cwd());
        assert_eq!(o.code, 2);
        assert!(!o.stderr.is_empty());
    }

    #[test]
    fn dispatch_new_missing_name_exits_two() {
        let o = dispatch(&[arg("new")], &cwd());
        assert_eq!(o.code, 2);
        assert!(o.stderr.contains("ember new <name>"));
    }

    #[test]
    fn dispatch_new_creates_project() {
        let base = std::env::temp_dir().join(format!("ember-cli-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let o = dispatch(&[arg("new"), arg("Wired")], &base);
        assert_eq!(o.code, 0);
        assert!(o.stdout.contains("Wired"));
        let loaded = ember_project::load_project(&base.join("Wired")).unwrap();
        assert_eq!(loaded.project.name, "Wired");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn dispatch_check_valid_project() {
        let base = std::env::temp_dir().join(format!("ember-cli-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        ember_project::create_project(&base.join("Game"), "Game").unwrap();
        let o = dispatch(&[arg("check"), arg("Game")], &base);
        assert_eq!(o.code, 0);
        assert!(o.stdout.contains("ok: 1 script(s) checked"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn dispatch_check_broken_script() {
        let base = std::env::temp_dir().join(format!("ember-cli-broken-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        ember_project::create_project(&base.join("Game"), "Game").unwrap();
        std::fs::write(
            base.join("Game").join("scripts").join("player.em"),
            "var x = \n",
        )
        .unwrap();
        let o = dispatch(&[arg("check"), arg("Game")], &base);
        assert_eq!(o.code, 1);
        assert!(o.stderr.contains("player.em:1:"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn dispatch_check_missing_dir() {
        let o = dispatch(&[arg("check"), arg("Nope")], &cwd());
        assert_eq!(o.code, 1);
        assert!(o.stderr.contains("not a directory"));
    }

    #[test]
    fn dispatch_build_compiles_bytecode() {
        let base = std::env::temp_dir().join(format!("ember-cli-build-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        ember_project::create_project(&base.join("Game"), "Game").unwrap();
        let o = dispatch(&[arg("build"), arg("Game")], &base);
        assert_eq!(o.code, 0);
        assert!(o.stdout.contains("built 1 script(s)"));
        let bytes = std::fs::read(base.join("Game").join("build").join("player.embc")).unwrap();
        assert_eq!(&bytes[0..4], b"EMBC");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn dispatch_build_broken_fails() {
        let base = std::env::temp_dir().join(format!("ember-cli-buildbad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        ember_project::create_project(&base.join("Game"), "Game").unwrap();
        std::fs::write(
            base.join("Game").join("scripts").join("player.em"),
            "var x = \n",
        )
        .unwrap();
        let o = dispatch(&[arg("build"), arg("Game")], &base);
        assert_eq!(o.code, 1);
        assert!(o.stderr.contains("player.em:1:"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn dispatch_package_bundles_player() {
        let player = resolve_player_exe().expect("player exe must exist after build");
        let base = std::env::temp_dir().join(format!("ember-cli-pkg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        ember_project::create_project(&base.join("Game"), "Game").unwrap();
        let o = dispatch(&[arg("package"), arg("Game")], &base);
        assert_eq!(o.code, 0, "stderr: {}", o.stderr);
        let out = base.join("Game").join("dist").join("Game-windows-x86_64");
        assert!(out.join(player.file_name().unwrap()).is_file());
        assert!(out.join("build").join("player.embc").is_file());
        assert!(out.join("assets").join("player.png").is_file());
        assert!(out.join("scenes").join("main.ron").is_file());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn dispatch_new_bad_name_exits_one() {
        let o = dispatch(&[arg("new"), arg("../evil")], &cwd());
        assert_eq!(o.code, 1);
        assert!(o.stderr.starts_with("error:"));
    }
}
