//! GPU-accelerated ASS subtitle compositing.
//!
//! Composites a pre-rendered source buffer onto the destination output
//! using the selected blend mode via a compute shader.

use std::sync::{atomic::AtomicBool, atomic::Ordering, Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Uniforms (matches ass_subtitle.wgsl layout)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct AssUniforms {
    width: u32,
    height: u32,
    blend_mode: u32,
    _pad: u32,
}

// ---------------------------------------------------------------------------
// GPU state
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

struct GpuContext {
    #[allow(dead_code)]
    device: &'static wgpu::Device,
    queue: &'static wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bufs: GpuBuffers,
}

struct GpuBuffers {
    src_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    width: u32,
    height: u32,
}

static GPU_CTX: OnceLock<Mutex<GpuContext>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Try to composite `src` onto `dst` using the GPU.
/// `src` and `dst` are RGBA8 packed buffers of `width * height * 4` bytes.
/// Returns `Ok(true)` if GPU compositing succeeded (dst is updated in-place).
pub fn try_ass_subtitle_gpu_composite(
    src: &[u8],
    dst: &mut [u8],
    width: u32,
    height: u32,
    blend_mode: u32,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed)
        || !super::SHARED_GPU_AVAILABLE.load(Ordering::Relaxed)
    {
        return Ok(false);
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
        guard.bufs = create_buffers(&guard.device, width, height);
    }

    let uniforms = AssUniforms {
        width,
        height,
        blend_mode,
        _pad: 0,
    };

    // Upload source
    let src_size = (width * height * 4) as u64;
    guard
        .queue
        .write_buffer(&guard.bufs.src_buf, 0, &src[..src_size as usize]);

    // Copy destination (initial state) to GPU dst buffer
    guard
        .queue
        .write_buffer(&guard.bufs.dst_buf, 0, &dst[..src_size as usize]);

    // Upload uniforms
    guard
        .queue
        .write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    // Build bind group
    let bg = guard.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ass_composite"),
        layout: &guard.pipeline.get_bind_group_layout(0),
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
                binding: 2,
                resource: guard.bufs.dst_buf.as_entire_binding(),
            },
        ],
    });

    // Dispatch
    let wg_x = (width + 15) / 16;
    let wg_y = (height + 15) / 16;
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
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    // Readback
    {
        let mut encoder = guard
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(
            &guard.bufs.dst_buf,
            0,
            &guard.bufs.staging_buf,
            0,
            src_size,
        );
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    let staging_slice = guard.bufs.staging_buf.slice(..src_size);
    staging_slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = guard.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
    let mapped = staging_slice.get_mapped_range();
    dst[..src_size as usize].copy_from_slice(&mapped);
    drop(mapped);
    guard.bufs.staging_buf.unmap();

    Ok(true)
}

// ---------------------------------------------------------------------------
// Internal: initialization
// ---------------------------------------------------------------------------

fn get_or_init_gpu() -> Result<&'static Mutex<GpuContext>, String> {
    static INIT_LOCK: Mutex<()> = Mutex::new(());

    if let Some(ctx) = GPU_CTX.get() {
        return Ok(ctx);
    }

    let _guard = INIT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ctx) = GPU_CTX.get() {
        return Ok(ctx);
    }

    let (device, queue) = super::get_or_init_shared_device()?;
    let pipeline = create_pipeline(device)?;
    let bufs = create_buffers(device, 256, 256);

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
        label: Some("ass_subtitle"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/ass_subtitle.wgsl").into()),
    });

    Ok(device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ass_subtitle"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    }))
}

fn create_buffers(device: &wgpu::Device, width: u32, height: u32) -> GpuBuffers {
    let buf_size = (width * height * 4) as u64;

    GpuBuffers {
        src_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ass_src"),
            size: buf_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        uniform_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ass_uniforms"),
            size: std::mem::size_of::<AssUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        dst_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ass_dst"),
            size: buf_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }),
        staging_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ass_staging"),
            size: buf_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        width,
        height,
    }
}
