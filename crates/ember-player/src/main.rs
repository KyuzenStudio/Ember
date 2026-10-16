use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use ember_player::load_game;
use ember_renderer::{SceneRenderer, load_png_file};
use ember_runtime::{InputState, Runtime};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

fn version() -> String {
    format!("ember-player {}", env!("CARGO_PKG_VERSION"))
}

fn key_value(code: KeyCode) -> Option<i32> {
    match code {
        KeyCode::ArrowUp => Some(0),
        KeyCode::ArrowDown => Some(1),
        KeyCode::ArrowLeft => Some(2),
        KeyCode::ArrowRight => Some(3),
        KeyCode::Space => Some(4),
        KeyCode::Escape => Some(5),
        _ => None,
    }
}

struct App {
    dir: PathBuf,
    window: Option<Arc<Window>>,
    renderer: Option<SceneRenderer>,
    runtime: Option<Runtime>,
    audio: Option<ember_audio::AudioEngine>,
    input: InputState,
    last: Option<Instant>,
    error: Option<String>,
}

impl App {
    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let window = match &self.window {
            Some(w) => w,
            None => return,
        };
        let renderer = match &mut self.renderer {
            Some(r) => r,
            None => return,
        };
        let runtime = match &mut self.runtime {
            Some(r) => r,
            None => return,
        };
        let now = Instant::now();
        let dt = match self.last {
            Some(t) => (now - t).as_secs_f32().min(0.1),
            None => 0.016,
        };
        self.last = Some(now);
        let mut logs = Vec::new();
        if let Err(e) = runtime.tick(&mut self.input, dt, &mut logs) {
            self.error = Some(e.to_string());
            event_loop.exit();
            return;
        }
        for line in &logs {
            println!("{line}");
        }
        if let Some(title) = runtime.take_title() {
            window.set_title(&title);
        }
        if runtime.take_restart_requested() {
            match load_game(&self.dir) {
                Ok(game) => {
                    let mut fresh_logs = Vec::new();
                    match Runtime::load_prebuilt(game.scene, game.scripts, &mut fresh_logs) {
                        Ok(fresh) => {
                            for line in &fresh_logs {
                                println!("{line}");
                            }
                            self.runtime = Some(fresh);
                        }
                        Err(e) => {
                            self.error = Some(e.to_string());
                            event_loop.exit();
                        }
                    }
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    event_loop.exit();
                }
            }
            return;
        }
        let cmds = runtime.take_audio();
        if !cmds.is_empty() {
            if self.audio.is_none() {
                self.audio = Some(ember_audio::AudioEngine::open());
            }
            if let Some(engine) = &mut self.audio {
                let mut failures = Vec::new();
                for cmd in &cmds {
                    let result = match cmd {
                        ember_runtime::AudioCmd::Play(path) => engine.play(&self.dir.join(path)),
                        ember_runtime::AudioCmd::Loop(path) => {
                            engine.play_loop(&self.dir.join(path))
                        }
                        ember_runtime::AudioCmd::Stop => {
                            engine.stop();
                            Ok(())
                        }
                    };
                    if let Err(e) = result {
                        failures.push(e.to_string());
                    }
                }
                for failure in failures {
                    eprintln!("error: {failure}");
                }
            }
        }
        let mut sizes = std::collections::HashMap::new();
        for entity in &runtime.scene().entities {
            for path in entity.texture_paths() {
                if let Some(size) = renderer.texture_size(path) {
                    sizes.insert(path.to_string(), size);
                }
            }
        }
        let (camera, draws) = ember_renderer::scene_sprites(runtime.scene(), &sizes);
        if let Err(e) = renderer.draw(&camera, &draws) {
            self.error = Some(e.to_string());
            event_loop.exit();
            return;
        }
        window.request_redraw();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let game = match load_game(&self.dir) {
            Ok(g) => g,
            Err(e) => {
                self.error = Some(e.to_string());
                event_loop.exit();
                return;
            }
        };
        let attrs = Window::default_attributes()
            .with_title(game.name.clone())
            .with_inner_size(LogicalSize::new(800.0, 600.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(format!("create window: {e}"));
                event_loop.exit();
                return;
            }
        };
        let mut renderer = match SceneRenderer::new(window.clone()) {
            Ok(r) => r,
            Err(e) => {
                self.error = Some(e.to_string());
                event_loop.exit();
                return;
            }
        };
        for entity in &game.scene.entities {
            for path in entity.texture_paths() {
                let full = self.dir.join(path);
                match load_png_file(&full) {
                    Ok(img) => {
                        if let Err(e) = renderer.cache_texture(path, &img) {
                            self.error = Some(e.to_string());
                            event_loop.exit();
                            return;
                        }
                    }
                    Err(e) => {
                        self.error = Some(e.to_string());
                        event_loop.exit();
                        return;
                    }
                }
            }
        }
        let mut logs = Vec::new();
        match Runtime::load_prebuilt(game.scene, game.scripts, &mut logs) {
            Ok(mut runtime) => {
                for line in &logs {
                    println!("{line}");
                }
                let cmds = runtime.take_audio();
                if !cmds.is_empty() {
                    if self.audio.is_none() {
                        self.audio = Some(ember_audio::AudioEngine::open());
                    }
                    if let Some(engine) = &mut self.audio {
                        for cmd in cmds {
                            let result = match &cmd {
                                ember_runtime::AudioCmd::Play(path) => {
                                    engine.play(&self.dir.join(path))
                                }
                                ember_runtime::AudioCmd::Loop(path) => {
                                    engine.play_loop(&self.dir.join(path))
                                }
                                ember_runtime::AudioCmd::Stop => {
                                    engine.stop();
                                    Ok(())
                                }
                            };
                            if let Err(e) = result {
                                eprintln!("error: {e}");
                            }
                        }
                    }
                }
                self.window = Some(window);
                self.renderer = Some(renderer);
                self.runtime = Some(runtime);
            }
            Err(e) => {
                self.error = Some(e.to_string());
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = &self.window else {
            return;
        };
        if window_id != window.id() {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => self.frame(event_loop),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        ..
                    },
                ..
            } => {
                if code == KeyCode::Escape {
                    event_loop.exit();
                    return;
                }
                if let Some(key) = key_value(code) {
                    match state {
                        ElementState::Pressed => self.input.press(key),
                        ElementState::Released => self.input.release(key),
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
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
    let mut app = App {
        dir,
        window: None,
        renderer: None,
        runtime: None,
        audio: None,
        input: InputState::new(),
        last: None,
        error: None,
    };
    if event_loop.run_app(&mut app).is_err() && app.error.is_none() {
        app.error = Some("event loop failed".to_string());
    }
    if let Some(msg) = app.error {
        eprintln!("error: {msg}");
        std::process::exit(1);
    }
}
