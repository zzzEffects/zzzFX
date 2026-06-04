use std::sync::{atomic::AtomicBool, atomic::Ordering, Mutex, OnceLock};

use super::get_or_init_shared_device;

/// Fast non-cryptographic hash for sheet change detection.
fn hash_sheet(data: &[u8]) -> u64 {
    if data.len() < 128 {
        let mut h: u64 = data.len() as u64;
        for &b in data { h = h.wrapping_mul(0x100000001b3).wrapping_add(b as u64); }
        return h;
    }
    let len = data.len();
    let mut h: u64 = len as u64;
    for &b in &data[..64] { h = h.wrapping_mul(0x100000001b3).wrapping_add(b as u64); }
    for &b in &data[len - 64..] { h = h.wrapping_mul(0x100000001b3).wrapping_add(b as u64); }
    h
}

// ---------------------------------------------------------------------------
// GPU-side struct matching sprite_sheet.wgsl
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SpriteSheetUniforms {
    dst_w: u32,
    dst_h: u32,
    sheet_w: u32,
    sheet_h: u32,
    crop_x: u32,
    crop_y: u32,
    crop_w: u32,
    crop_h: u32,
    scale: f32,
    filter_mode: u32, // 0 = nearest, 1 = bilinear
    displacement_x: f32,
    displacement_y: f32,
    rotation_enabled: u32, // 0 or 1
    cos_rotation: f32,
    sin_rotation: f32,
    _pad: [u32; 1], // pad to 16-byte alignment
}

// ---------------------------------------------------------------------------
// GPU state
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

struct GpuContext {
    device: &'static wgpu::Device,
    queue: &'static wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bufs: GpuBuffers,
    bind_group: Option<wgpu::BindGroup>,
    cached_sheet_hash: u64,
}

struct GpuBuffers {
    sheet_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_bufs: [wgpu::Buffer; 2],
    output_w: u32,
    output_h: u32,
    sheet_size: u64,
}

static GPU_CTX: OnceLock<Mutex<GpuContext>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// GPU render for normal mode (crop + scale + center + displacement + rotation).
pub fn try_sprite_sheet_gpu_render(
    crop_rect: (u32, u32, u32, u32),
    sheet_rgba: &[u8],
    sheet_w: u32,
    sheet_h: u32,
    scale: f32,
    filter_mode: u32,
    displacement_x: f32,
    displacement_y: f32,
    _displacement_pixel_based: bool,
    rotation_deg: f32,
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
) -> Result<bool, String> {
    gpu_render_impl(crop_rect, sheet_rgba, sheet_w, sheet_h, scale, filter_mode, displacement_x, displacement_y, rotation_deg, dst, dst_w, dst_h)
}

/// GPU render for selection mode (full sheet scaling + centering + displacement).
pub fn try_selection_mode_gpu_render(
    sheet_rgba: &[u8],
    sheet_w: u32,
    sheet_h: u32,
    fit_scale: f32,
    displacement_x: f32,
    displacement_y: f32,
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
) -> Result<bool, String> {
    gpu_render_impl((0, 0, sheet_w, sheet_h), sheet_rgba, sheet_w, sheet_h, fit_scale, 0, displacement_x, displacement_y, 0.0, dst, dst_w, dst_h)
}

fn gpu_render_impl(
    crop_rect: (u32, u32, u32, u32),
    sheet_rgba: &[u8],
    sheet_w: u32,
    sheet_h: u32,
    scale: f32,
    filter_mode: u32,
    displacement_x: f32,
    displacement_y: f32,
    rotation_deg: f32,
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed) {
        return Ok(false);
    }

    let (cx, cy, cw, ch) = crop_rect;

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

    let sheet_byte_size = sheet_w as u64 * sheet_h as u64 * 4 ;

    // Sanity check: reject zero or absurdly large dimensions
    if dst_w == 0 || dst_h == 0 || sheet_w == 0 || sheet_h == 0 || dst_w > 16384 || dst_h > 16384 {
        return Ok(false);
    }

    // Recreate buffers if output dimensions or sheet size changed
    if guard.bufs.output_w != dst_w
        || guard.bufs.output_h != dst_h
        || guard.bufs.sheet_size < sheet_byte_size
    {
        guard.bufs = create_buffers(guard.device, dst_w, dst_h, sheet_byte_size);
        guard.bind_group = None; // Invalidate cached bind group
    }

    // Cache sheet upload: skip if unchanged
    let sheet_hash = hash_sheet(sheet_rgba);
    if guard.cached_sheet_hash != sheet_hash {
        guard.queue.write_buffer(&guard.bufs.sheet_buf, 0, sheet_rgba);
        guard.cached_sheet_hash = sheet_hash;
    }

    let (rot_enabled, cos_r, sin_r) = if rotation_deg != 0.0 {
        let rad = rotation_deg as f64 * std::f64::consts::PI / 180.0;
        (1u32, rad.cos() as f32, rad.sin() as f32)
    } else {
        (0u32, 1.0f32, 0.0f32)
    };
    let uniforms = SpriteSheetUniforms {
        dst_w, dst_h, sheet_w, sheet_h,
        crop_x: cx, crop_y: cy, crop_w: cw, crop_h: ch,
        scale, filter_mode,
        displacement_x, displacement_y,
        rotation_enabled: rot_enabled,
        cos_rotation: cos_r,
        sin_rotation: sin_r,
        _pad: [0],
    };
    guard.queue.write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    // Bind group (cached across frames)
    if guard.bind_group.is_none() {
        let layout = guard.pipeline.get_bind_group_layout(0);
        guard.bind_group = Some(guard.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite_sheet"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: guard.bufs.sheet_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: guard.bufs.uniform_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: guard.bufs.dst_buf.as_entire_binding() },
            ],
        }));
    }
    let bind_group = guard.bind_group.as_ref().ok_or("bind group not initialized")?;

    let image_size = dst_w as u64 * dst_h as u64 * 4;

    // Single encoder: compute dispatch + copy to staging
    {
        let mut encoder = guard.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None, timestamp_writes: None,
            });
            pass.set_pipeline(&guard.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups((dst_w + 15) / 16, (dst_h + 15) / 16, 1);
        }
        encoder.copy_buffer_to_buffer(&guard.bufs.dst_buf, 0, &guard.bufs.staging_bufs[0], 0, image_size);
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    // Blocking readback
    let staging = &guard.bufs.staging_bufs[0];
    staging.slice(..image_size).map_async(wgpu::MapMode::Read, |_| {});
    let _ = guard.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_millis(100)),
    });
    let mapped = staging.slice(..image_size).get_mapped_range();
    dst[..image_size as usize].copy_from_slice(&mapped);
    drop(mapped);
    staging.unmap();

    Ok(true)
}

// ---------------------------------------------------------------------------
// Internal: initialization
// ---------------------------------------------------------------------------

fn get_or_init_gpu() -> Result<&'static Mutex<GpuContext>, String> {
    if let Some(ctx) = GPU_CTX.get() {
        return Ok(ctx);
    }

    static INIT_LOCK: Mutex<()> = Mutex::new(());
    let _guard = INIT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ctx) = GPU_CTX.get() {
        return Ok(ctx);
    }

    let (device, queue) = get_or_init_shared_device()?;
    let pipeline = create_pipeline(device)?;
    let bufs = create_buffers(device, 256, 256, 256 * 256 * 4);

    let _ = GPU_CTX.set(Mutex::new(GpuContext {
        device,
        queue,
        pipeline,
        bufs,
        bind_group: None,
        cached_sheet_hash: 0,
    }));
    GPU_CTX
        .get()
        .ok_or_else(|| "sprite_sheet: GPU ctx init race".to_string())
}

fn create_pipeline(device: &wgpu::Device) -> Result<wgpu::ComputePipeline, String> {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sprite_sheet"),
        source: super::load_shader(include_str!("../shaders/sprite_sheet.wgsl")),
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("sprite_sheet"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    Ok(pipeline)
}

// ---------------------------------------------------------------------------
// Internal: buffer management
// ---------------------------------------------------------------------------

fn create_buffers(
    device: &wgpu::Device,
    output_w: u32,
    output_h: u32,
    sheet_size: u64,
) -> GpuBuffers {
    let image_size = output_w as u64 * output_h as u64 * 4;
    let uniform_size = std::mem::size_of::<SpriteSheetUniforms>() as u64;

    let make_staging = |label: &str| device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: image_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    GpuBuffers {
        sheet_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sheet"),
            size: sheet_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        uniform_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: uniform_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        dst_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dst"),
            size: image_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }),
        staging_bufs: [make_staging("staging0"), make_staging("staging1")],
        output_w,
        output_h,
        sheet_size,
    }
}
