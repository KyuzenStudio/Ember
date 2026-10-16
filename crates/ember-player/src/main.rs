use std::path::PathBuf;

use winit::event_loop::EventLoop;

mod app;

use app::App;

fn version() -> String {
    format!("ember-player {}", env!("CARGO_PKG_VERSION"))
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    if let Some(flag) = &first
        && (flag == "--version" || flag == "-V")
        && args.next().is_none()
    {
        println!("{}", version());
        return;
    }
    if let Some(flag) = &first
        && (flag == "--help" || flag == "-h")
        && args.next().is_none()
    {
        println!("Usage: ember-player [path]\n\nRun a built game without opening the editor.");
        return;
    }
    let dir = match &first {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    if args.next().is_some() {
        eprintln!("error: usage: ember-player [path]");
        std::process::exit(2);
    }
    let event_loop = EventLoop::new().unwrap_or_else(|e| {
        eprintln!("error: create event loop: {e}");
        std::process::exit(1);
    });
    let mut app = App::new(dir);
    if event_loop.run_app(&mut app).is_err() && app.error.is_none() {
        app.error = Some("event loop failed".to_string());
    }
    if let Some(msg) = app.error {
        eprintln!("error: {msg}");
        std::process::exit(1);
    }
}
