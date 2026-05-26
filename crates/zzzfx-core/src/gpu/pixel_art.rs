//! GPU-accelerated Pixel Art rendering via wgpu compute shader.
//!
//! ## Architecture
//!
//! A single compute shader dispatch processes one pixel per workgroup invocation.
//! Each invocation computes the average/quantized color for its cell, applies
//! optional ordered dithering, and draws grid lines at cell boundaries.
//!
//! Floyd-Steinberg dithering is rejected on GPU (serial error diffusion) and
//! falls back to CPU.
//!
//! ## Fallback strategy
//!
//! - `try_render` returns `Ok(true)` on success.
//! - If the shared GPU device is unavailable, returns `Ok(false)` → caller uses CPU.
//! - If the GPU device is lost at runtime, marks GPU unavailable and returns `Ok(false)`.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::settings::pixel_art::{Dithering, ZzzPixelArt};

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    frame_width: u32,
    frame_height: u32,
    pixel_size_w: u32,
    pixel_size_h: u32,
    color_levels: u32,
    dithering: u32,
    dither_amount: f32,
    show_grid: u32,
    grid_thickness: f32,
    grid_opacity: f32,
    contrast: f32,
    saturation: f32,
    _pad: u32,
}

pub(crate) fn try_render(
    settings: &ZzzPixelArt,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed) {
        return Ok(false);
    }

    // Floyd-Steinberg requires serial error diffusion — not practical on GPU
    if matches!(settings.dithering, Dithering::FloydSteinberg) {
        return Ok(false);
    }

    let w = width as u32;
    let h = height as u32;

    let pixel_size_w = ((w as f32 * settings.pixel_size_h.clamp(0.0, 1.0)).round() as u32)
        .clamp(1, w);
    let pixel_size_h = if settings.square {
        pixel_size_w
    } else {
        ((h as f32 * settings.pixel_size_v.clamp(0.0, 1.0)).round() as u32).clamp(1, h)
    };

    let (device, queue) = super::get_or_init_shared_device()?;

    let uniforms = Uniforms {
        frame_width: w,
        frame_height: h,
        pixel_size_w,
        pixel_size_h,
        color_levels: settings.color_levels.clamp(2, 256) as u32,
        dithering: settings.dithering as u32,
        dither_amount: settings.dithering_amount.clamp(0.0, 1.0),
        show_grid: if settings.show_grid { 1 } else { 0 },
        grid_thickness: settings.grid_thickness.clamp(0.0, 1.0),
        grid_opacity: settings.grid_opacity.clamp(0.0, 1.0),
        contrast: settings.contrast.clamp(0.0, 1.0),
        saturation: settings.saturation.clamp(0.0, 1.0),
        _pad: 0,
    };

    let buf_size = (w * h * 4) as u64;
    let src_data = &src[..(buf_size as usize).min(src.len())];

    let src_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pixel_art_src"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&src_buf, 0, src_data);

    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pixel_art_uniforms"),
        size: std::mem::size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    let dst_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pixel_art_dst"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pixel_art_staging"),
        size: buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Pipeline
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pixel_art"),
        source: super::load_shader(include_str!("../shaders/pixel_art.wgsl")),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("pixel_art"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    // Bind group
    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pixel_art"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: src_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: dst_buf.as_entire_binding(),
            },
        ],
    });

    // Dispatch
    let wg_x = (w + 7) / 8;
    let wg_y = (h + 7) / 8;

    {
        let mut encoder = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("pixel_art") },
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("pixel_art"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }
        encoder.copy_buffer_to_buffer(&dst_buf, 0, &staging_buf, 0, buf_size);
        queue.submit(std::iter::once(encoder.finish()));
    }

    // Readback
    let staging_slice = staging_buf.slice(..buf_size);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    staging_slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });

    match rx.recv() {
        Ok(Ok(())) => {
            let mapped = staging_slice.get_mapped_range();
            dst[..buf_size as usize].copy_from_slice(&mapped);
            drop(mapped);
            staging_buf.unmap();
            Ok(true)
        }
        _ => {
            let _ = staging_buf.unmap();
            GPU_AVAILABLE.store(false, Ordering::Relaxed);
            Ok(false)
        }
    }
}
