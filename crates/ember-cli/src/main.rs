use std::ffi::OsString;

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
    let outcome = dispatch(&args);
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

fn usage() -> String {
    "Usage: ember [--version|--help]\n\nOptions:\n  --version, -V  Print version and exit\n  --help, -h     Print this help and exit\n"
        .to_string()
}

fn dispatch(args: &[OsString]) -> Outcome {
    if args.is_empty() {
        return Outcome {
            code: 2,
            stdout: String::new(),
            stderr: usage(),
        };
    }
    if args.len() > 1 {
        return Outcome {
            code: 2,
            stdout: String::new(),
            stderr: format!(
                "error: expected one argument, got {}\n{}",
                args.len(),
                usage()
            ),
        };
    }
    let arg = args[0].to_string_lossy();
    match arg.as_ref() {
        "--version" | "-V" => Outcome {
            code: 0,
            stdout: format!("{}\n", version()),
            stderr: String::new(),
        },
        "--help" | "-h" | "help" => Outcome {
            code: 0,
            stdout: usage(),
            stderr: String::new(),
        },
        _ => Outcome {
            code: 2,
            stdout: String::new(),
            stderr: format!("error: unknown argument '{arg}'. Try 'ember --help'.\n"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(s: &str) -> OsString {
        OsString::from(s)
    }

    #[test]
    fn version_has_ember_prefix() {
        assert!(version().starts_with("ember "));
    }

    #[test]
    fn usage_mentions_flags() {
        let u = usage();
        assert!(u.contains("--version"));
        assert!(u.contains("--help"));
    }

    #[test]
    fn dispatch_version_exits_zero() {
        let o = dispatch(&[arg("--version")]);
        assert_eq!(o.code, 0);
        assert!(o.stdout.starts_with("ember "));
        assert!(o.stderr.is_empty());
    }

    #[test]
    fn dispatch_help_exits_zero() {
        let o = dispatch(&[arg("--help")]);
        assert_eq!(o.code, 0);
        assert!(o.stdout.contains("Usage: ember"));
    }

    #[test]
    fn dispatch_no_args_exits_two() {
        let o = dispatch(&[]);
        assert_eq!(o.code, 2);
        assert!(!o.stderr.is_empty());
    }

    #[test]
    fn dispatch_unknown_exits_two() {
        let o = dispatch(&[arg("run")]);
        assert_eq!(o.code, 2);
        assert!(o.stdout.is_empty());
        assert!(o.stderr.contains("unknown argument"));
    }

    #[test]
    fn dispatch_two_args_exits_two() {
        let o = dispatch(&[arg("--version"), arg("--help")]);
        assert_eq!(o.code, 2);
        assert!(!o.stderr.is_empty());
    }
}
