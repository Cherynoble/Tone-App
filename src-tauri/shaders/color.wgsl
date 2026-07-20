struct HslAdjustment {
  hue: f32,
  saturation: f32,
  luminance: f32,
  _pad: f32,
};

struct GradeUniforms {
  global: HslAdjustment,
  luminance_mask_points: vec4<f32>,
  luminance_mask_mode: u32,
  black_and_white: u32,
  debug_output: u32,
  _pad: u32,
  source_size: vec2<f32>,
  output_size: vec2<f32>,
};

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(6) var source_sampler: sampler;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> grade: GradeUniforms;
@group(0) @binding(3) var<storage, read> hue_band_adjustments: array<HslAdjustment, 8>;
@group(0) @binding(4) var<storage, read> hue_band_lut: array<vec4<f32>, 2048>;
@group(0) @binding(5) var<storage, read> bw_luminance_adjustments: array<f32, 8>;

fn srgb_channel_to_linear(channel: f32) -> f32 {
  if (channel <= 0.04045) {
    return channel / 12.92;
  }
  return pow((channel + 0.055) / 1.055, 2.4);
}

fn linear_channel_to_srgb(channel: f32) -> f32 {
  if (channel <= 0.0031308) {
    return channel * 12.92;
  }
  return 1.055 * pow(channel, 1.0 / 2.4) - 0.055;
}

fn srgb_to_linear(rgb: vec3<f32>) -> vec3<f32> {
  return vec3<f32>(
    srgb_channel_to_linear(rgb.r),
    srgb_channel_to_linear(rgb.g),
    srgb_channel_to_linear(rgb.b)
  );
}

fn linear_to_srgb(rgb: vec3<f32>) -> vec3<f32> {
  return vec3<f32>(
    linear_channel_to_srgb(rgb.r),
    linear_channel_to_srgb(rgb.g),
    linear_channel_to_srgb(rgb.b)
  );
}

fn rgb_to_hsl(rgb: vec3<f32>) -> vec3<f32> {
  let max_channel = max(max(rgb.r, rgb.g), rgb.b);
  let min_channel = min(min(rgb.r, rgb.g), rgb.b);
  let chroma = max_channel - min_channel;
  let luminance = (max_channel + min_channel) * 0.5;

  if (chroma <= 0.0000001) {
    return vec3<f32>(0.0, 0.0, luminance);
  }

  let saturation = chroma / (1.0 - abs(2.0 * luminance - 1.0));
  var hue_sector: f32;
  if (max_channel == rgb.r) {
    hue_sector = ((rgb.g - rgb.b) / chroma) % 6.0;
  } else if (max_channel == rgb.g) {
    hue_sector = ((rgb.b - rgb.r) / chroma) + 2.0;
  } else {
    hue_sector = ((rgb.r - rgb.g) / chroma) + 4.0;
  }

  return vec3<f32>((hue_sector * 60.0 + 360.0) % 360.0, saturation, luminance);
}

fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
  let hue = (hsl.x + 360.0) % 360.0;
  let saturation = clamp(hsl.y, 0.0, 1.0);
  let luminance = clamp(hsl.z, 0.0, 1.0);
  let chroma = (1.0 - abs(2.0 * luminance - 1.0)) * saturation;
  let x = chroma * (1.0 - abs((hue / 60.0) % 2.0 - 1.0));
  let m = luminance - chroma * 0.5;

  var rgb_prime: vec3<f32>;
  if (hue < 60.0) {
    rgb_prime = vec3<f32>(chroma, x, 0.0);
  } else if (hue < 120.0) {
    rgb_prime = vec3<f32>(x, chroma, 0.0);
  } else if (hue < 180.0) {
    rgb_prime = vec3<f32>(0.0, chroma, x);
  } else if (hue < 240.0) {
    rgb_prime = vec3<f32>(0.0, x, chroma);
  } else if (hue < 300.0) {
    rgb_prime = vec3<f32>(x, 0.0, chroma);
  } else {
    rgb_prime = vec3<f32>(chroma, 0.0, x);
  }

  return rgb_prime + vec3<f32>(m);
}

fn luminance_mask(luminance: f32) -> f32 {
  if (grade.luminance_mask_mode == 0u) {
    return 1.0;
  }

  var points = grade.luminance_mask_points;
  if (grade.luminance_mask_mode == 1u) {
    points = vec4<f32>(0.0, 0.0, 0.35, 0.6);
  } else if (grade.luminance_mask_mode == 2u) {
    points = vec4<f32>(0.2, 0.4, 0.6, 0.8);
  } else if (grade.luminance_mask_mode == 3u) {
    points = vec4<f32>(0.4, 0.65, 1.0, 1.0);
  }

  let fade_in = smoothstep(points.x, points.y, luminance);
  let fade_out = 1.0 - smoothstep(points.z, points.w, luminance);
  return clamp(fade_in * fade_out, 0.0, 1.0);
}

fn lut_weight(hue: f32, band: u32) -> f32 {
  let normalized_hue = (hue + 360.0) % 360.0;
  let index = u32(round(normalized_hue / 360.0 * 1023.0));
  let lo = hue_band_lut[index * 2u];
  let hi = hue_band_lut[index * 2u + 1u];

  switch band {
    case 0u: { return lo.x; }
    case 1u: { return lo.y; }
    case 2u: { return lo.z; }
    case 3u: { return lo.w; }
    case 4u: { return hi.x; }
    case 5u: { return hi.y; }
    case 6u: { return hi.z; }
    default: { return hi.w; }
  }
}

fn dominant_lut_weight(hue: f32) -> f32 {
  return max(
    max(max(lut_weight(hue, 0u), lut_weight(hue, 1u)), max(lut_weight(hue, 2u), lut_weight(hue, 3u))),
    max(max(lut_weight(hue, 4u), lut_weight(hue, 5u)), max(lut_weight(hue, 6u), lut_weight(hue, 7u)))
  );
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
  let dimensions = textureDimensions(output_texture);
  if (global_id.x >= dimensions.x || global_id.y >= dimensions.y) {
    return;
  }

  let uv = (vec2<f32>(global_id.xy) + vec2<f32>(0.5)) / grade.output_size;
  let source = textureSampleLevel(source_texture, source_sampler, uv, 0.0);
  let linear_rgb = srgb_to_linear(source.rgb);
  var hsl = rgb_to_hsl(linear_rgb);
  let pre_adjust_hue = hsl.x;
  let pre_adjust_luminance = hsl.z;
  let mask = luminance_mask(pre_adjust_luminance);
  let saturation_gate = smoothstep(0.02, 0.12, hsl.y);
  hsl.x = (hsl.x + grade.global.hue * mask) % 360.0;
  hsl.y = clamp(hsl.y + grade.global.saturation * mask, 0.0, 1.0);
  hsl.z = clamp(hsl.z + grade.global.luminance * mask, 0.0, 1.0);

  var band_luminance_delta = 0.0;
  for (var band = 0u; band < 8u; band = band + 1u) {
    let hue_weight = lut_weight(pre_adjust_hue, band);
    let weight = hue_weight * saturation_gate * mask;
    let adjustment = hue_band_adjustments[band];
    hsl.x = hsl.x + adjustment.hue * weight;
    hsl.y = clamp(hsl.y + adjustment.saturation * weight, 0.0, 1.0);
    hsl.z = clamp(hsl.z + adjustment.luminance * weight, 0.0, 1.0);
    band_luminance_delta = band_luminance_delta + bw_luminance_adjustments[band] * hue_weight * saturation_gate;
  }

  if (grade.debug_output == 1u) {
    textureStore(output_texture, vec2<i32>(global_id.xy), vec4<f32>(vec3<f32>(mask), source.a));
    return;
  }

  if (grade.debug_output == 2u) {
    let dominant_weight = dominant_lut_weight(pre_adjust_hue);
    textureStore(output_texture, vec2<i32>(global_id.xy), vec4<f32>(vec3<f32>(dominant_weight), source.a));
    return;
  }

  if (grade.black_and_white != 0u) {
    hsl.y = 0.0;
    hsl.z = clamp(pre_adjust_luminance + band_luminance_delta * mask + grade.global.luminance * mask, 0.0, 1.0);
  }

  let graded_linear = hsl_to_rgb(hsl);
  let graded_srgb = linear_to_srgb(clamp(graded_linear, vec3<f32>(0.0), vec3<f32>(1.0)));
  textureStore(output_texture, vec2<i32>(global_id.xy), vec4<f32>(graded_srgb, source.a));
}
