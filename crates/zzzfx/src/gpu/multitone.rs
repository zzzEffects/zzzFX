//! GPU-accelerated MultiTone rendering via single-pass wgpu compute shader.
//! Supports None and Ordered dithering. Floyd-Steinberg falls back to CPU.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use crate::settings::multitone::{MultiTone, ToneDithering};

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    width: u32,
    height: u32,
    levels_i: u32,
    levels_f: f32,
    mode: u32,
    dithering: u32,
    dither_amount: f32,
    edge_softness: f32,
    preserve_lum: u32,
    color_map_enabled: u32,
    shadow_r: f32, shadow_g: f32, shadow_b: f32,
    midtone_r: f32, midtone_g: f32, midtone_b: f32,
    highlight_r: f32, highlight_g: f32, highlight_b: f32,
    midtone_pos: f32,
    cm_blend: f32,
}

// ---------------------------------------------------------------------------
// Read-only pipeline — shared without Mutex
// ---------------------------------------------------------------------------

struct Res {
    pipeline: wgpu::ComputePipeline,
    bg_layout: wgpu::BindGroupLayout,
}

static RES: OnceLock<Res> = OnceLock::new();

fn get_res(device: &wgpu::Device) -> Result<&'static Res, String> {
    if let Some(r) = RES.get() { return Ok(r); }
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("multitone"),
        source: super::load_shader(include_str!("../shaders/multitone.wgsl")),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("multitone"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bg_layout = pipeline.get_bind_group_layout(0);
    RES.set(Res { pipeline, bg_layout }).map_err(|_| "multitone: init race".to_string())?;
    RES.get().ok_or_else(|| "multitone: init race".to_string())
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
    let _ = BUF_POOL.try_with(|cell| { *cell.borrow_mut() = Some(bufs); });
}

fn create_bufs(device: &wgpu::Device, res: &Res, width: u32, height: u32) -> Bufs {
    let buf_size = (width * height * 4) as u64;
    let src_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("multitone_src"), size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("multitone_uniforms"),
        size: std::mem::size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let dst_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("multitone_dst"), size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("multitone_staging"), size: buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("multitone"),
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

pub(crate) fn try_render(
    settings: &MultiTone,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
) -> Result<bool, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        try_render_inner(settings, src, dst, width, height)
    }))
    .unwrap_or(Err("multitone GPU render panicked".into()))
}

fn try_render_inner(
    settings: &MultiTone,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
) -> Result<bool, String> {
    if matches!(settings.dithering, ToneDithering::FloydSteinberg) {
        return Ok(false);
    }
    if !GPU_AVAILABLE.load(Ordering::Relaxed) {
        return Ok(false);
    }
    let w = width as u32;
    let h = height as u32;

    let tone_levels_f = (settings.tone_levels.clamp(2.0, 32.0).floor() as u32).max(2);
    let levels_f = (tone_levels_f - 1) as f32;
    let (cm_enabled, sr, sg, sb, mr, mg, mb, hr, hg, hb, mp, cb) =
        if let Some(ref cm) = settings.color_mapping {
            (1u32,
             cm.shadow_color_r.clamp(0.0, 1.0), cm.shadow_color_g.clamp(0.0, 1.0), cm.shadow_color_b.clamp(0.0, 1.0),
             cm.midtone_color_r.clamp(0.0, 1.0), cm.midtone_color_g.clamp(0.0, 1.0), cm.midtone_color_b.clamp(0.0, 1.0),
             cm.highlight_color_r.clamp(0.0, 1.0), cm.highlight_color_g.clamp(0.0, 1.0), cm.highlight_color_b.clamp(0.0, 1.0),
             cm.midtone_position.clamp(0.0, 1.0),
             cm.blend_with_original.clamp(0.0, 1.0))
        } else {
            (0u32, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 0.5, 0.0)
        };

    let uniforms = Uniforms {
        width: w, height: h,
        levels_i: tone_levels_f, levels_f,
        mode: settings.mode as u32,
        dithering: settings.dithering as u32,
        dither_amount: settings.dithering_amount.clamp(0.0, 1.0),
        edge_softness: settings.edge_softness.clamp(0.0, 1.0),
        preserve_lum: if settings.preserve_luminosity { 1 } else { 0 },
        color_map_enabled: cm_enabled,
        shadow_r: sr, shadow_g: sg, shadow_b: sb,
        midtone_r: mr, midtone_g: mg, midtone_b: mb,
        highlight_r: hr, highlight_g: hg, highlight_b: hb,
        midtone_pos: mp, cm_blend: cb,
    };

    let (device, queue) = super::get_or_init_shared_device()?;
    let res = get_res(device).map_err(|e| { GPU_AVAILABLE.store(false, Ordering::Relaxed); e })?;
    let bufs = take_or_create_bufs(device, res, w, h);

    let buf_size = (w * h * 4) as u64;
    let src_data = &src[..(buf_size as usize).min(src.len())];

    queue.write_buffer(&bufs.src_buf, 0, src_data);
    queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("multitone") });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("multitone"), timestamp_writes: None });
        pass.set_pipeline(&res.pipeline);
        pass.set_bind_group(0, &bufs.bind_group, &[]);
        pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
    }
    encoder.copy_buffer_to_buffer(&bufs.dst_buf, 0, &bufs.staging_buf, 0, buf_size);
    queue.submit(std::iter::once(encoder.finish()));

    let result = super::blocking_readback(device, &bufs.staging_buf, buf_size, &mut dst[..buf_size as usize]);
    return_bufs(bufs);
    result.map(|()| true)
}
