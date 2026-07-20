pub const MAX_PREVIEW_LONG_EDGE: u32 = 2048;

pub fn preview_dimensions(width: u32, height: u32) -> [u32; 2] {
    assert!(width > 0 && height > 0, "image dimensions must be non-zero");
    let long_edge = width.max(height);
    if long_edge <= MAX_PREVIEW_LONG_EDGE {
        return [width, height];
    }
    let scale = MAX_PREVIEW_LONG_EDGE as f64 / long_edge as f64;
    [
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calculates_preview_dimensions_for_common_shapes() {
        assert_eq!(preview_dimensions(4000, 2000), [2048, 1024]);
        assert_eq!(preview_dimensions(2000, 4000), [1024, 2048]);
        assert_eq!(preview_dimensions(3000, 3000), [2048, 2048]);
        assert_eq!(preview_dimensions(800, 600), [800, 600]);
        assert_eq!(preview_dimensions(50000, 10000), [2048, 410]);
    }
}
