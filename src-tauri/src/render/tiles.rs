use serde::Serialize;

pub const FULL_RES_TILE_SIZE: u32 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderTile {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub fn full_resolution_tiles(width: u32, height: u32) -> Vec<RenderTile> {
    let mut tiles = Vec::new();
    let mut y = 0;
    while y < height {
        let mut x = 0;
        while x < width {
            tiles.push(RenderTile {
                x,
                y,
                width: (width - x).min(FULL_RES_TILE_SIZE),
                height: (height - y).min(FULL_RES_TILE_SIZE),
            });
            x += FULL_RES_TILE_SIZE;
        }
        y += FULL_RES_TILE_SIZE;
    }
    tiles
}
