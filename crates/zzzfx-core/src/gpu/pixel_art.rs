//! GPU-accelerated Pixel Art rendering via wgpu compute shader (two-pass).
//!
//! ## Architecture
//!
//! **Pass 1 "cell_average":** one invocation per CELL. Each computes the average
//! RGBA color for its cell, applies contrast/saturation/dithering/quantization,
//! and writes the result to the `cell_colors` buffer.
//!
//! **Pass 2 "fill":** one invocation per OUTPUT PIXEL. Each looks up its cell's
//! pre-computed color from `cell_colors`, applies the grid overlay, and writes
//! to the output buffer.
//!
//! This avoids the redundant per-pixel cell averaging of a single-pass approach.
//! For pixel_size=16×16, Pass 1 does the averaging once per cell (256× less work),
//! and Pass 2 becomes a simple buffer lookup.
//!
//! ## Floyd-Steinberg hybrid path
//!
//! When Floyd-Steinberg dithering is selected:
//! 1. GPU runs Pass 1 (cell averaging)
//! 2. Cell colors are read back to CPU
//! 3. CPU runs the serial error-diffusion on the cell grid
//! 4. Modified cell colors are uploaded back to GPU
//! 5. GPU runs Pass 2 (fill)
//!
//! Since FS operates on cells (not pixels), the readback is small for typical
//! pixel art settings (e.g. ~8000 cells for 1080p with 16×16 blocks ≈ 32 KB).
//!
//! ## Resource caching
//!
//! Pipelines, buffers, and bind groups are cached. Buffers are recreated only
//! when dimensions change. Bind groups (which reference the buffers) are also
//! cached and recreated alongside buffers.
//!
//! ## Fallback strategy
//!
//! - `try_render` returns `Ok(true)` on success.
//! - If the shared GPU device is unavailable, returns `Ok(false)` → caller uses CPU.
//! - If the GPU device is lost at runtime, marks GPU unavailable and returns `Ok(false)`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::settings::pixel_art::{Dithering, ZzzPixelArt};

// ---------------------------------------------------------------------------
// GPU availability flag
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

// ---------------------------------------------------------------------------
// Uniforms — must match the WGSL Uniforms struct exactly
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    frame_width: u32,
    frame_height: u32,
    pixel_size_w: u32,
    pixel_size_h: u32,
    num_cols: u32,
    num_rows: u32,
    color_levels: u32,
    dithering: u32,
    dither_amount: f32,
    show_grid: u32,
    grid_thickness: f32,
    grid_color_r: f32,
    grid_color_g: f32,
    grid_color_b: f32,
    grid_color_a: f32,
    contrast: f32,
    saturation: f32,
    _pad: u32,
}

// ---------------------------------------------------------------------------
// Cached GPU resources
// ---------------------------------------------------------------------------

struct GpuBuffers {
    src_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    cell_colors_buf: wgpu::Buffer,
    cell_staging_buf: wgpu::Buffer,
    width: u32,
    height: u32,
}

struct GpuCtx {
    pipeline_cell: wgpu::ComputePipeline,
    pipeline_fill: wgpu::ComputePipeline,
    bufs: GpuBuffers,
    bind_group_cell: Option<wgpu::BindGroup>,
    bind_group_fill: Option<wgpu::BindGroup>,
}

static GPU_CTX: OnceLock<Mutex<GpuCtx>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

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

    // Variable-cell mode requires per-column/row widths — fall back to CPU
    if !settings.use_same_integer {
        return Ok(false);
    }

    let w = width as u32;
    let h = height as u32;

    let pixel_size_w = ((w as f32 * (settings.pixel_size_h.clamp(0.0, 100.0) / 100.0)).round() as u32)
        .clamp(1, w);
    let pixel_size_h = if settings.square {
        pixel_size_w
    } else {
        ((h as f32 * (settings.pixel_size_v.clamp(0.0, 100.0) / 100.0)).round() as u32).clamp(1, h)
    };

    let cols = w.div_ceil(pixel_size_w);
    let rows = h.div_ceil(pixel_size_h);

    // Fall back to CPU when cell count is too low for efficient GPU occupancy.
    // With < 4 workgroups, occupancy is so low the optimized CPU path is faster.
    let workgroups = cols.div_ceil(8) * rows.div_ceil(8);
    if workgroups < 4 {
        return Ok(false);
    }

    let is_fs = matches!(settings.dithering, Dithering::FloydSteinberg);

    let ctx = get_or_init_ctx()?;

    let mut guard = match ctx.lock() {
        Ok(g) => g,
        Err(_) => return Ok(false),
    };

    // Recreate buffers (and invalidate bind groups) when dimensions change
    if guard.bufs.width != w || guard.bufs.height != h {
        guard.bufs = create_buffers(super::get_or_init_shared_device()?.0, w, h);
        guard.bind_group_cell = None;
        guard.bind_group_fill = None;
    }

    let uniforms = Uniforms {
        frame_width: w,
        frame_height: h,
        pixel_size_w,
        pixel_size_h,
        num_cols: cols,
        num_rows: rows,
        color_levels: (settings.color_levels.clamp(2.0, 256.0).floor() as u32).max(2),
        dithering: settings.dithering as u32,
        dither_amount: settings.dithering_amount.clamp(0.0, 1.0),
        show_grid: if settings.show_grid { 1 } else { 0 },
        grid_thickness: settings.grid_thickness.clamp(0.0, 1.0),
        grid_color_r: settings.grid_color_r.clamp(0.0, 1.0),
        grid_color_g: settings.grid_color_g.clamp(0.0, 1.0),
        grid_color_b: settings.grid_color_b.clamp(0.0, 1.0),
        grid_color_a: settings.grid_color_a.clamp(0.0, 1.0),
        contrast: settings.contrast.clamp(0.0, 1.0),
        saturation: settings.saturation.clamp(0.0, 1.0),
        _pad: 0,
    };

    let buf_size = (w * h * 4) as u64;
    let cell_buf_size = (cols * rows * 4) as u64;
    let src_data = &src[..(buf_size as usize).min(src.len())];

    let (device, queue) = super::get_or_init_shared_device()?;

    // Upload source and uniforms
    queue.write_buffer(&guard.bufs.src_buf, 0, src_data);
    queue.write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    // ── Pass 1: cell averaging ──────────────────────────────────────────

    // Create or reuse cell bind group
    if guard.bind_group_cell.is_none() {
        guard.bind_group_cell = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pixel_art_cell"),
            layout: &guard.pipeline_cell.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: guard.bufs.src_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: guard.bufs.uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: guard.bufs.cell_colors_buf.as_entire_binding(),
                },
            ],
        }));
    }

    {
        let mut encoder = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("pixel_art_cell") },
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("cell_average"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&guard.pipeline_cell);
            pass.set_bind_group(0, guard.bind_group_cell.as_ref()
                .expect("pixel_art: cell bind group not initialized"), &[]);
            let wg_x = cols.div_ceil(8);
            let wg_y = rows.div_ceil(8);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    // ── Floyd-Steinberg hybrid: read back cells, diffuse on CPU, write back ──

    if is_fs {
        // Read back cell colors into staging buffer
        {
            let mut encoder = device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("pixel_art_fs_readback") },
            );
            encoder.copy_buffer_to_buffer(
                &guard.bufs.cell_colors_buf, 0,
                &guard.bufs.cell_staging_buf, 0,
                cell_buf_size,
            );
            queue.submit(std::iter::once(encoder.finish()));
        }

        // Map staging buffer and run FS diffusion
        let staging_slice = guard.bufs.cell_staging_buf.slice(..cell_buf_size);
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

                // Unpack u32 RGBA to [f32; 4] cells
                let num_cells = (cols * rows) as usize;
                let mut cells: Vec<[f32; 4]> = Vec::with_capacity(num_cells);
                for i in 0..num_cells {
                    let base = i * 4;
                    let r = mapped[base] as f32 / 255.0;
                    let g = mapped[base + 1] as f32 / 255.0;
                    let b = mapped[base + 2] as f32 / 255.0;
                    let a = mapped[base + 3] as f32 / 255.0;
                    cells.push([r, g, b, a]);
                }
                drop(mapped);
                guard.bufs.cell_staging_buf.unmap();

                // Run serial error diffusion on CPU
                crate::pixel_art::floyd_steinberg_diffuse(
                    &mut cells,
                    cols as usize,
                    rows as usize,
                    uniforms.color_levels as usize,
                    uniforms.dither_amount,
                );

                // Pack back to u8 and upload
                let mut cell_bytes: Vec<u8> = Vec::with_capacity(num_cells * 4);
                for cell in &cells {
                    cell_bytes.push((cell[0] * 255.0).round() as u8);
                    cell_bytes.push((cell[1] * 255.0).round() as u8);
                    cell_bytes.push((cell[2] * 255.0).round() as u8);
                    cell_bytes.push((cell[3] * 255.0).round() as u8);
                }
                queue.write_buffer(&guard.bufs.cell_colors_buf, 0, &cell_bytes);
            }
            _ => {
                guard.bufs.cell_staging_buf.unmap();
                GPU_AVAILABLE.store(false, Ordering::Relaxed);
                return Ok(false);
            }
        }
    }

    // ── Pass 2: fill output pixels ────────────────────────────────────────

    // Create or reuse fill bind group
    if guard.bind_group_fill.is_none() {
        guard.bind_group_fill = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pixel_art_fill"),
            layout: &guard.pipeline_fill.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: guard.bufs.uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: guard.bufs.dst_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: guard.bufs.cell_colors_buf.as_entire_binding(),
                },
            ],
        }));
    }

    {
        let mut encoder = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("pixel_art_fill") },
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("fill"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&guard.pipeline_fill);
            pass.set_bind_group(0, guard.bind_group_fill.as_ref()
                .expect("pixel_art: fill bind group not initialized"), &[]);
            let wg_x = w.div_ceil(8);
            let wg_y = h.div_ceil(8);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }
        encoder.copy_buffer_to_buffer(&guard.bufs.dst_buf, 0, &guard.bufs.staging_buf, 0, buf_size);
        queue.submit(std::iter::once(encoder.finish()));
    }

    // ── Readback output ────────────────────────────────────────────────────

    let staging_slice = guard.bufs.staging_buf.slice(..buf_size);
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
            guard.bufs.staging_buf.unmap();
            Ok(true)
        }
        _ => {
            guard.bufs.staging_buf.unmap();
            GPU_AVAILABLE.store(false, Ordering::Relaxed);
            Ok(false)
        }
    }
}

// ---------------------------------------------------------------------------
// Internal: context initialization
// ---------------------------------------------------------------------------

fn get_or_init_ctx() -> Result<&'static Mutex<GpuCtx>, String> {
    static INIT_LOCK: Mutex<()> = Mutex::new(());

    if let Some(ctx) = GPU_CTX.get() {
        return Ok(ctx);
    }

    let _guard = INIT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ctx) = GPU_CTX.get() {
        return Ok(ctx);
    }

    let (device, _queue) = super::get_or_init_shared_device()?;
    let (pipeline_cell, pipeline_fill) = create_pipelines(device)?;
    let bufs = create_buffers(device, 256, 256);

    let _ = GPU_CTX.set(Mutex::new(GpuCtx {
        pipeline_cell,
        pipeline_fill,
        bufs,
        bind_group_cell: None,
        bind_group_fill: None,
    }));
    Ok(GPU_CTX.get().unwrap())
}

fn create_pipelines(
    device: &wgpu::Device,
) -> Result<(wgpu::ComputePipeline, wgpu::ComputePipeline), String> {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pixel_art"),
        source: super::load_shader(include_str!("../shaders/pixel_art.wgsl")),
    });

    let pipeline_cell = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("pixel_art_cell"),
        layout: None,
        module: &shader,
        entry_point: Some("cell_average"),
        compilation_options: Default::default(),
        cache: None,
    });

    let pipeline_fill = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("pixel_art_fill"),
        layout: None,
        module: &shader,
        entry_point: Some("fill"),
        compilation_options: Default::default(),
        cache: None,
    });

    Ok((pipeline_cell, pipeline_fill))
}

fn create_buffers(device: &wgpu::Device, width: u32, height: u32) -> GpuBuffers {
    let n_pixels = (width * height) as u64;
    let buf_size = n_pixels * 4; // frame-sized buffers

    // cell_colors worst case: pixel_size=1 → one cell per pixel
    let cell_buf_size = buf_size;

    GpuBuffers {
        src_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixel_art_src"),
            size: buf_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        uniform_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixel_art_uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        dst_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixel_art_dst"),
            size: buf_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }),
        staging_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixel_art_staging"),
            size: buf_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        cell_colors_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixel_art_cell_colors"),
            size: cell_buf_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        cell_staging_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixel_art_cell_staging"),
            size: cell_buf_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        width,
        height,
    }
}
