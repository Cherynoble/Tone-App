struct ColorParams {
  exposure: f32,
  contrast: f32,
  saturation: f32,
  temperature: f32,
  tint: f32,
};

@group(0) @binding(0)
var<uniform> params: ColorParams;

// Placeholder shader entry point for the future GPU color pipeline.
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) _global_id: vec3<u32>) {
}
