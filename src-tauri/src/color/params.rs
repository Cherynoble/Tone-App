use serde::{Deserialize, Serialize};

pub const HUE_BAND_COUNT: usize = 8;
pub const HUE_LUT_SIZE: usize = 1024;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LuminanceMaskMode {
    Off,
    Shadows,
    Midtones,
    Highlights,
    Custom,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DebugOutputMode {
    Off,
    LuminanceMask,
    HueBandWeights,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HslAdjustment {
    pub hue: f32,
    pub saturation: f32,
    pub luminance: f32,
}

impl Default for HslAdjustment {
    fn default() -> Self {
        Self {
            hue: 0.0,
            saturation: 0.0,
            luminance: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LuminanceMask {
    pub mode: LuminanceMaskMode,
    pub control_points: [f32; 4],
}

impl Default for LuminanceMask {
    fn default() -> Self {
        Self {
            mode: LuminanceMaskMode::Off,
            control_points: [0.0, 0.25, 0.75, 1.0],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GradeParams {
    pub global: HslAdjustment,
    pub hue_bands: [HslAdjustment; HUE_BAND_COUNT],
    pub luminance_mask: LuminanceMask,
    pub black_and_white: bool,
    pub black_and_white_luminance: [f32; HUE_BAND_COUNT],
    pub debug_output: DebugOutputMode,
}

impl Default for GradeParams {
    fn default() -> Self {
        Self {
            global: HslAdjustment::default(),
            hue_bands: [HslAdjustment::default(); HUE_BAND_COUNT],
            luminance_mask: LuminanceMask::default(),
            black_and_white: false,
            black_and_white_luminance: [0.0; HUE_BAND_COUNT],
            debug_output: DebugOutputMode::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsl {
    pub hue: f32,
    pub saturation: f32,
    pub luminance: f32,
}

pub fn rgb_to_hsl(rgb: [f32; 3]) -> Hsl {
    let [r, g, b] = rgb;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let chroma = max - min;
    let luminance = (max + min) * 0.5;

    if chroma <= f32::EPSILON {
        return Hsl {
            hue: 0.0,
            saturation: 0.0,
            luminance,
        };
    }

    let saturation = chroma / (1.0 - (2.0 * luminance - 1.0).abs());
    let hue_sector = if max == r {
        ((g - b) / chroma).rem_euclid(6.0)
    } else if max == g {
        ((b - r) / chroma) + 2.0
    } else {
        ((r - g) / chroma) + 4.0
    };

    Hsl {
        hue: hue_sector * 60.0,
        saturation,
        luminance,
    }
}

pub fn hsl_to_rgb(hsl: Hsl) -> [f32; 3] {
    let hue = hsl.hue.rem_euclid(360.0);
    let saturation = hsl.saturation.clamp(0.0, 1.0);
    let luminance = hsl.luminance.clamp(0.0, 1.0);

    let chroma = (1.0 - (2.0 * luminance - 1.0).abs()) * saturation;
    let x = chroma * (1.0 - ((hue / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = luminance - chroma * 0.5;

    let [r1, g1, b1] = match hue {
        h if h < 60.0 => [chroma, x, 0.0],
        h if h < 120.0 => [x, chroma, 0.0],
        h if h < 180.0 => [0.0, chroma, x],
        h if h < 240.0 => [0.0, x, chroma],
        h if h < 300.0 => [x, 0.0, chroma],
        _ => [chroma, 0.0, x],
    };

    [r1 + m, g1 + m, b1 + m]
}

pub fn generate_hue_band_lut() -> Vec<[f32; HUE_BAND_COUNT]> {
    (0..HUE_LUT_SIZE)
        .map(|index| normalized_hue_weights(index as f32 * 360.0 / HUE_LUT_SIZE as f32))
        .collect()
}

pub fn normalized_hue_weights(hue_degrees: f32) -> [f32; HUE_BAND_COUNT] {
    let sigma = 360.0 / HUE_BAND_COUNT as f32 * 0.5;
    let mut weights = [0.0; HUE_BAND_COUNT];
    let mut sum = 0.0;

    for (band, weight) in weights.iter_mut().enumerate() {
        let center = band as f32 * 360.0 / HUE_BAND_COUNT as f32;
        let distance = circular_hue_distance(hue_degrees, center);
        *weight = (-0.5 * (distance / sigma).powi(2)).exp();
        sum += *weight;
    }

    if sum > 0.0 {
        for weight in &mut weights {
            *weight /= sum;
        }
    }

    weights
}

fn circular_hue_distance(a: f32, b: f32) -> f32 {
    let delta = (a - b).rem_euclid(360.0).abs();
    delta.min(360.0 - delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_hsl_round_trip_is_precise() {
        let samples = [
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.18, 0.42, 0.73],
            [0.93, 0.51, 0.12],
            [0.02, 0.08, 0.11],
        ];

        for rgb in samples {
            let round_trip = hsl_to_rgb(rgb_to_hsl(rgb));
            for channel in 0..3 {
                assert!(
                    (rgb[channel] - round_trip[channel]).abs() <= 1e-5,
                    "channel {channel} failed for {rgb:?}: got {round_trip:?}"
                );
            }
        }
    }

    #[test]
    fn hue_band_weights_are_normalized_for_every_degree() {
        for hue in 0..360 {
            let sum: f32 = normalized_hue_weights(hue as f32).iter().sum();
            assert!((0.999..=1.001).contains(&sum), "hue {hue} sum was {sum}");
        }
    }
}
