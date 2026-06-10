//! GPU-accelerated HalfTone rendering via single-pass wgpu compute shader.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::settings::halftone::HalfTone;

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    width: u32,
    height: u32,
    cell_spacing: f32,
    half_cell: f32,
    cos0: f32, sin0: f32,
    cos1: f32, sin1: f32,
    cos2: f32, sin2: f32,
    cos3: f32, sin3: f32,
    ax: f32, ay: f32,
    dot_shape: u32,
    channel_mode: u32,
    invert: u32,
    contrast_factor: f32,
    smoothness: f32,
    blend: f32,
}

struct GpuCtx {
    pipeline: wgpu::ComputePipeline,
    bind_group: Option<wgpu::BindGroup>,
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

static GPU_CTX: OnceLock<Mutex<GpuCtx>> = OnceLock::new();

pub(crate) fn try_render(
    settings: &HalfTone,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
) -> Result<bool, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        try_render_inner(settings, src, dst, width, height)
    }))
    .unwrap_or(Err("halftone GPU render panicked".into()))
}

fn try_render_inner(
    settings: &HalfTone,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed) {
        return Ok(false);
    }
    let w = width as u32;
    let h = height as u32;

    // Compute all uniforms on CPU side
    let dot_size = settings.dot_size.clamp(0.0, 100.0);
    let diagonal = ((w as f32 * w as f32 + h as f32 * h as f32)).sqrt();
    let cell_spacing = (dot_size / 100.0 * diagonal).max(2.0);
    let half_cell = cell_spacing * 0.5;
    let ax = settings.position_x.clamp(0.0, 1.0) * w as f32;
    let ay = settings.position_y.clamp(0.0, 1.0) * h as f32;

    let rad = settings.angle.clamp(0.0, 360.0).to_radians();
    let cos0 = rad.cos();
    let sin0 = rad.sin();

    let a1 = rad + 15f32.to_radians();
    let a2 = rad + 45f32.to_radians();
    let a3 = rad + 75f32.to_radians();

    let smoothness = (settings.smoothness.clamp(0.0, 1.0) * 0.5).max(0.001);
    let contrast = settings.contrast.clamp(0.0, 1.0);
    let contrast_factor = 1.0 + (contrast - 0.5) * 2.0;

    let uniforms = Uniforms {
        width: w,
        height: h,
        cell_spacing,
        half_cell,
        cos0, sin0,
        cos1: a1.cos(), sin1: a1.sin(),
        cos2: a2.cos(), sin2: a2.sin(),
        cos3: a3.cos(), sin3: a3.sin(),
        ax, ay,
        dot_shape: settings.dot_shape as u32,
        channel_mode: settings.channel_mode as u32,
        invert: if settings.invert { 1 } else { 0 },
        contrast_factor,
        smoothness,
        blend: settings.blend_with_original.clamp(0.0, 1.0),
    };

    let ctx = get_or_init_ctx()?;
    let mut guard = match ctx.lock() {
        Ok(g) => g,
        Err(_) => return Ok(false),
    };

    let buf_size = (w * h * 4) as u64;
    let src_data = &src[..(buf_size as usize).min(src.len())];

    let (device, queue) = super::get_or_init_shared_device()?;

    if guard.bufs.width != w || guard.bufs.height != h {
        guard.bufs = create_buffers(device, w, h);
        guard.bind_group = None;
    }

    queue.write_buffer(&guard.bufs.src_buf, 0, src_data);
    queue.write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    if guard.bind_group.is_none() {
        guard.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("halftone"),
            layout: &guard.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: guard.bufs.src_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: guard.bufs.uniform_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: guard.bufs.dst_buf.as_entire_binding() },
            ],
        }));
    }

    {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("halftone") });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("halftone"), timestamp_writes: None });
            pass.set_pipeline(&guard.pipeline);
            pass.set_bind_group(0, guard.bind_group.as_ref().expect("halftone: bind group"), &[]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        encoder.copy_buffer_to_buffer(&guard.bufs.dst_buf, 0, &guard.bufs.staging_buf, 0, buf_size);
        queue.submit(std::iter::once(encoder.finish()));
    }

    super::blocking_readback(device, &guard.bufs.staging_buf, buf_size, &mut dst[..buf_size as usize])?;
    Ok(true)
}

fn get_or_init_ctx() -> Result<&'static Mutex<GpuCtx>, String> {
    static INIT_LOCK: Mutex<()> = Mutex::new(());
    if let Some(ctx) = GPU_CTX.get() { return Ok(ctx); }
    let _guard = INIT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ctx) = GPU_CTX.get() { return Ok(ctx); }

    let (device, _queue) = super::get_or_init_shared_device()?;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("halftone"),
        source: super::load_shader(include_str!("../shaders/halftone.wgsl")),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("halftone"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bufs = create_buffers(device, 256, 256);

    let _ = GPU_CTX.set(Mutex::new(GpuCtx { pipeline, bind_group: None, bufs }));
    GPU_CTX.get().ok_or_else(|| "halftone: GPU ctx init race".to_string())
}

fn create_buffers(device: &wgpu::Device, width: u32, height: u32) -> GpuBuffers {
    let buf_size = (width * height * 4) as u64;
    GpuBuffers {
        src_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("halftone_src"), size: buf_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        uniform_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("halftone_uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        dst_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("halftone_dst"), size: buf_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }),
        staging_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("halftone_staging"), size: buf_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        width, height,
    }
}
