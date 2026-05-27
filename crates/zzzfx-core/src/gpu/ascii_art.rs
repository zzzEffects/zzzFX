#![allow(dead_code)]
//! GPU-accelerated ASCII art rendering via wgpu compute shader.
//!
//! ## Architecture
//!
//! A single compute shader dispatch processes one cell per workgroup invocation.
//! The shader samples the source buffer, averages the luminance and colour of
//! each cell, maps the luminance to a character index, and stamps the glyph from
//! a pre-baked atlas (flat storage buffer) into the output.
//!
//! ## Fallback strategy
//!
//! - `try_render` returns `Ok(true)` on success.
//! - If the shared GPU device is unavailable, returns `Ok(false)` → caller uses CPU.
//! - If the GPU device is lost at runtime, marks GPU unavailable and returns `Ok(false)`
//!   so subsequent frames use the CPU path.

use std::sync::{atomic::AtomicBool, atomic::Ordering};

use crate::ascii_art::GlyphCache;
use crate::settings::ascii_art::ZzzAsciiArt;

// ---------------------------------------------------------------------------
// GPU availability flag
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

// ---------------------------------------------------------------------------
// Uniform buffer (must match WGSL `Uniforms` struct layout)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    frame_width: u32,
    frame_height: u32,
    cell_size: u32,
    atlas_cols: u32,
    atlas_cell_w: u32,
    atlas_cell_h: u32,
    atlas_max_w: u32,
    atlas_max_h: u32,
    charset_len: u32,
    brightness: f32,
    contrast: f32,
    invert_luma: u32,
    color_mode: u32,
    bg_alpha: f32,
    _pad: [u32; 1],
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Attempt GPU-accelerated ASCII art rendering.
///
/// Returns `Ok(true)` on success, `Ok(false)` if GPU is unavailable
/// (caller should fall back to CPU), or `Err(String)` on unexpected error.
pub(crate) fn try_render(
    settings: &ZzzAsciiArt,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
    cache: &GlyphCache,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed) {
        return Ok(false);
    }

    let cell_size = (settings.font_size * 256.0 / 100.0).round() as u32;
    let w = width as u32;
    let h = height as u32;
    let charset_len = cache.bitmaps.len() as u32;

    if charset_len == 0 {
        return Ok(false);
    }

    let (device, queue) = super::get_or_init_shared_device()?;

    // Build atlas data
    let (atlas_data, atlas_cols, atlas_cell_w, atlas_cell_h, atlas_max_w, atlas_max_h) =
        build_atlas_data(cache, cell_size);

    let atlas_u32: Vec<u32> = atlas_data
        .chunks_exact(4)
        .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
        .collect();

    let uniforms = Uniforms {
        frame_width: w,
        frame_height: h,
        cell_size,
        atlas_cols,
        atlas_cell_w,
        atlas_cell_h,
        atlas_max_w,
        atlas_max_h,
        charset_len,
        brightness: settings.brightness.clamp(0.0, 1.0),
        contrast: settings.contrast.clamp(0.0, 1.0),
        invert_luma: if settings.invert_luma { 1 } else { 0 },
        color_mode: settings.color_mode as u32,
        bg_alpha: settings.bg_color_a.clamp(0.0, 1.0),
        _pad: [0; 1],
    };

    let buf_size = (w * h * 4) as u64;
    let src_data = &src[..(buf_size as usize).min(src.len())];

    // Buffers
    let src_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ascii_src"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&src_buf, 0, src_data);

    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ascii_uniforms"),
        size: std::mem::size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    let atlas_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ascii_atlas"),
        size: (atlas_u32.len() * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&atlas_buf, 0, bytemuck::cast_slice(&atlas_u32));

    let dst_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ascii_dst"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ascii_staging"),
        size: buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Pipeline
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ascii_art"),
        source: wgpu::ShaderSource::Wgsl(
            include_str!("../shaders/ascii_art.wgsl").into(),
        ),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ascii_art"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    // Bind group
    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ascii_art"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: src_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: atlas_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: dst_buf.as_entire_binding() },
        ],
    });

    // Dispatch
    let cols = (w + cell_size - 1) / cell_size;
    let rows = (h + cell_size - 1) / cell_size;
    let wg_x = (cols + 7) / 8;
    let wg_y = (rows + 7) / 8;

    {
        let mut encoder = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("ascii_art") },
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ascii_art"),
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

// ---------------------------------------------------------------------------
// Atlas construction (CPU-side, packed as RGBA8 byte array then u32)
// ---------------------------------------------------------------------------

fn build_atlas_data(
    cache: &GlyphCache,
    cell_size: u32,
) -> (Vec<u8>, u32, u32, u32, u32, u32) {
    let n = cache.bitmaps.len() as u32;
    // Single-row atlas: N glyphs side by side
    let atlas_w = cell_size * n;
    let atlas_h = cell_size;

    // Round up atlas_w to multiple of 4 for u32 alignment
    let atlas_w_padded = (atlas_w + 3) / 4 * 4;

    let mut atlas = vec![0u8; (atlas_w_padded * atlas_h * 4) as usize];
    for (gi, glyph) in cache.bitmaps.iter().enumerate() {
        let base_x = gi as u32 * cell_size;
        let gw = glyph.width.min(cell_size);
        let gh = glyph.height.min(cell_size);
        let ox = (cell_size - gw) / 2;
        let oy = (cell_size - gh) / 2;
        for gy in 0..gh as usize {
            for gx in 0..gw as usize {
                let alpha = glyph.data[gy * glyph.width as usize + gx];
                let px = (base_x + ox + gx as u32) as usize;
                let py = (oy + gy as u32) as usize;
                let idx = (py * atlas_w_padded as usize + px) * 4;
                atlas[idx] = 255;
                atlas[idx + 1] = 255;
                atlas[idx + 2] = 255;
                atlas[idx + 3] = alpha;
            }
        }
    }

    (
        atlas,
        n,                  // atlas_cols
        cell_size,          // atlas_cell_w
        cell_size,          // atlas_cell_h
        atlas_w_padded,     // max_w
        atlas_h,            // max_h
    )
}
