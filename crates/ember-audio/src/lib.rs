use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use rodio::Source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioError(pub String);

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AudioError {}

fn check_path(path: &Path) -> Result<(), AudioError> {
    if !path.is_file() {
        return Err(AudioError(format!("audio '{}' not found", path.display())));
    }
    let ok = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("wav") || e.eq_ignore_ascii_case("ogg"));
    if !ok {
        return Err(AudioError(format!(
            "audio '{}': only .wav and .ogg supported",
            path.display()
        )));
    }
    Ok(())
}

fn decode(path: &Path) -> Result<rodio::Decoder<BufReader<File>>, AudioError> {
    check_path(path)?;
    let file =
        File::open(path).map_err(|e| AudioError(format!("audio '{}': {e}", path.display())))?;
    rodio::Decoder::new(BufReader::new(file))
        .map_err(|e| AudioError(format!("audio '{}': cannot decode: {e}", path.display())))
}

pub struct AudioEngine {
    sink: Option<rodio::MixerDeviceSink>,
    music: Option<rodio::Player>,
    recorded: Vec<String>,
}

impl AudioEngine {
    pub fn open() -> Self {
        let sink = rodio::DeviceSinkBuilder::open_default_sink().ok();
        Self {
            sink,
            music: None,
            recorded: Vec::new(),
        }
    }

    pub fn is_live(&self) -> bool {
        self.sink.is_some()
    }

    pub fn recorded(&self) -> &[String] {
        &self.recorded
    }

    pub fn play(&mut self, path: &Path) -> Result<(), AudioError> {
        let source = decode(path)?;
        self.recorded.push(format!("play {}", path.display()));
        if let Some(sink) = &self.sink {
            sink.mixer().add(source);
        }
        Ok(())
    }

    pub fn play_loop(&mut self, path: &Path) -> Result<(), AudioError> {
        let source = decode(path)?;
        self.recorded.push(format!("loop {}", path.display()));
        self.stop();
        if let Some(sink) = &self.sink {
            let player = rodio::Player::connect_new(sink.mixer());
            player.append(source.repeat_infinite());
            self.music = Some(player);
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        self.recorded.push("stop".to_string());
        if let Some(player) = self.music.take() {
            player.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_wav(path: &Path) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&44u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&44100u32.to_le_bytes());
        bytes.extend_from_slice(&88200u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 8]);
        std::fs::write(path, bytes).unwrap();
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("ember-audio-m19");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn valid_wav_accepted_and_recorded() {
        let path = tmp("s.wav");
        tiny_wav(&path);
        let mut engine = AudioEngine::open();
        engine.play(&path).unwrap();
        engine.play_loop(&path).unwrap();
        engine.stop();
        assert!(engine.recorded().len() >= 3);
        assert!(engine.recorded()[0].starts_with("play "));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_errors_with_path() {
        let path = tmp("gone.wav");
        let _ = std::fs::remove_file(&path);
        let mut engine = AudioEngine::open();
        let err = engine.play(&path).unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
        assert!(err.to_string().contains("gone.wav"), "{err}");
    }

    #[test]
    fn mp3_rejected() {
        let path = tmp("s.mp3");
        std::fs::write(&path, b"ID3").unwrap();
        let mut engine = AudioEngine::open();
        let err = engine.play(&path).unwrap_err();
        assert!(err.to_string().contains("only .wav and .ogg"), "{err}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn garbage_wav_rejected() {
        let path = tmp("junk.wav");
        std::fs::write(&path, b"not audio at all").unwrap();
        let mut engine = AudioEngine::open();
        let err = engine.play(&path).unwrap_err();
        assert!(err.to_string().contains("cannot decode"), "{err}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sample_assets_decode() {
        let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/coin-dash/assets");
        let mut engine = AudioEngine::open();
        for name in ["coin.wav", "jump.wav", "hit.wav"] {
            engine
                .play(&base.join(name))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        engine
            .play_loop(&base.join("music.wav"))
            .expect("music.wav must decode");
    }
}
