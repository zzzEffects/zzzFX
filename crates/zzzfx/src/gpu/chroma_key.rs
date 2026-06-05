use std::sync::{atomic::AtomicBool, atomic::Ordering, Mutex, OnceLock};

use crate::settings::chroma_key::ChromaKey;

use super::get_or_init_shared_device;

// ---------------------------------------------------------------------------
// Uniforms (must match WGSL struct layout)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct AlphaUniforms {
    width: u32,
    height: u32,
    key_cb: f32,
    key_cr: f32,
    threshold_sq: f32,
    soft_end_sq: f32,
    range_sq: f32,
    edge_softness: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BlurUniforms {
    width: u32,
    height: u32,
    radius: u32,
    horizontal: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CompositeUniforms {
    width: u32,
    height: u32,
    spill_suppression: f32,
    show_matte: u32,
    invert: u32,
}

const UNIFORM_SIZE: u64 = {
    let a = std::mem::size_of::<AlphaUniforms>();
    let b = std::mem::size_of::<BlurUniforms>();
    let c = std::mem::size_of::<CompositeUniforms>();
    let max = if a >= b && a >= c { a } else if b >= c { b } else { c };
    max as u64
};

// ---------------------------------------------------------------------------
// GPU state
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

struct Ctx {
    device: &'static wgpu::Device,
    queue: &'static wgpu::Queue,
    pipeline_alpha: wgpu::ComputePipeline,
    pipeline_blur: wgpu::ComputePipeline,
    pipeline_composite: wgpu::ComputePipeline,
    bufs: Bufs,
    bind_groups: Option<(
        wgpu::BindGroup, // bg_alpha
        wgpu::BindGroup, // bg_blur_a_to_b
        wgpu::BindGroup, // bg_blur_b_to_a
        wgpu::BindGroup, // bg_composite_a
        wgpu::BindGroup, // bg_composite_b
    )>,
}

struct Bufs {
    src_buf: wgpu::Buffer,
    alpha_a: wgpu::Buffer,
    alpha_b: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    w: u32,
    h: u32,
}

static CTX: OnceLock<Mutex<Ctx>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Try to render the chroma key effect on GPU.
/// Returns `Ok(true)` if GPU rendering succeeded.
/// Returns `Ok(false)` if GPU is unavailable (caller should fall back to CPU).
pub fn try_chroma_key_gpu_render(
    settings: &ChromaKey,
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

    let ctx = match get_or_init() {
        Ok(c) => c,
        Err(_) => {
            GPU_AVAILABLE.store(false, Ordering::Relaxed);
            return Ok(false);
        }
    };

    let mut guard = match ctx.lock() {
        Ok(g) => g,
        Err(_) => return Ok(false),
    };

    if guard.bufs.w != w || guard.bufs.h != h {
        guard.bufs = create_bufs(guard.device, w, h);
        guard.bind_groups = None;
    }

    let total = (w * h) as usize;
    let image_size = (total * 4) as u64;
    let wx = (w + 15) / 16;
    let wy = (h + 15) / 16;

    let threshold = settings.threshold.clamp(0.0, 1.0);
    let edge_softness = settings.edge_softness.clamp(0.0, 1.0);
    let spill_suppression = settings.spill_suppression.clamp(0.0, 1.0);
    let edge_blur = settings.edge_blur.clamp(0.0, 20.0);
    let show_matte = settings.show_matte;

    // Pre-compute key color in YCbCr (same BT.601 as CPU)
    let key_r = settings.key_color_r.clamp(0.0, 1.0);
    let key_g = settings.key_color_g.clamp(0.0, 1.0);
    let key_b = settings.key_color_b.clamp(0.0, 1.0);
    let key_cb = -0.168736 * key_r - 0.331264 * key_g + 0.5 * key_b + 0.5;
    let key_cr = 0.5 * key_r - 0.418688 * key_g - 0.081312 * key_b + 0.5;

    let threshold_sq = threshold * threshold;
    let soft_end = (threshold + edge_softness).clamp(0.0, 1.0);
    let soft_end_sq = soft_end * soft_end;
    let range_sq = soft_end_sq - threshold_sq;

    // Upload source
    guard.queue.write_buffer(&guard.bufs.src_buf, 0, &src[..image_size as usize]);

    // Build or reuse bind groups
    if guard.bind_groups.is_none() {
        guard.bind_groups = Some(create_bind_groups(&guard, guard.device));
    }
    let (bg_alpha, bg_blur_a_to_b, bg_blur_b_to_a, bg_composite_a, bg_composite_b) =
        guard.bind_groups.as_ref().ok_or("bind groups not initialized")?;

    // ---- Stage 1: compute_alpha ----
    let alpha_uniforms = AlphaUniforms {
        width: w, height: h,
        key_cb, key_cr,
        threshold_sq, soft_end_sq, range_sq,
        edge_softness,
    };
    guard.queue.write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&alpha_uniforms));

    {
        let mut enc = guard.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            pass.set_pipeline(&guard.pipeline_alpha);
            pass.set_bind_group(0, bg_alpha, &[]);
            pass.dispatch_workgroups(wx, wy, 1);
        }
        guard.queue.submit(std::iter::once(enc.finish()));
    }

    // ---- Stage 2: blur (3 iterations H+V if edge_blur > 0) ----
    let blur_radius = edge_blur.round() as u32;
    let blur_iterations: u32 = if blur_radius > 0 { 3 } else { 0 };
    // Track which buffer holds the current alpha: true = alpha_a, false = alpha_b
    let mut alpha_in_a = true;

    for _ in 0..blur_iterations {
        // Horizontal pass
        let h_uniforms = BlurUniforms { width: w, height: h, radius: blur_radius, horizontal: 1 };
        guard.queue.write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&h_uniforms));
        {
            let h_bg = if alpha_in_a { bg_blur_a_to_b } else { bg_blur_b_to_a };
            let mut enc = guard.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
                pass.set_pipeline(&guard.pipeline_blur);
                pass.set_bind_group(0, h_bg, &[]);
                pass.dispatch_workgroups(wx, wy, 1);
            }
            guard.queue.submit(std::iter::once(enc.finish()));
        }
        alpha_in_a = !alpha_in_a;

        // Vertical pass
        let v_uniforms = BlurUniforms { width: w, height: h, radius: blur_radius, horizontal: 0 };
        guard.queue.write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&v_uniforms));
        {
            let v_bg = if alpha_in_a { bg_blur_a_to_b } else { bg_blur_b_to_a };
            let mut enc = guard.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
                pass.set_pipeline(&guard.pipeline_blur);
                pass.set_bind_group(0, v_bg, &[]);
                pass.dispatch_workgroups(wx, wy, 1);
            }
            guard.queue.submit(std::iter::once(enc.finish()));
        }
        alpha_in_a = !alpha_in_a;
    }
    // After blur, alpha is in alpha_a (if even number of blur passes incl. 0, alpha_in_a=true)

    // ---- Stage 3: composite ----
    let composite_uniforms = CompositeUniforms {
        width: w, height: h,
        spill_suppression,
        show_matte: if show_matte { 1 } else { 0 },
        invert: if settings.invert { 1 } else { 0 },
    };
    guard.queue.write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&composite_uniforms));

    {
        let comp_bg = if alpha_in_a { bg_composite_a } else { bg_composite_b };
        let mut enc = guard.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            pass.set_pipeline(&guard.pipeline_composite);
            pass.set_bind_group(0, comp_bg, &[]);
            pass.dispatch_workgroups(wx, wy, 1);
        }
        guard.queue.submit(std::iter::once(enc.finish()));
    }

    // ---- Readback ----
    {
        let mut enc = guard.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(&guard.bufs.dst_buf, 0, &guard.bufs.staging_buf, 0, image_size);
        guard.queue.submit(std::iter::once(enc.finish()));
    }

    let _ = guard.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });

    let slice = guard.bufs.staging_buf.slice(..image_size);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });
    let _ = guard.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });

    match rx.recv() {
        Ok(Ok(())) => {
            let mapped = slice.get_mapped_range();
            dst[..image_size as usize].copy_from_slice(&mapped);
            drop(mapped);
            guard.bufs.staging_buf.unmap();
            Ok(true)
        }
        _ => {
            // Defensive: ensure buffer is unmapped on failure. In wgpu, a failed
            // map_async leaves the buffer unmapped, but calling unmap() is a safe
            // no-op in that state and protects against unexpected partial-map states.
            guard.bufs.staging_buf.unmap();
            Err("staging map failed".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Internal: initialization
// ---------------------------------------------------------------------------

fn get_or_init() -> Result<&'static Mutex<Ctx>, String> {
    if let Some(c) = CTX.get() {
        return Ok(c);
    }
    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = CTX.get() {
        return Ok(c);
    }

    let (device, queue) = get_or_init_shared_device()?;
    let (pipeline_alpha, pipeline_blur, pipeline_composite) = create_pipelines(device)?;
    let bufs = create_bufs(device, 256, 256);
    let _ = CTX.set(Mutex::new(Ctx {
        device,
        queue,
        pipeline_alpha,
        pipeline_blur,
        pipeline_composite,
        bufs,
        bind_groups: None,
    }));
    CTX.get()
        .ok_or_else(|| "chroma_key: GPU ctx init race".to_string())
}

fn create_pipelines(
    device: &wgpu::Device,
) -> Result<(wgpu::ComputePipeline, wgpu::ComputePipeline, wgpu::ComputePipeline), String> {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("chroma_key"),
        source: super::load_shader(include_str!("../shaders/chroma_key.wgsl")),
    });

    let pipeline_alpha = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("chroma_key_alpha"),
        layout: None,
        module: &shader,
        entry_point: Some("compute_alpha"),
        compilation_options: Default::default(),
        cache: None,
    });

    let pipeline_blur = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("chroma_key_blur"),
        layout: None,
        module: &shader,
        entry_point: Some("blur"),
        compilation_options: Default::default(),
        cache: None,
    });

    let pipeline_composite = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("chroma_key_composite"),
        layout: None,
        module: &shader,
        entry_point: Some("composite"),
        compilation_options: Default::default(),
        cache: None,
    });

    Ok((pipeline_alpha, pipeline_blur, pipeline_composite))
}

fn create_bufs(device: &wgpu::Device, w: u32, h: u32) -> Bufs {
    let np = (w * h) as u64;
    let image_size = np * 4;
    let alpha_size = np * 4; // f32 = 4 bytes
    let ro = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
    let rw = ro | wgpu::BufferUsages::COPY_SRC;
    let st = wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST;

    Bufs {
        src_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ck_src"), size: image_size, usage: ro, mapped_at_creation: false,
        }),
        alpha_a: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ck_alpha_a"), size: alpha_size, usage: rw, mapped_at_creation: false,
        }),
        alpha_b: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ck_alpha_b"), size: alpha_size, usage: rw, mapped_at_creation: false,
        }),
        uniform_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ck_unif"), size: UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        dst_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ck_dst"), size: image_size, usage: rw, mapped_at_creation: false,
        }),
        staging_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ck_stage"), size: image_size, usage: st, mapped_at_creation: false,
        }),
        w, h,
    }
}

fn create_bind_groups(
    ctx: &Ctx,
    device: &wgpu::Device,
) -> (
    wgpu::BindGroup,
    wgpu::BindGroup,
    wgpu::BindGroup,
    wgpu::BindGroup,
    wgpu::BindGroup,
) {
    let alpha_layout = &ctx.pipeline_alpha.get_bind_group_layout(0);
    let blur_layout = &ctx.pipeline_blur.get_bind_group_layout(0);
    let composite_layout = &ctx.pipeline_composite.get_bind_group_layout(0);

    // compute_alpha: src(0) + uniforms(1) + alpha_a(2)
    let bg_alpha = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ck_alpha"),
        layout: alpha_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ctx.bufs.src_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: ctx.bufs.uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: ctx.bufs.alpha_a.as_entire_binding() },
        ],
    });

    // blur: alpha_src(0) + uniforms(1) + alpha_dst(2)
    let bg_blur_a_to_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ck_blur_atob"),
        layout: blur_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ctx.bufs.alpha_a.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: ctx.bufs.uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: ctx.bufs.alpha_b.as_entire_binding() },
        ],
    });

    let bg_blur_b_to_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ck_blur_btoa"),
        layout: blur_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ctx.bufs.alpha_b.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: ctx.bufs.uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: ctx.bufs.alpha_a.as_entire_binding() },
        ],
    });

    // composite: src(0) + alpha(1) + uniforms(2) + dst(3)
    let bg_composite_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ck_comp_a"),
        layout: composite_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ctx.bufs.src_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: ctx.bufs.alpha_a.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: ctx.bufs.uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: ctx.bufs.dst_buf.as_entire_binding() },
        ],
    });

    let bg_composite_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ck_comp_b"),
        layout: composite_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: ctx.bufs.src_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: ctx.bufs.alpha_b.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: ctx.bufs.uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: ctx.bufs.dst_buf.as_entire_binding() },
        ],
    });

    (bg_alpha, bg_blur_a_to_b, bg_blur_b_to_a, bg_composite_a, bg_composite_b)
}
