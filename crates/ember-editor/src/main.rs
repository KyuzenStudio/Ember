mod app;

use std::path::PathBuf;

use app::EditorApp;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    if let Some(flag) = &first
        && (flag == "--version" || flag == "-V")
        && args.next().is_none()
    {
        println!("ember-editor {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let dir = match &first {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    if args.next().is_some() {
        eprintln!("error: usage: ember-editor [path]");
        std::process::exit(2);
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "Ember",
        options,
        Box::new(|_cc| Ok(Box::new(EditorApp::new(&dir)) as Box<dyn eframe::App>)),
    ) {
        eprintln!("error: editor: {e}");
        std::process::exit(1);
    }
}
