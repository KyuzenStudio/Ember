use std::path::Path;

use crate::types::{DecodeError, DecodedImage};

pub fn decode_png(bytes: &[u8]) -> Result<DecodedImage, DecodeError> {
    if bytes.is_empty() {
        return Err(DecodeError("empty PNG input".to_string()));
    }
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| DecodeError(format!("invalid PNG ({e})")))?;
    let rgba = img.to_rgba8();
    Ok(DecodedImage {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

pub fn load_png_file(path: &Path) -> Result<DecodedImage, DecodeError> {
    let bytes = std::fs::read(path)
        .map_err(|e| DecodeError(format!("cannot read '{}': {e}", path.display())))?;
    decode_png(&bytes).map_err(|e| DecodeError(format!("'{}': {e}", path.display())))
}

pub(crate) fn effective_size(width: u32, height: u32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 {
        None
    } else {
        Some((width, height))
    }
}
