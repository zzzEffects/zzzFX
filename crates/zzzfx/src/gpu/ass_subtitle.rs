//! GPU-accelerated ASS subtitle compositing.
//!
//! Composites a pre-rendered source buffer onto the destination output
//! using the selected blend mode via a compute shader.

use std::cell::RefCell;
use std::sync::{atomic::AtomicBool, atomic::Ordering, OnceLock};

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
        label: Some("ass_subtitle"),
        source: super::load_shader(include_str!("../shaders/ass_subtitle.wgsl")),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ass_subtitle"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bg_layout = pipeline.get_bind_group_layout(0);
    RES.set(Res { pipeline, bg_layout }).map_err(|_| "ass_subtitle: init race".to_string())?;
    RES.get().ok_or_else(|| "ass_subtitle: init race".to_string())
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
        label: Some("ass_src"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ass_uniforms"),
        size: std::mem::size_of::<AssUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let dst_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ass_dst"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ass_staging"),
        size: buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ass_composite"),
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
        || !super::is_shared_device_ready()
    {
        return Ok(false);
    }

    // Validate buffer sizes before slicing
    let needed = (width * height * 4) as usize;
    if src.len() < needed || dst.len() < needed {
        return Err("buffer size mismatch".to_string());
    }

    let (device, queue) = super::get_or_init_shared_device()?;
    let res = get_res(device).map_err(|e| { GPU_AVAILABLE.store(false, Ordering::Relaxed); e })?;
    let bufs = take_or_create_bufs(device, res, width, height);

    let uniforms = AssUniforms {
        width,
        height,
        blend_mode,
        _pad: 0,
    };

    let src_size = (width * height * 4) as u64;

    // Upload source
    queue.write_buffer(&bufs.src_buf, 0, &src[..src_size as usize]);

    // Copy destination (initial state) to GPU dst buffer
    queue.write_buffer(&bufs.dst_buf, 0, &dst[..src_size as usize]);

    // Upload uniforms
    queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    // Dispatch
    let wg_x = (width + 15) / 16;
    let wg_y = (height + 15) / 16;
    {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&res.pipeline);
            pass.set_bind_group(0, &bufs.bind_group, &[]);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }
        encoder.copy_buffer_to_buffer(
            &bufs.dst_buf, 0,
            &bufs.staging_buf, 0,
            src_size,
        );
        queue.submit(std::iter::once(encoder.finish()));
    }

    // Readback
    let result = super::blocking_readback(device, &bufs.staging_buf, src_size, &mut dst[..src_size as usize]);
    return_bufs(bufs);
    result.map(|()| true)
}
