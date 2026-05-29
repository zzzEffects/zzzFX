pub mod ambient_light;
pub mod ascii_art;
pub mod ass_glyph;
pub mod ass_subtitle;
#[cfg(feature = "gpu")]
pub mod pixel_art;
pub mod repeater;
pub mod sprite_sheet;

use std::borrow::Cow;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Mutex, OnceLock};

use crate::settings::stroke::ZzzStroke;

// ---------------------------------------------------------------------------
// Shared GPU device — one wgpu instance per process, shared by all effects.
// This avoids crashes from creating multiple GPU backends inside plugin hosts
// (e.g. VEGAS Pro) that already manage their own GPU contexts.
// ---------------------------------------------------------------------------

/// Load a WGSL shader by prepending the shared function definitions.
pub fn load_shader(specific: &'static str) -> wgpu::ShaderSource<'static> {
    let shared = include_str!("../shaders/shared.wgsl");
    wgpu::ShaderSource::Wgsl(Cow::Owned(format!("{shared}\n{specific}")))
}

static SHARED_GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);
static SHARED_DEVICE: OnceLock<wgpu::Device> = OnceLock::new();
static SHARED_QUEUE: OnceLock<wgpu::Queue> = OnceLock::new();

pub fn get_or_init_shared_device() -> Result<(&'static wgpu::Device, &'static wgpu::Queue), String> {
    if let (Some(d), Some(q)) = (SHARED_DEVICE.get(), SHARED_QUEUE.get()) {
        return Ok((d, q));
    }

    if !SHARED_GPU_AVAILABLE.load(Ordering::Relaxed) {
        return Err("GPU unavailable".to_string());
    }

    static INIT_LOCK: Mutex<()> = Mutex::new(());
    let _guard = INIT_LOCK.lock().map_err(|_| "init lock poisoned".to_string())?;

    if let (Some(d), Some(q)) = (SHARED_DEVICE.get(), SHARED_QUEUE.get()) {
        return Ok((d, q));
    }

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .map_err(|e| {
        SHARED_GPU_AVAILABLE.store(false, Ordering::Relaxed);
        format!("adapter request failed: {e}")
    })?;

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("zzz shared GPU"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: Default::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            trace: Default::default(),
        },
    ))
    .map_err(|e| {
        SHARED_GPU_AVAILABLE.store(false, Ordering::Relaxed);
        format!("failed to create GPU device: {e}")
    })?;

    let _ = SHARED_DEVICE.set(device);
    let _ = SHARED_QUEUE.set(queue);

    Ok((SHARED_DEVICE.get().unwrap(), SHARED_QUEUE.get().unwrap()))
}

/// Check if the shared GPU device is already initialized WITHOUT attempting to create one.
/// Safe to call from any context (including VEGAS Pro plugin host) — does not block,
/// does not create resources. Returns false if GPU init was never triggered.
pub(crate) fn is_shared_device_ready() -> bool {
    SHARED_GPU_AVAILABLE.load(Ordering::Relaxed)
        && SHARED_DEVICE.get().is_some()
        && SHARED_QUEUE.get().is_some()
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
// GPU state
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

struct GpuContext {
    device: &'static wgpu::Device,
    queue: &'static wgpu::Queue,
    pipeline_mask: wgpu::ComputePipeline,
    pipeline_jfa: wgpu::ComputePipeline,
    pipeline_compose: wgpu::ComputePipeline,
    bufs: GpuBuffers,
}

struct GpuBuffers {
    src_buf: wgpu::Buffer,
    seeds_a: wgpu::Buffer,
    seeds_b: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    mask_uniform_buf: wgpu::Buffer,
    width: u32,
    height: u32,
    // Cached bind groups — only recreated when buffers are recreated
    bind_groups: Option<(wgpu::BindGroup, wgpu::BindGroup, wgpu::BindGroup, wgpu::BindGroup, wgpu::BindGroup)>,
}

static GPU_CTX: OnceLock<Mutex<GpuContext>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Try to render the stroke effect on GPU.
/// Returns `Ok(true)` if GPU rendering succeeded.
/// Returns `Ok(false)` if GPU is unavailable (caller should fall back to CPU).
pub fn try_gpu_render(
    settings: &ZzzStroke,
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
        guard.bufs = create_buffers(&guard.device, w, h);
        // Invalidate cached bind groups since buffers changed
        guard.bufs.bind_groups = None;
    }

    let uniforms = build_uniforms(settings, w, h);
    let buf_size = (w * h * 4) as u64;
    let src_data = &src[..buf_size as usize];

    // Upload source data (immediate CPU-side)
    guard
        .queue
        .write_buffer(&guard.bufs.src_buf, 0, src_data);

    let mask_uniforms = MaskUniforms {
        width: w,
        height: h,
        alpha_threshold: uniforms.alpha_threshold,
        _pad: 0,
    };
    guard
        .queue
        .write_buffer(&guard.bufs.mask_uniform_buf, 0, bytemuck::bytes_of(&mask_uniforms));

    guard
        .queue
        .write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    // Build or reuse bind groups (only recreate when buffers change dimensions)
    if guard.bufs.bind_groups.is_none() {
        guard.bufs.bind_groups = Some(create_bind_groups(&guard, &guard.device));
    }
    let (bg_mask, bg_jfa_from_a, bg_jfa_from_b, bg_compose_a, bg_compose_b) =
        guard.bufs.bind_groups.as_ref().unwrap();

    let workgroup_count_x = (w + 15) / 16;
    let workgroup_count_y = (h + 15) / 16;

    let max_dim = w.max(h);
    let n = max_dim.next_power_of_two();
    let jfa_passes = n.ilog2();

    // Stage 1+2: Mask + edge detection + JFA init
    {
        let mut encoder =
            guard
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&guard.pipeline_mask);
            pass.set_bind_group(0, bg_mask, &[]);
            pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
        }
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    // Stage 3: JFA passes — one submit per pass so uniforms are visible
    for pass_idx in 0..jfa_passes {
        let step = n >> (pass_idx + 1);
        let jfa_uniforms = JfaUniforms {
            width: w,
            height: h,
            step,
            use_sharp_corners: if settings.use_sharp_corners { 1 } else { 0 },
        };

        guard
            .queue
            .write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&jfa_uniforms));

        let bind_group = if pass_idx % 2 == 0 {
            bg_jfa_from_a
        } else {
            bg_jfa_from_b
        };

        let mut encoder =
            guard
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&guard.pipeline_jfa);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
        }
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    // Stage 4+5: Stroke composition
    {
        guard
            .queue
            .write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

        let compose_bg = if jfa_passes % 2 == 0 {
            bg_compose_a
        } else {
            bg_compose_b
        };

        let mut encoder =
            guard
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&guard.pipeline_compose);
            pass.set_bind_group(0, compose_bg, &[]);
            pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
        }
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    // Readback: copy dst_buf → staging, then map and copy to CPU
    {
        let mut encoder =
            guard
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(&guard.bufs.dst_buf, 0, &guard.bufs.staging_buf, 0, buf_size);
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    // Map staging buffer and copy to CPU dst
    let staging_slice = guard.bufs.staging_buf.slice(..buf_size);
    staging_slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = guard.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
    let mapped = staging_slice.get_mapped_range();
    dst[..buf_size as usize].copy_from_slice(&mapped);
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

    let (device, queue) = get_or_init_shared_device()?;
    let (pipeline_mask, pipeline_jfa, pipeline_compose) = create_pipelines(device)?;
    let bufs = create_buffers(device, 256, 256);

    let _ = GPU_CTX.set(Mutex::new(GpuContext {
        device,
        queue,
        pipeline_mask,
        pipeline_jfa,
        pipeline_compose,
        bufs,
    }));
    Ok(GPU_CTX.get().unwrap())
}

fn create_pipelines(
    device: &wgpu::Device,
) -> Result<
    (
        wgpu::ComputePipeline,
        wgpu::ComputePipeline,
        wgpu::ComputePipeline,
    ),
    String,
> {
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

    let pipeline_mask = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("mask"),
        layout: None, // auto-derived
        module: &shader_mask,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let pipeline_jfa = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("jfa"),
        layout: None,
        module: &shader_jfa,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let pipeline_compose = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("compose"),
        layout: None,
        module: &shader_compose,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    Ok((pipeline_mask, pipeline_jfa, pipeline_compose))
}

// ---------------------------------------------------------------------------
// Internal: buffer management
// ---------------------------------------------------------------------------

fn create_buffers(device: &wgpu::Device, width: u32, height: u32) -> GpuBuffers {
    let n_pixels = (width * height) as u64;
    let src_size = n_pixels * 4;
    let seeds_size = n_pixels * 4;
    let uniform_size = std::mem::size_of::<StrokeUniforms>() as u64;

    GpuBuffers {
        src_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("src"),
            size: src_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        seeds_a: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("seeds_a"),
            size: seeds_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }),
        seeds_b: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("seeds_b"),
            size: seeds_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }),
        dst_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dst"),
            size: src_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }),
        staging_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging"),
            size: src_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        uniform_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: uniform_size.max(std::mem::size_of::<JfaUniforms>() as u64),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        mask_uniform_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mask_uniforms"),
            size: std::mem::size_of::<MaskUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        width,
        height,
        bind_groups: None,
    }
}


// ---------------------------------------------------------------------------
// Internal: uniform construction
// ---------------------------------------------------------------------------

fn build_uniforms(settings: &ZzzStroke, width: u32, height: u32) -> StrokeUniforms {
    let max_dim = width.max(height) as f32;
    let sw = settings.stroke_width.clamp(0.0, 1.0);
    let w_px = (sw / 10.0) * max_dim;
    let feather = settings.stroke_feathering.clamp(0.0, 1.0);
    let feather_px = feather * w_px;

    let (grad_start_x, grad_start_y, grad_end_x, grad_end_y) =
        if let Some(ref g) = settings.gradient {
            (g.start_x, g.start_y, g.end_x, g.end_y)
        } else {
            (0.0, 0.0, 1.0, 1.0)
        };

    let (gsr, gsg, gsb) = if let Some(ref g) = settings.gradient {
        (
            g.start_color_r,
            g.start_color_g,
            g.start_color_b,
        )
    } else {
        (0.0, 0.0, 0.0)
    };

    let (ger, geg, geb) = if let Some(ref g) = settings.gradient {
        (g.end_color_r, g.end_color_g, g.end_color_b)
    } else {
        (1.0, 1.0, 1.0)
    };

    StrokeUniforms {
        width,
        height,
        max_dim,
        stroke_width_px: w_px,
        feather_px,
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
        grad_start_x,
        grad_start_y,
        grad_end_x,
        grad_end_y,
        grad_start_r: gsr,
        grad_start_g: gsg,
        grad_start_b: gsb,
        _pad0: 0,
        grad_end_r: ger,
        grad_end_g: geg,
        grad_end_b: geb,
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

// ---------------------------------------------------------------------------
// Internal: bind group creation
// ---------------------------------------------------------------------------

/// Creates all bind groups. Returns:
/// - mask: reads src, uniforms, writes seeds_a
/// - jfa_from_a: reads seeds_a, uniforms, writes seeds_b
/// - jfa_from_b: reads seeds_b, uniforms, writes seeds_a
/// - compose_a: reads src, uniforms, seeds_a, writes dst (JFA ended on seeds_a as output)
/// - compose_b: reads src, uniforms, seeds_b, writes dst (JFA ended on seeds_b as output)
fn create_bind_groups(
    ctx: &GpuContext,
    device: &wgpu::Device,
) -> (
    wgpu::BindGroup,
    wgpu::BindGroup,
    wgpu::BindGroup,
    wgpu::BindGroup,
    wgpu::BindGroup,
) {
    // Get bind group layouts from pipelines (index 0 for all since we use single bind group)
    let mask_layout = &ctx.pipeline_mask.get_bind_group_layout(0);
    let jfa_layout = &ctx.pipeline_jfa.get_bind_group_layout(0);
    let compose_layout = &ctx.pipeline_compose.get_bind_group_layout(0);

    // Mask bind group: src (0), mask_uniforms (1), seeds_a (2)
    let bg_mask = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mask"),
        layout: mask_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: ctx.bufs.src_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: ctx.bufs.mask_uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: ctx.bufs.seeds_a.as_entire_binding(),
            },
        ],
    });

    // JFA from A→B: reads seeds_a (0), uniforms (1), writes seeds_b (2)
    let bg_jfa_from_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("jfa_a_to_b"),
        layout: jfa_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: ctx.bufs.seeds_a.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: ctx.bufs.uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: ctx.bufs.seeds_b.as_entire_binding(),
            },
        ],
    });

    // JFA from B→A: reads seeds_b (0), uniforms (1), writes seeds_a (2)
    let bg_jfa_from_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("jfa_b_to_a"),
        layout: jfa_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: ctx.bufs.seeds_b.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: ctx.bufs.uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: ctx.bufs.seeds_a.as_entire_binding(),
            },
        ],
    });

    // Compose with seeds_a as JFA output
    let bg_compose_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("compose_a"),
        layout: compose_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: ctx.bufs.src_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: ctx.bufs.uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: ctx.bufs.seeds_a.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: ctx.bufs.dst_buf.as_entire_binding(),
            },
        ],
    });

    // Compose with seeds_b as JFA output
    let bg_compose_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("compose_b"),
        layout: compose_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: ctx.bufs.src_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: ctx.bufs.uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: ctx.bufs.seeds_b.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: ctx.bufs.dst_buf.as_entire_binding(),
            },
        ],
    });

    (
        bg_mask,
        bg_jfa_from_a,
        bg_jfa_from_b,
        bg_compose_a,
        bg_compose_b,
    )
}
