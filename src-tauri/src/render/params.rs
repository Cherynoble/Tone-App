use crate::color::{
    generate_hue_band_lut, DebugOutputMode, GradeParams, HslAdjustment, LuminanceMaskMode,
    HUE_BAND_COUNT, HUE_LUT_SIZE,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuHslAdjustment {
    pub hue: f32,
    pub saturation: f32,
    pub luminance: f32,
    pub _pad: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuGradeUniforms {
    pub global: GpuHslAdjustment,
    pub luminance_mask_points: [f32; 4],
    pub luminance_mask_mode: u32,
    pub black_and_white: u32,
    pub debug_output: u32,
    pub _pad: u32,
    pub source_size: [f32; 2],
    pub output_size: [f32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuGradeParams {
    pub uniforms: GpuGradeUniforms,
    pub hue_band_adjustments: [GpuHslAdjustment; HUE_BAND_COUNT],
    pub bw_luminance_adjustments: [f32; HUE_BAND_COUNT],
}

impl GpuGradeParams {
    pub fn from_grade_params(
        params: &GradeParams,
        source_size: [u32; 2],
        output_size: [u32; 2],
    ) -> Self {
        Self {
            uniforms: GpuGradeUniforms {
                global: params.global.into(),
                luminance_mask_points: params.luminance_mask.control_points,
                luminance_mask_mode: luminance_mask_mode_id(params.luminance_mask.mode),
                black_and_white: u32::from(params.black_and_white),
                debug_output: debug_output_mode_id(params.debug_output),
                _pad: 0,
                source_size: [source_size[0] as f32, source_size[1] as f32],
                output_size: [output_size[0] as f32, output_size[1] as f32],
            },
            hue_band_adjustments: params.hue_bands.map(Into::into),
            bw_luminance_adjustments: params.black_and_white_luminance,
        }
    }
}

impl From<HslAdjustment> for GpuHslAdjustment {
    fn from(value: HslAdjustment) -> Self {
        Self {
            hue: value.hue,
            saturation: value.saturation,
            luminance: value.luminance,
            _pad: 0.0,
        }
    }
}

pub fn luminance_mask_mode_id(mode: LuminanceMaskMode) -> u32 {
    match mode {
        LuminanceMaskMode::Off => 0,
        LuminanceMaskMode::Shadows => 1,
        LuminanceMaskMode::Midtones => 2,
        LuminanceMaskMode::Highlights => 3,
        LuminanceMaskMode::Custom => 4,
    }
}

pub fn debug_output_mode_id(mode: DebugOutputMode) -> u32 {
    match mode {
        DebugOutputMode::Off => 0,
        DebugOutputMode::LuminanceMask => 1,
        DebugOutputMode::HueBandWeights => 2,
    }
}

pub fn packed_hue_lut() -> Vec<[f32; 4]> {
    generate_hue_band_lut()
        .into_iter()
        .flat_map(|weights| {
            [
                [weights[0], weights[1], weights[2], weights[3]],
                [weights[4], weights[5], weights[6], weights[7]],
            ]
        })
        .collect()
}

pub const PACKED_HUE_LUT_VEC4_COUNT: usize = HUE_LUT_SIZE * 2;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{DebugOutputMode, LuminanceMask, LuminanceMaskMode};
    use bytemuck::Zeroable;

    #[test]
    fn converts_grade_params_to_stable_gpu_layout() {
        let mut params = GradeParams::default();
        params.global.hue = 12.0;
        params.luminance_mask = LuminanceMask {
            mode: LuminanceMaskMode::Highlights,
            control_points: [0.1, 0.2, 0.8, 0.9],
        };
        params.black_and_white = true;
        params.debug_output = DebugOutputMode::HueBandWeights;
        params.hue_bands[2].saturation = 0.25;
        params.black_and_white_luminance[3] = -0.1;
        let gpu = GpuGradeParams::from_grade_params(&params, [4000, 3000], [2048, 1536]);
        assert_eq!(gpu.uniforms.global.hue, 12.0);
        assert_eq!(gpu.uniforms.luminance_mask_mode, 3);
        assert_eq!(gpu.uniforms.black_and_white, 1);
        assert_eq!(gpu.uniforms.debug_output, 2);
        assert_eq!(gpu.uniforms.source_size, [4000.0, 3000.0]);
        assert_eq!(gpu.hue_band_adjustments[2].saturation, 0.25);
        assert_eq!(gpu.bw_luminance_adjustments[3], -0.1);
    }

    #[test]
    fn identity_default_params_are_stable() {
        let gpu = GpuGradeParams::from_grade_params(&GradeParams::default(), [1, 1], [1, 1]);
        assert_eq!(gpu.uniforms.luminance_mask_mode, 0);
        assert_eq!(gpu.uniforms.debug_output, 0);
        assert_eq!(gpu.uniforms.black_and_white, 0);
        assert!(gpu
            .hue_band_adjustments
            .iter()
            .all(|a| *a == GpuHslAdjustment::zeroed()));
        assert_eq!(gpu.bw_luminance_adjustments, [0.0; HUE_BAND_COUNT]);
    }

    #[test]
    fn packs_hue_lut_as_two_vec4s_per_shader_entry() {
        let lut = packed_hue_lut();
        assert_eq!(lut.len(), PACKED_HUE_LUT_VEC4_COUNT);
        for pair in lut.chunks_exact(2) {
            let sum: f32 = pair[0].iter().chain(pair[1].iter()).sum();
            assert!((0.999..=1.001).contains(&sum));
        }
    }
}
