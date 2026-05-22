use std::sync::{atomic::AtomicBool, atomic::Ordering, Mutex, OnceLock};

use super::get_or_init_shared_device;

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
}

struct GpuBuffers {
    sheet_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    output_w: u32,
    output_h: u32,
    sheet_size: u64,
}

static GPU_CTX: OnceLock<Mutex<GpuContext>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Try to render the sprite on GPU with cropping + scaling + centering.
/// `filter_mode`: 0 = nearest-neighbor, 1 = bilinear.
/// Returns `Ok(true)` if GPU rendering succeeded.
/// Returns `Ok(false)` if GPU is unavailable (caller should fall back to CPU).
pub fn try_sprite_sheet_gpu_render(
    crop_rect: (u32, u32, u32, u32),
    sheet_rgba: &[u8],
    sheet_w: u32,
    sheet_h: u32,
    scale: f32,
    filter_mode: u32,
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

    let sheet_byte_size = (sheet_w * sheet_h * 4) as u64;

    // Recreate buffers if output dimensions or sheet size changed
    if guard.bufs.output_w != dst_w
        || guard.bufs.output_h != dst_h
        || guard.bufs.sheet_size < sheet_byte_size
    {
        guard.bufs = create_buffers(guard.device, dst_w, dst_h, sheet_byte_size);
    }

    let uniforms = SpriteSheetUniforms {
        dst_w,
        dst_h,
        sheet_w,
        sheet_h,
        crop_x: cx,
        crop_y: cy,
        crop_w: cw,
        crop_h: ch,
        scale,
        filter_mode,
    };

    // Upload sheet and uniforms
    guard
        .queue
        .write_buffer(&guard.bufs.sheet_buf, 0, sheet_rgba);
    guard
        .queue
        .write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    // Bind group
    let layout = guard.pipeline.get_bind_group_layout(0);
    let bind_group = guard.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sprite_sheet"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: guard.bufs.sheet_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: guard.bufs.uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: guard.bufs.dst_buf.as_entire_binding(),
            },
        ],
    });

    // Dispatch
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
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups((dst_w + 15) / 16, (dst_h + 15) / 16, 1);
        }
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    // Readback: copy dst_buf → staging, then map and copy to CPU
    let image_size = (dst_w * dst_h * 4) as u64;
    {
        let mut encoder = guard
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(
            &guard.bufs.dst_buf,
            0,
            &guard.bufs.staging_buf,
            0,
            image_size,
        );
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    let staging_slice = guard.bufs.staging_buf.slice(..image_size);
    staging_slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = guard.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
    let mapped = staging_slice.get_mapped_range();
    dst[..image_size as usize].copy_from_slice(&mapped);
    drop(mapped);
    guard.bufs.staging_buf.unmap();

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
    }));
    Ok(GPU_CTX.get().unwrap())
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
    let image_size = (output_w * output_h * 4) as u64;
    let uniform_size = std::mem::size_of::<SpriteSheetUniforms>() as u64;

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
        staging_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging"),
            size: image_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        output_w,
        output_h,
        sheet_size,
    }
}
