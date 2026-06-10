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
//! pixel art settings (e.g. ~8000 cells for 1080p with 16x16 blocks ~ 32 KB).

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use crate::settings::pixel_art::{Dithering, PixelArt};

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
    grid_thickness: f32,
    grid_color_r: f32,
    grid_color_g: f32,
    grid_color_b: f32,
    grid_color_a: f32,
    grid_offset_x: u32,
    grid_offset_y: u32,
    contrast: f32,
    saturation: f32,
    _pad: u32,
}

// ---------------------------------------------------------------------------
// Read-only pipelines — shared without Mutex
// ---------------------------------------------------------------------------

struct Res {
    pipeline_cell: wgpu::ComputePipeline,
    pipeline_fill: wgpu::ComputePipeline,
    cell_layout: wgpu::BindGroupLayout,
    fill_layout: wgpu::BindGroupLayout,
}

static RES: OnceLock<Res> = OnceLock::new();

fn get_res(device: &wgpu::Device) -> Result<&'static Res, String> {
    if let Some(r) = RES.get() {
        return Ok(r);
    }
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
    let cell_layout = pipeline_cell.get_bind_group_layout(0);
    let fill_layout = pipeline_fill.get_bind_group_layout(0);
    RES.set(Res { pipeline_cell, pipeline_fill, cell_layout, fill_layout })
        .map_err(|_| "pixel_art: init race".to_string())?;
    RES.get().ok_or_else(|| "pixel_art: init race".to_string())
}

// ---------------------------------------------------------------------------
// Per-thread buffer pool — no Mutex, no contention
// ---------------------------------------------------------------------------

struct Bufs {
    src_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    cell_colors_buf: wgpu::Buffer,
    cell_staging_buf: wgpu::Buffer,
    bind_group_cell: wgpu::BindGroup,
    bind_group_fill: wgpu::BindGroup,
    width: u32,
    height: u32,
}

thread_local! {
    static BUF_POOL: RefCell<Option<Bufs>> = const { RefCell::new(None) };
}

fn take_or_create_bufs(device: &wgpu::Device, res: &Res, w: u32, h: u32) -> Bufs {
    BUF_POOL.with(|cell| {
        let mut bufs = cell.borrow_mut().take();
        if bufs.as_ref().map_or(true, |b| b.width != w || b.height != h) {
            bufs = Some(create_bufs(device, res, w, h));
        }
        bufs.unwrap()
    })
}

fn return_bufs(bufs: Bufs) {
    let _ = BUF_POOL.try_with(|cell| {
        *cell.borrow_mut() = Some(bufs);
    });
}

fn create_bufs(device: &wgpu::Device, res: &Res, width: u32, height: u32) -> Bufs {
    let n_pixels = (width * height) as u64;
    let buf_size = n_pixels * 4;
    let cell_buf_size = buf_size; // worst case: pixel_size=1 -> one cell per pixel

    let src_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pixel_art_src"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pixel_art_uniforms"),
        size: std::mem::size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
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
    let cell_colors_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pixel_art_cell_colors"),
        size: cell_buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let cell_staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pixel_art_cell_staging"),
        size: cell_buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group_cell = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pixel_art_cell"),
        layout: &res.cell_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: src_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: cell_colors_buf.as_entire_binding() },
        ],
    });
    let bind_group_fill = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pixel_art_fill"),
        layout: &res.fill_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: dst_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: cell_colors_buf.as_entire_binding() },
        ],
    });

    Bufs { src_buf, uniform_buf, dst_buf, staging_buf, cell_colors_buf, cell_staging_buf, bind_group_cell, bind_group_fill, width, height }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub(crate) fn try_render(
    settings: &PixelArt,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
) -> Result<bool, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        try_render_inner(settings, src, dst, width, height)
    }))
    .unwrap_or(Err("GPU render panicked".into()))
}

fn try_render_inner(
    settings: &PixelArt,
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

    let px = settings.grid_position_x.clamp(0.0, 1.0);
    let py = settings.grid_position_y.clamp(0.0, 1.0);
    let ox = px * w as f32 - (px * w as f32 / pixel_size_w as f32).round() * pixel_size_w as f32;
    let oy = py * h as f32 - (py * h as f32 / pixel_size_h as f32).round() * pixel_size_h as f32;
    let off_x = ox.rem_euclid(pixel_size_w as f32).round() as u32 % pixel_size_w;
    let off_y = oy.rem_euclid(pixel_size_h as f32).round() as u32 % pixel_size_h;

    let cols = if off_x > 0 { (w - off_x).div_ceil(pixel_size_w) + 1 } else { w.div_ceil(pixel_size_w) };
    let rows = if off_y > 0 { (h - off_y).div_ceil(pixel_size_h) + 1 } else { h.div_ceil(pixel_size_h) };

    // Fall back to CPU when cell count is too low for efficient GPU occupancy.
    let workgroups = cols.div_ceil(8) * rows.div_ceil(8);
    if workgroups < 4 {
        return Ok(false);
    }

    let is_fs = matches!(settings.dithering, Dithering::FloydSteinberg);

    let (device, queue) = super::get_or_init_shared_device()?;
    let res = get_res(device).map_err(|e| { GPU_AVAILABLE.store(false, Ordering::Relaxed); e })?;
    let bufs = take_or_create_bufs(device, res, w, h);

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
        grid_thickness: settings.grid_thickness.clamp(0.0, 1.0),
        grid_color_r: settings.grid_color_r.clamp(0.0, 1.0),
        grid_color_g: settings.grid_color_g.clamp(0.0, 1.0),
        grid_color_b: settings.grid_color_b.clamp(0.0, 1.0),
        grid_color_a: settings.grid_color_a.clamp(0.0, 1.0),
        grid_offset_x: off_x,
        grid_offset_y: off_y,
        contrast: settings.contrast.clamp(0.0, 1.0),
        saturation: settings.saturation.clamp(0.0, 1.0),
        _pad: 0,
    };

    let buf_size = (w * h * 4) as u64;
    let cell_buf_size = (cols * rows * 4) as u64;
    let src_data = &src[..(buf_size as usize).min(src.len())];

    // Upload source and uniforms
    queue.write_buffer(&bufs.src_buf, 0, src_data);
    queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    // ── Pass 1: cell averaging ──────────────────────────────────────────

    {
        let mut encoder = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("pixel_art_cell") },
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("cell_average"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&res.pipeline_cell);
            pass.set_bind_group(0, &bufs.bind_group_cell, &[]);
            let wg_x = cols.div_ceil(8);
            let wg_y = rows.div_ceil(8);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    // ── Floyd-Steinberg hybrid: read back cells, diffuse on CPU, write back ──

    if is_fs {
        // Read back cell colors into cell_staging_buf
        {
            let mut encoder = device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("pixel_art_fs_readback") },
            );
            encoder.copy_buffer_to_buffer(
                &bufs.cell_colors_buf, 0,
                &bufs.cell_staging_buf, 0,
                cell_buf_size,
            );
            queue.submit(std::iter::once(encoder.finish()));
        }

        // Use a thread-local buffer for cell staging readback
        thread_local! {
            static CELL_STAGING: RefCell<Vec<u8>> = RefCell::new(Vec::new());
        }
        let cell_readback_result = CELL_STAGING.with(|cs| {
            let mut cs = cs.borrow_mut();
            cs.resize(cell_buf_size as usize, 0);
            super::blocking_readback(device, &bufs.cell_staging_buf, cell_buf_size, &mut cs)
        });

        match cell_readback_result {
            Ok(()) => {
                CELL_STAGING.with(|cs| {
                    let cs = cs.borrow();
                    let num_cells = (cols * rows) as usize;
                    let mut cells: Vec<[f32; 4]> = Vec::with_capacity(num_cells);
                    for i in 0..num_cells {
                        let base = i * 4;
                        let r = cs[base] as f32 / 255.0;
                        let g = cs[base + 1] as f32 / 255.0;
                        let b = cs[base + 2] as f32 / 255.0;
                        let a = cs[base + 3] as f32 / 255.0;
                        cells.push([r, g, b, a]);
                    }

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
                    queue.write_buffer(&bufs.cell_colors_buf, 0, &cell_bytes);
                });
            }
            Err(_) => {
                GPU_AVAILABLE.store(false, Ordering::Relaxed);
                // Return bufs before bailing
                return_bufs(bufs);
                return Ok(false);
            }
        }
    }

    // ── Pass 2: fill output pixels ────────────────────────────────────────

    {
        let mut encoder = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("pixel_art_fill") },
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("fill"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&res.pipeline_fill);
            pass.set_bind_group(0, &bufs.bind_group_fill, &[]);
            let wg_x = w.div_ceil(8);
            let wg_y = h.div_ceil(8);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }
        encoder.copy_buffer_to_buffer(&bufs.dst_buf, 0, &bufs.staging_buf, 0, buf_size);
        queue.submit(std::iter::once(encoder.finish()));
    }

    // ── Readback output ────────────────────────────────────────────────────

    let result = super::blocking_readback(device, &bufs.staging_buf, buf_size, &mut dst[..buf_size as usize]);
    return_bufs(bufs);
    result.map(|()| true)
}
