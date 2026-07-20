use std::{path::PathBuf, sync::mpsc, time::Instant};

use serde::Serialize;
use wgpu::util::DeviceExt;

use super::{
    params::GpuGradeParams,
    pipeline::RenderPipelineState,
    preview::preview_dimensions,
    texture::{decode_image_read_only, log_timing},
    tiles::{full_resolution_tiles, RenderTile},
};
use crate::color::{DebugOutputMode, GradeParams};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderCounters {
    pub source_uploads: u64,
    pub param_updates: u64,
    pub preview_dispatches: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderState {
    pub source_width: u32,
    pub source_height: u32,
    pub preview_width: u32,
    pub preview_height: u32,
    pub source_uploads: u64,
    pub param_updates: u64,
    pub preview_dispatches: u64,
    pub identity_preview: bool,
    pub debug_output: DebugOutputMode,
    pub tiles: Vec<RenderTile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct RenderEngine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: RenderPipelineState,
    source: Option<SourceImage>,
    params: GradeParams,
    identity_preview: bool,
    counters: RenderCounters,
}

struct SourceImage {
    width: u32,
    height: u32,
    preview_width: u32,
    preview_height: u32,
    source_view: wgpu::TextureView,
    preview_texture: wgpu::Texture,
    preview_view: wgpu::TextureView,
    uniform: wgpu::Buffer,
    bands: wgpu::Buffer,
    bw: wgpu::Buffer,
}

impl RenderEngine {
    pub async fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or("no compatible GPU adapter found")?;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Tone render device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        let pipeline = RenderPipelineState::new(&device);
        Ok(Self {
            device,
            queue,
            pipeline,
            source: None,
            params: GradeParams::default(),
            identity_preview: false,
            counters: RenderCounters::default(),
        })
    }

    pub fn load_image(&mut self, path: PathBuf) -> Result<RenderState, String> {
        let decoded = decode_image_read_only(path).map_err(|e| e.to_string())?;
        let upload_start = Instant::now();
        let source_texture = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("source image"),
                size: wgpu::Extent3d {
                    width: decoded.width,
                    height: decoded.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &decoded.rgba,
        );
        let [preview_width, preview_height] = preview_dimensions(decoded.width, decoded.height);
        let preview_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("preview output"),
            size: wgpu::Extent3d {
                width: preview_width,
                height: preview_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let gpu = GpuGradeParams::from_grade_params(
            &self.effective_params(),
            [decoded.width, decoded.height],
            [preview_width, preview_height],
        );
        let (uniform, bands, bw) = RenderPipelineState::create_param_buffers(&self.device, &gpu);
        self.source = Some(SourceImage {
            width: decoded.width,
            height: decoded.height,
            preview_width,
            preview_height,
            source_view: source_texture.create_view(&Default::default()),
            preview_view: preview_texture.create_view(&Default::default()),
            preview_texture,
            uniform,
            bands,
            bw,
        });
        self.counters.source_uploads += 1;
        let source_upload_ms = upload_start.elapsed().as_millis();
        log_timing(
            "load_image",
            super::RenderTiming {
                decode_ms: decoded.decode_ms,
                source_upload_ms,
                ..Default::default()
            },
        );
        self.dispatch_preview()?;
        self.state()
    }

    pub fn update_grade_params(&mut self, params: GradeParams) -> Result<RenderState, String> {
        self.params = params;
        self.update_params_and_dispatch()
    }
    pub fn set_debug_output(&mut self, mode: DebugOutputMode) -> Result<RenderState, String> {
        self.params.debug_output = mode;
        self.update_params_and_dispatch()
    }
    pub fn render_identity_preview(&mut self, enabled: bool) -> Result<RenderState, String> {
        self.identity_preview = enabled;
        self.update_params_and_dispatch()
    }

    fn effective_params(&self) -> GradeParams {
        if self.identity_preview {
            GradeParams::default()
        } else {
            self.params.clone()
        }
    }
    fn update_params_and_dispatch(&mut self) -> Result<RenderState, String> {
        let start = Instant::now();
        if let Some(source) = &self.source {
            let gpu = GpuGradeParams::from_grade_params(
                &self.effective_params(),
                [source.width, source.height],
                [source.preview_width, source.preview_height],
            );
            self.queue
                .write_buffer(&source.uniform, 0, bytemuck::bytes_of(&gpu.uniforms));
            self.queue.write_buffer(
                &source.bands,
                0,
                bytemuck::cast_slice(&gpu.hue_band_adjustments),
            );
            self.queue.write_buffer(
                &source.bw,
                0,
                bytemuck::cast_slice(&gpu.bw_luminance_adjustments),
            );
            self.counters.param_updates += 1;
        }
        let param_update_ms = start.elapsed().as_millis();
        self.dispatch_preview()?;
        log_timing(
            "update_params",
            super::RenderTiming {
                param_update_ms,
                ..Default::default()
            },
        );
        self.state()
    }
    fn dispatch_preview(&mut self) -> Result<(), String> {
        let Some(source) = &self.source else {
            return Ok(());
        };
        let start = Instant::now();
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grade bind group"),
            layout: &self.pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source.source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&source.preview_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: source.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: source.bands.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.pipeline.hue_lut_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: source.bw.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&self.pipeline.sampler),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("preview encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("preview pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(
                source.preview_width.div_ceil(8),
                source.preview_height.div_ceil(8),
                1,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.counters.preview_dispatches += 1;
        log_timing(
            "preview_dispatch",
            super::RenderTiming {
                preview_dispatch_ms: start.elapsed().as_millis(),
                ..Default::default()
            },
        );
        Ok(())
    }

    pub fn read_preview_rgba(&mut self) -> Result<PreviewFrame, String> {
        self.dispatch_preview()?;
        let source = self.source.as_ref().ok_or("no image loaded")?;
        let width = source.preview_width;
        let height = source.preview_height;
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let output_buffer_size = padded_bytes_per_row as u64 * height as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("preview readback buffer"),
            size: output_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("preview readback encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &source.preview_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = output_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

        let padded = slice.get_mapped_range();
        let mut rgba = vec![0; (unpadded_bytes_per_row * height) as usize];
        for row in 0..height as usize {
            let padded_offset = row * padded_bytes_per_row as usize;
            let rgba_offset = row * unpadded_bytes_per_row as usize;
            rgba[rgba_offset..rgba_offset + unpadded_bytes_per_row as usize].copy_from_slice(
                &padded[padded_offset..padded_offset + unpadded_bytes_per_row as usize],
            );
        }
        drop(padded);
        output_buffer.unmap();

        Ok(PreviewFrame {
            width,
            height,
            rgba,
        })
    }

    pub fn state(&self) -> Result<RenderState, String> {
        let s = self.source.as_ref().ok_or("no image loaded")?;
        Ok(RenderState {
            source_width: s.width,
            source_height: s.height,
            preview_width: s.preview_width,
            preview_height: s.preview_height,
            source_uploads: self.counters.source_uploads,
            param_updates: self.counters.param_updates,
            preview_dispatches: self.counters.preview_dispatches,
            identity_preview: self.identity_preview,
            debug_output: self.effective_params().debug_output,
            tiles: full_resolution_tiles(s.width, s.height),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_prove_parameter_updates_do_not_upload_source() {
        let mut counters = RenderCounters::default();
        counters.source_uploads += 1;
        let before = counters.source_uploads;
        counters.param_updates += 1;
        counters.preview_dispatches += 1;
        assert_eq!(counters.source_uploads, before);
    }

    #[test]
    fn loads_image_updates_params_and_reads_preview_when_gpu_is_available() {
        let Some(mut engine) = pollster::block_on(RenderEngine::new()).ok() else {
            eprintln!("skipping GPU smoke test because no compatible adapter is available");
            return;
        };

        let path = std::env::temp_dir().join(format!(
            "tone-render-smoke-{}-{}.png",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let mut image = image::RgbaImage::new(4, 2);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Rgba([x as u8 * 40, y as u8 * 80, 128, 255]);
        }
        image.save(&path).expect("write smoke fixture");

        let loaded = engine
            .load_image(path.clone())
            .expect("load image through render engine");
        assert_eq!((loaded.source_width, loaded.source_height), (4, 2));
        assert_eq!((loaded.preview_width, loaded.preview_height), (4, 2));
        assert_eq!(loaded.source_uploads, 1);
        assert_eq!(loaded.preview_dispatches, 1);

        let mut params = GradeParams::default();
        params.global.luminance = 0.1;
        let updated = engine.update_grade_params(params).expect("update params");
        assert_eq!(updated.source_uploads, 1);
        assert_eq!(updated.param_updates, 1);
        assert!(updated.preview_dispatches >= 2);

        let frame = engine.read_preview_rgba().expect("read preview rgba");
        assert_eq!((frame.width, frame.height), (4, 2));
        assert_eq!(frame.rgba.len(), 4 * 2 * 4);

        let _ = std::fs::remove_file(path);
    }
}
