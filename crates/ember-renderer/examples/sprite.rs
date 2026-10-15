fn main() {
    let path = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("assets/sprite-demo.png"));
    if let Err(e) = ember_renderer::run_sprite_window(&path) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
