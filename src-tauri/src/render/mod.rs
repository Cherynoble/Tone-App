pub mod engine;
pub mod params;
pub mod pipeline;
pub mod preview;
pub mod texture;
pub mod tiles;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderTiming {
    pub decode_ms: u128,
    pub source_upload_ms: u128,
    pub preview_dispatch_ms: u128,
    pub param_update_ms: u128,
    pub tile_render_ms: u128,
}
