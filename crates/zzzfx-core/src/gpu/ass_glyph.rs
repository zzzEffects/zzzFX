//! GPU-accelerated ASS subtitle glyph compositing.
//!
//! Uploads glyph bitmaps + metadata and composites fill, shadow, and outline
//! directly onto the output buffer via a compute shader.

use std::sync::{atomic::AtomicBool, atomic::Ordering, Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Uniforms + GlyphData (match ass_glyph.wgsl layout)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphUniforms {
    output_width: u32,
    output_height: u32,
    glyph_count: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphGpuData {
    pub glyph_offset: u32,
    pub bitmap_w: u32,
    pub bitmap_h: u32,
    pub pos_x: i32,
    pub pos_y: i32,
    pub fill_color: u32,
    pub outline_color: u32,
    pub shadow_color: u32,
    pub outline_radius: i32,
    pub shadow_dx: f32,
    pub shadow_dy: f32,
    pub flags: u32,
}

// ---------------------------------------------------------------------------
// GPU state
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

struct GpuContext {
    device: &'static wgpu::Device,
    queue: &'static wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bufs: GpuBuffers,
    bind_group: Option<wgpu::BindGroup>,
}

struct GpuBuffers {
    glyphs_buf: wgpu::Buffer,
    bitmaps_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    width: u32,
    height: u32,
}

static GPU_CTX: OnceLock<Mutex<GpuContext>> = OnceLock::new();
static INIT_LOCK: Mutex<()> = Mutex::new(());

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn try_ass_glyph_gpu_composite(
    glyphs: &[GlyphGpuData],
    bitmaps: &[u8],
    output: &mut [u8],
    width: u32,
    height: u32,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed)
        || !super::SHARED_GPU_AVAILABLE.load(Ordering::Relaxed)
        || !super::is_shared_device_ready()
    {
        return Ok(false);
    }
    if glyphs.is_empty() {
        return Ok(true);
    }

    // Validate output buffer size
    let needed = (width * height * 4) as usize;
    if output.len() < needed {
        return Err("output buffer too small".to_string());
    }

    let ctx = match get_or_init_gpu() {
        Ok(ctx) => ctx,
        Err(_) => {
            GPU_AVAILABLE.store(false, Ordering::Relaxed);
            return Ok(false);
        }
    };

    let mut guard = match ctx.lock() {
        Ok(g) => g,
        Err(_) => return Ok(false),
    };

    if guard.bufs.width != width || guard.bufs.height != height {
        guard.bufs = create_buffers(guard.device, width, height);
        guard.bind_group = None;
    }

    let uniforms = GlyphUniforms {
        output_width: width,
        output_height: height,
        glyph_count: glyphs.len() as u32,
        _pad: 0,
    };

    let dst_size = (width * height * 4) as u64;

    // Upload glyph metadata (resize buffer if needed)
    let glyphs_bytes = bytemuck::cast_slice(glyphs);
    if glyphs_bytes.len() as u64 > guard.bufs.glyphs_buf.size() {
        guard.bufs.glyphs_buf = guard.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ass_glyphs_meta"),
            size: (glyphs_bytes.len() as u64).next_multiple_of(256).max(8192),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        guard.bind_group = None;
    }
    guard
        .queue
        .write_buffer(&guard.bufs.glyphs_buf, 0, glyphs_bytes);

    // Convert bitmaps to u32 for WGSL (u8 not valid in storage buffers)
    let bitmap_words: Vec<u32> = bitmaps
        .chunks(4)
        .map(|chunk| {
            let b0 = chunk.first().copied().unwrap_or(0) as u32;
            let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
            let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
            let b3 = chunk.get(3).copied().unwrap_or(0) as u32;
            b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
        })
        .collect();
    let bitmap_bytes_u32 = bytemuck::cast_slice(&bitmap_words);

    // Upload bitmaps (resize if needed)
    if !bitmap_bytes_u32.is_empty() {
        let needed = bitmap_bytes_u32.len() as u64;
        if needed > guard.bufs.bitmaps_buf.size() {
            guard.bufs.bitmaps_buf = guard.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ass_glyphs_bitmaps"),
                size: needed.next_multiple_of(256).max(65536),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            guard.bind_group = None;
        }
        guard
            .queue
            .write_buffer(&guard.bufs.bitmaps_buf, 0, bitmap_bytes_u32);
    }

    // Upload output (initial state)
    guard
        .queue
        .write_buffer(&guard.bufs.dst_buf, 0, &output[..dst_size as usize]);

    // Upload uniforms
    guard
        .queue
        .write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    // Build or reuse bind group
    if guard.bind_group.is_none() {
        guard.bind_group = Some(guard.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ass_glyph"),
            layout: &guard.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: guard.bufs.uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: guard.bufs.glyphs_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: guard.bufs.bitmaps_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: guard.bufs.dst_buf.as_entire_binding(),
                },
            ],
        }));
    }
    let bg = guard.bind_group.as_ref().unwrap();

    // Dispatch one workgroup per glyph
    {
        let mut encoder = guard
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&guard.pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.dispatch_workgroups(glyphs.len() as u32, 1, 1);
        }
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    // Readback via staging buffer
    {
        let mut encoder = guard
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(
            &guard.bufs.dst_buf, 0,
            &guard.bufs.staging_buf, 0,
            dst_size,
        );
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    // Blocking readback
    {
        let staging_slice = guard.bufs.staging_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        staging_slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = guard.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let _ = rx.recv();
        let mapped = staging_slice.get_mapped_range();
        output[..dst_size as usize].copy_from_slice(&mapped);
    }
    drop(guard);

    Ok(true)
}

// ---------------------------------------------------------------------------
// Internal GPU initialization
// ---------------------------------------------------------------------------

fn get_or_init_gpu() -> Result<&'static Mutex<GpuContext>, String> {
    if let Some(ctx) = GPU_CTX.get() {
        if GPU_AVAILABLE.load(Ordering::Relaxed) {
            return Ok(ctx);
        }
        return Err("GPU previously marked unavailable".to_string());
    }

    let _lock = INIT_LOCK.lock().map_err(|e| format!("Lock error: {e}"))?;
    if let Some(ctx) = GPU_CTX.get() {
        if GPU_AVAILABLE.load(Ordering::Relaxed) {
            return Ok(ctx);
        }
        return Err("GPU unavailable".to_string());
    }

    let (device, queue) = super::get_or_init_shared_device()?;
    let pipeline = create_pipeline(device)?;
    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bufs = create_buffers(device, 256, 256);

    let ctx = Mutex::new(GpuContext {
        device,
        queue,
        pipeline,
        bind_group_layout,
        bufs,
        bind_group: None,
    });

    GPU_CTX.set(ctx).map_err(|_| "GPU already initialized".to_string())?;
    Ok(GPU_CTX.get().unwrap())
}

fn create_pipeline(device: &wgpu::Device) -> Result<wgpu::ComputePipeline, String> {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ass_glyph"),
        source: super::load_shader(include_str!("../shaders/ass_glyph.wgsl")),
    });
    Ok(device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ass_glyph"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    }))
}

fn create_buffers(device: &wgpu::Device, width: u32, height: u32) -> GpuBuffers {
    let buf_size = (width * height * 4) as u64;

    GpuBuffers {
        glyphs_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ass_glyphs_meta"),
            size: 8192,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        bitmaps_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ass_glyphs_bitmaps"),
            size: 65536,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        uniform_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ass_glyph_uniforms"),
            size: std::mem::size_of::<GlyphUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        dst_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ass_glyph_dst"),
            size: buf_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        staging_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ass_glyph_staging"),
            size: buf_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        width,
        height,
    }
}
