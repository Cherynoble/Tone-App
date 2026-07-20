use std::{path::Path, time::Instant};

use image::GenericImageView;

use super::RenderTiming;

#[derive(Debug)]
pub struct DecodedImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub decode_ms: u128,
}

pub fn decode_image_read_only(path: impl AsRef<Path>) -> image::ImageResult<DecodedImage> {
    let start = Instant::now();
    let bytes = std::fs::read(path)?;
    let image = image::load_from_memory(&bytes)?;
    let (width, height) = image.dimensions();
    Ok(DecodedImage {
        rgba: image.to_rgba8().into_raw(),
        width,
        height,
        decode_ms: start.elapsed().as_millis(),
    })
}

pub(crate) fn log_timing(event: &str, timing: RenderTiming) {
    println!("render_event={event} decode_ms={} source_upload_ms={} param_update_ms={} preview_dispatch_ms={} tile_render_ms={}", timing.decode_ms, timing.source_upload_ms, timing.param_update_ms, timing.preview_dispatch_ms, timing.tile_render_ms);
}
