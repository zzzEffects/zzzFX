use std::cell::RefCell;
use std::sync::{atomic::AtomicBool, atomic::Ordering, OnceLock};

use crate::settings::repeater::Repeater;
use crate::CompositorLayer;

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
// Read-only pipeline — shared without Mutex
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

struct Res {
    pipeline: wgpu::ComputePipeline,
    bg_layout: wgpu::BindGroupLayout,
}

static RES: OnceLock<Res> = OnceLock::new();

fn get_res(device: &wgpu::Device) -> Result<&'static Res, String> {
    if let Some(r) = RES.get() {
        return Ok(r);
    }
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
    let bg_layout = pipeline.get_bind_group_layout(0);
    RES.set(Res { pipeline, bg_layout }).map_err(|_| "repeater: init race".to_string())?;
    RES.get().ok_or_else(|| "repeater: init race".to_string())
}

// ---------------------------------------------------------------------------
// Per-thread buffer pool — no Mutex, no contention
// ---------------------------------------------------------------------------

struct Bufs {
    src_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
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
    let buf_size = (width * height * 4) as u64;
    let src_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("repeater_src"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("repeater_uniforms"),
        size: std::mem::size_of::<RepeaterUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let dst_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("repeater_dst"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("repeater_staging"),
        size: buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("repeater"),
        layout: &res.bg_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: src_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: dst_buf.as_entire_binding() },
        ],
    });
    Bufs { src_buf, uniform_buf, dst_buf, staging_buf, bind_group, width, height }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Try to render the repeater effect on GPU using multi-pass compositing.
/// Each layer is dispatched as a separate compute pass within its own submit.
/// Returns `Ok(true)` if GPU rendering succeeded.
/// Returns `Ok(false)` if GPU is unavailable (caller should fall back to CPU).
pub fn try_repeater_gpu_render(
    settings: &Repeater,
    layers: &[CompositorLayer],
    dst: &mut [u8],
    width: usize,
    height: usize,
) -> Result<bool, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        try_repeater_inner(settings, layers, dst, width, height)
    }))
    .unwrap_or(Err("repeater GPU render panicked".into()))
}

fn try_repeater_inner(
    settings: &Repeater,
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

    let (device, queue) = super::get_or_init_shared_device()?;
    let res = get_res(device).map_err(|e| { GPU_AVAILABLE.store(false, Ordering::Relaxed); e })?;
    let bufs = take_or_create_bufs(device, res, w, h);

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

    let workgroup_count_x = (w + 15) / 16;
    let workgroup_count_y = (h + 15) / 16;

    // Zero out dst buffer before first dispatch — reuse a thread-local buffer
    thread_local! {
        static ZERO_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    }
    ZERO_BUF.with(|zb| {
        let mut zb = zb.borrow_mut();
        if zb.len() < image_size as usize {
            zb.resize(image_size as usize, 0);
        }
        queue.write_buffer(&bufs.dst_buf, 0, &zb[..image_size as usize]);
    });

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

        queue.write_buffer(&bufs.src_buf, 0, layer.rgba);
        queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&res.pipeline);
            pass.set_bind_group(0, &bufs.bind_group, &[]);
            pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    // Readback: copy dst_buf → staging, then map and copy to CPU
    {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(&bufs.dst_buf, 0, &bufs.staging_buf, 0, image_size);
        queue.submit(std::iter::once(encoder.finish()));
    }

    let result = super::blocking_readback(device, &bufs.staging_buf, image_size, &mut dst[..image_size as usize]);
    return_bufs(bufs);
    result.map(|()| true)
}
