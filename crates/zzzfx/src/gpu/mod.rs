pub mod ambient_light;
pub mod cast_shadow;
pub mod ass_glyph;
pub mod ass_subtitle;
pub mod chroma_key;
pub mod device;
pub mod halftone;
pub mod midi_display;
pub mod multitone;
pub mod pixel_art;
pub mod repeater;
pub mod sprite_sheet;

use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::{atomic::AtomicBool, atomic::Ordering, OnceLock};

use crate::settings::stroke::Stroke;

// ---------------------------------------------------------------------------
// Shared GPU device — from crate::gpu::device.
// ---------------------------------------------------------------------------

pub use crate::gpu::device::{get_or_init_shared_device, try_init_shared_device, is_shared_device_ready, blocking_readback};
use crate::gpu::device::SHARED_GPU_AVAILABLE;

/// Load a WGSL shader by prepending the shared function definitions.
pub fn load_shader(specific: &'static str) -> wgpu::ShaderSource<'static> {
    let shared = include_str!("../shaders/shared.wgsl");
    wgpu::ShaderSource::Wgsl(Cow::Owned(format!("{shared}\n{specific}")))
}

// ---------------------------------------------------------------------------
// Uniform buffer matching the WGSL Uniforms struct layout
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct StrokeUniforms {
    width: u32,
    height: u32,
    max_dim: f32,
    stroke_width_px: f32,
    feather_px: f32,
    stroke_a: f32,
    stroke_r: f32,
    stroke_g: f32,
    stroke_b: f32,
    alpha_threshold: f32,
    edge_blend: f32,
    source_opacity: f32,
    stroke_position: u32,
    fill_mode: u32,
    blend_mode: u32,
    use_sharp_corners: u32,
    grad_start_x: f32,
    grad_start_y: f32,
    grad_end_x: f32,
    grad_end_y: f32,
    grad_start_r: f32,
    grad_start_g: f32,
    grad_start_b: f32,
    _pad0: u32,
    grad_end_r: f32,
    grad_end_g: f32,
    grad_end_b: f32,
    _pad1: u32,
}

// ---------------------------------------------------------------------------
// Read-only pipelines — shared without Mutex
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

struct StrokeRes {
    pipeline_mask: wgpu::ComputePipeline,
    pipeline_jfa: wgpu::ComputePipeline,
    pipeline_compose: wgpu::ComputePipeline,
    mask_layout: wgpu::BindGroupLayout,
    jfa_layout: wgpu::BindGroupLayout,
    compose_layout: wgpu::BindGroupLayout,
}

static STROKE_RES: OnceLock<StrokeRes> = OnceLock::new();

fn get_res(device: &wgpu::Device) -> Result<&'static StrokeRes, String> {
    if let Some(r) = STROKE_RES.get() { return Ok(r); }
    let shader_mask = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mask"),
        source: load_shader(include_str!("../shaders/mask.wgsl")),
    });
    let shader_jfa = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("jfa"),
        source: load_shader(include_str!("../shaders/jfa.wgsl")),
    });
    let shader_compose = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("compose"),
        source: load_shader(include_str!("../shaders/compose.wgsl")),
    });
    let p_mask = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("mask"), layout: None, module: &shader_mask, entry_point: Some("main"),
        compilation_options: Default::default(), cache: None,
    });
    let p_jfa = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("jfa"), layout: None, module: &shader_jfa, entry_point: Some("main"),
        compilation_options: Default::default(), cache: None,
    });
    let p_compose = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("compose"), layout: None, module: &shader_compose, entry_point: Some("main"),
        compilation_options: Default::default(), cache: None,
    });
    let mask_layout = p_mask.get_bind_group_layout(0);
    let jfa_layout = p_jfa.get_bind_group_layout(0);
    let compose_layout = p_compose.get_bind_group_layout(0);
    let r = StrokeRes {
        pipeline_mask: p_mask, pipeline_jfa: p_jfa, pipeline_compose: p_compose,
        mask_layout, jfa_layout, compose_layout,
    };
    STROKE_RES.set(r).map_err(|_| "stroke: init race".to_string())?;
    STROKE_RES.get().ok_or_else(|| "stroke: init race".to_string())
}

// ---------------------------------------------------------------------------
// Per-thread buffer pool — no Mutex, no contention
// ---------------------------------------------------------------------------

struct StrokeBufs {
    src_buf: wgpu::Buffer,
    #[allow(dead_code)]
    seeds_a: wgpu::Buffer,
    #[allow(dead_code)]
    seeds_b: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    mask_uniform_buf: wgpu::Buffer,
    bg_mask: wgpu::BindGroup,
    bg_jfa_from_a: wgpu::BindGroup,
    bg_jfa_from_b: wgpu::BindGroup,
    bg_compose_a: wgpu::BindGroup,
    bg_compose_b: wgpu::BindGroup,
    width: u32,
    height: u32,
}

thread_local! {
    static BUF_POOL: RefCell<Option<StrokeBufs>> = const { RefCell::new(None) };
}

fn take_or_create_bufs(device: &wgpu::Device, res: &StrokeRes, w: u32, h: u32) -> StrokeBufs {
    BUF_POOL.with(|cell| {
        let mut bufs = cell.borrow_mut().take();
        if bufs.as_ref().map_or(true, |b| b.width != w || b.height != h) {
            bufs = Some(create_bufs(device, res, w, h));
        }
        bufs.unwrap()
    })
}

fn return_bufs(bufs: StrokeBufs) {
    let _ = BUF_POOL.try_with(|cell| { *cell.borrow_mut() = Some(bufs); });
}

fn create_bufs(device: &wgpu::Device, res: &StrokeRes, width: u32, height: u32) -> StrokeBufs {
    let n_pixels = (width * height) as u64;
    let src_size = n_pixels * 4;
    let seeds_size = n_pixels * 4;
    let uniform_size = std::mem::size_of::<StrokeUniforms>() as u64;
    let mask_uniform_size = std::mem::size_of::<MaskUniforms>() as u64;

    let src_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("src"), size: src_size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let seeds_a = device.create_buffer(&wgpu::BufferDescriptor { label: Some("seeds_a"), size: seeds_size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
    let seeds_b = device.create_buffer(&wgpu::BufferDescriptor { label: Some("seeds_b"), size: seeds_size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
    let dst_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("dst"), size: src_size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("staging"), size: src_size, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("uniforms"), size: uniform_size.max(std::mem::size_of::<JfaUniforms>() as u64), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let mask_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("mask_uniforms"), size: mask_uniform_size, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });

    let bg_mask = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mask"), layout: &res.mask_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: src_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: mask_uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: seeds_a.as_entire_binding() },
        ],
    });
    let bg_jfa_from_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("jfa_a_to_b"), layout: &res.jfa_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: seeds_a.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: seeds_b.as_entire_binding() },
        ],
    });
    let bg_jfa_from_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("jfa_b_to_a"), layout: &res.jfa_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: seeds_b.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: seeds_a.as_entire_binding() },
        ],
    });
    let bg_compose_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("compose_a"), layout: &res.compose_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: src_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: seeds_a.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: dst_buf.as_entire_binding() },
        ],
    });
    let bg_compose_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("compose_b"), layout: &res.compose_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: src_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: seeds_b.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: dst_buf.as_entire_binding() },
        ],
    });

    StrokeBufs { src_buf, seeds_a, seeds_b, dst_buf, staging_buf, uniform_buf, mask_uniform_buf, bg_mask, bg_jfa_from_a, bg_jfa_from_b, bg_compose_a, bg_compose_b, width, height }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Try to render the stroke effect on GPU.
/// Returns `Ok(true)` if GPU rendering succeeded.
/// Returns `Ok(false)` if GPU is unavailable (caller should fall back to CPU).
pub fn try_gpu_render(
    settings: &Stroke,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed) || !SHARED_GPU_AVAILABLE.load(Ordering::Relaxed) {
        return Ok(false);
    }

    let w = width as u32;
    let h = height as u32;

    let (device, queue) = get_or_init_shared_device()?;
    let res = get_res(device).map_err(|e| { GPU_AVAILABLE.store(false, Ordering::Relaxed); e })?;
    let bufs = take_or_create_bufs(device, res, w, h);

    let uniforms = build_uniforms(settings, w, h);
    let buf_size = (w * h * 4) as u64;
    let src_data = &src[..buf_size as usize];

    queue.write_buffer(&bufs.src_buf, 0, src_data);

    let mask_uniforms = MaskUniforms { width: w, height: h, alpha_threshold: uniforms.alpha_threshold, _pad: 0 };
    queue.write_buffer(&bufs.mask_uniform_buf, 0, bytemuck::bytes_of(&mask_uniforms));
    queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    let workgroup_count_x = (w + 15) / 16;
    let workgroup_count_y = (h + 15) / 16;
    let max_dim = w.max(h);
    let n = max_dim.next_power_of_two();
    let jfa_passes = n.ilog2();

    // Stage 1+2: Mask + edge detection + JFA init
    {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            pass.set_pipeline(&res.pipeline_mask);
            pass.set_bind_group(0, &bufs.bg_mask, &[]);
            pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    // Stage 3: JFA passes — one submit per pass so uniforms are visible
    for pass_idx in 0..jfa_passes {
        let step = n >> (pass_idx + 1);
        let jfa_uniforms = JfaUniforms { width: w, height: h, step, use_sharp_corners: if settings.use_sharp_corners { 1 } else { 0 } };
        queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&jfa_uniforms));

        let bind_group = if pass_idx % 2 == 0 { &bufs.bg_jfa_from_a } else { &bufs.bg_jfa_from_b };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            pass.set_pipeline(&res.pipeline_jfa);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    // Stage 4+5: Stroke composition + copy to staging
    {
        queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
        let compose_bg = if jfa_passes % 2 == 0 { &bufs.bg_compose_a } else { &bufs.bg_compose_b };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            pass.set_pipeline(&res.pipeline_compose);
            pass.set_bind_group(0, compose_bg, &[]);
            pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
        }
        encoder.copy_buffer_to_buffer(&bufs.dst_buf, 0, &bufs.staging_buf, 0, buf_size);
        queue.submit(std::iter::once(encoder.finish()));
    }

    let result = blocking_readback(device, &bufs.staging_buf, buf_size, &mut dst[..buf_size as usize]);
    return_bufs(bufs);
    result.map(|()| true)
}

// ---------------------------------------------------------------------------
// Internal: uniform construction
// ---------------------------------------------------------------------------

fn build_uniforms(settings: &Stroke, width: u32, height: u32) -> StrokeUniforms {
    let max_dim = width.max(height) as f32;
    let sw = settings.stroke_width.clamp(0.0, 100.0);
    let w_px = (sw * max_dim) / 1000.0;
    let feather = settings.stroke_feathering.clamp(0.0, 1.0);
    let feather_px = feather * w_px;

    let (grad_start_x, grad_start_y, grad_end_x, grad_end_y) =
        if let Some(ref g) = settings.gradient {
            (g.start_x, g.start_y, g.end_x, g.end_y)
        } else {
            (0.0, 0.0, 1.0, 1.0)
        };

    let (gsr, gsg, gsb) = if let Some(ref g) = settings.gradient {
        (g.start_color_r, g.start_color_g, g.start_color_b)
    } else {
        (0.0, 0.0, 0.0)
    };

    let (ger, geg, geb) = if let Some(ref g) = settings.gradient {
        (g.end_color_r, g.end_color_g, g.end_color_b)
    } else {
        (1.0, 1.0, 1.0)
    };

    StrokeUniforms {
        width, height, max_dim,
        stroke_width_px: w_px, feather_px,
        stroke_a: settings.stroke_color_a.clamp(0.0, 1.0),
        stroke_r: settings.stroke_color_r.clamp(0.0, 1.0),
        stroke_g: settings.stroke_color_g.clamp(0.0, 1.0),
        stroke_b: settings.stroke_color_b.clamp(0.0, 1.0),
        alpha_threshold: settings.alpha_threshold.clamp(0.0, 1.0),
        edge_blend: settings.edge_blend.clamp(0.0, 1.0),
        source_opacity: settings.source_opacity.clamp(0.0, 1.0),
        stroke_position: settings.stroke_position as u32,
        fill_mode: settings.fill_mode as u32,
        blend_mode: settings.blend_mode as u32,
        use_sharp_corners: if settings.use_sharp_corners { 1 } else { 0 },
        grad_start_x, grad_start_y, grad_end_x, grad_end_y,
        grad_start_r: gsr, grad_start_g: gsg, grad_start_b: gsb,
        _pad0: 0,
        grad_end_r: ger, grad_end_g: geg, grad_end_b: geb,
        _pad1: 0,
    }
}

// ---------------------------------------------------------------------------
// Mask uniform (matches mask.wgsl layout)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MaskUniforms {
    width: u32,
    height: u32,
    alpha_threshold: f32,
    _pad: u32,
}

// ---------------------------------------------------------------------------
// JFA uniform (smaller struct for JFA passes)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct JfaUniforms {
    width: u32,
    height: u32,
    step: u32,
    use_sharp_corners: u32,
}
