use std::sync::{atomic::AtomicBool, atomic::Ordering, Mutex, OnceLock};

use crate::settings::repeater::ZzzRepeater;
use crate::CompositorLayer;

use super::get_or_init_shared_device;

// ---------------------------------------------------------------------------
// GPU-side structs matching repeater.wgsl
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct RepeaterUniforms {
    width: u32,
    height: u32,
    blend_mode: u32, // 0-21
    center_x: f32,
    center_y: f32,
    offset_x: f32,
    offset_y: f32,
    cos_a: f32,
    sin_a: f32,
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

/// Try to render the repeater effect on GPU using multi-pass compositing.
/// Each layer is dispatched as a separate compute pass within a single encoder + submit.
/// Returns `Ok(true)` if GPU rendering succeeded.
/// Returns `Ok(false)` if GPU is unavailable (caller should fall back to CPU).
pub fn try_repeater_gpu_render(
    settings: &ZzzRepeater,
    layers: &[CompositorLayer],
    dst: &mut [u8],
    width: usize,
    height: usize,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed) {
        return Ok(false);
    }

    let w = width as u32;
    let h = height as u32;

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

    if guard.bufs.width != w || guard.bufs.height != h {
        guard.bufs = create_buffers(guard.device, w, h);
        guard.bind_group = None; // Invalidate cached bind group
    }

    let center_x = w as f32 * 0.5;
    let center_y = h as f32 * 0.5;
    let wf = w as f32;
    let hf = h as f32;
    let blend_mode = settings.blend_mode as u32;
    let image_size = (w * h * 4) as u64;

    // Precompute layer params
    let layer_params: Vec<(f32, f32, f32, f32)> = layers
        .iter()
        .map(|layer| {
            let offset_x = (layer.position_x - 0.5) * wf;
            let offset_y = (layer.position_y - 0.5) * hf;
            let angle_rad = (-layer.rotation_deg).to_radians();
            let (sin_a, cos_a) = angle_rad.sin_cos();
            (offset_x, offset_y, cos_a, sin_a)
        })
        .collect();

    let is_below = matches!(settings.layer_order, crate::settings::repeater::LayerOrder::Below);

    // Zero out dst buffer before first dispatch
    guard.queue.write_buffer(&guard.bufs.dst_buf, 0, &vec![0u8; image_size as usize]);

    let workgroup_count_x = (w + 15) / 16;
    let workgroup_count_y = (h + 15) / 16;
    let layout = guard.pipeline.get_bind_group_layout(0);

    // Create or reuse bind group (references buffers by handle, valid across frames)
    if guard.bind_group.is_none() {
        guard.bind_group = Some(guard.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("repeater"),
            layout: &layout,
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
        }));
    }
    let bind_group = guard.bind_group.as_ref().unwrap();

    // One submit per layer so per-layer uniform writes are visible
    for i in 0..layers.len() {
        let li = if is_below { layers.len() - 1 - i } else { i };
        let (offset_x, offset_y, cos_a, sin_a) = layer_params[li];
        let layer = &layers[li];

        let uniforms = RepeaterUniforms {
            width: w,
            height: h,
            blend_mode,
            center_x,
            center_y,
            offset_x,
            offset_y,
            cos_a,
            sin_a,
        };

        // Upload source layer and uniforms
        guard
            .queue
            .write_buffer(&guard.bufs.src_buf, 0, layer.rgba);
        guard
            .queue
            .write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

        let mut encoder = guard
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&guard.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
        }
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    // Readback: copy dst_buf → staging, then map and copy to CPU
    {
        let mut encoder = guard
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(&guard.bufs.dst_buf, 0, &guard.bufs.staging_buf, 0, image_size);
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    // Map staging buffer and copy to CPU dst
    let _ = guard.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });

    let staging_slice = guard.bufs.staging_buf.slice(..image_size);
    let (tx, rx) = std::sync::mpsc::channel();
    staging_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });

    let _ = guard.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });

    let map_result = rx
        .recv()
        .map_err(|_| "GPU staging buffer map callback dropped".to_string())?;

    if let Err(e) = map_result {
        return Err(format!("GPU staging buffer map failed: {:?}", e));
    }

    {
        let mapped = staging_slice.get_mapped_range();
        dst[..image_size as usize].copy_from_slice(&mapped);
    }

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
    let bufs = create_buffers(device, 256, 256);

    let _ = GPU_CTX.set(Mutex::new(GpuContext {
        device,
        queue,
        pipeline,
        bufs,
        bind_group: None,
    }));
    Ok(GPU_CTX.get().unwrap())
}

fn create_pipeline(device: &wgpu::Device) -> Result<wgpu::ComputePipeline, String> {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("repeater"),
        source: super::load_shader(include_str!("../shaders/repeater.wgsl")),
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("repeater"),
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

fn create_buffers(device: &wgpu::Device, width: u32, height: u32) -> GpuBuffers {
    let image_size = (width * height * 4) as u64;
    let uniform_size = std::mem::size_of::<RepeaterUniforms>() as u64;

    GpuBuffers {
        src_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("src"),
            size: image_size,
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
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        staging_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging"),
            size: image_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        width,
        height,
    }
}
