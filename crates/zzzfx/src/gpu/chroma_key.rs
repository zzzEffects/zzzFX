use std::cell::RefCell;
use std::sync::{atomic::AtomicBool, atomic::Ordering, OnceLock};

use crate::settings::chroma_key::ChromaKey;

use super::get_or_init_shared_device;

// ---------------------------------------------------------------------------
// Uniforms (must match WGSL struct layout)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct AlphaUniforms {
    width: u32, height: u32,
    key_cb: f32, key_cr: f32,
    threshold_sq: f32, soft_end_sq: f32, range_sq: f32,
    edge_softness: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BlurUniforms {
    width: u32, height: u32,
    radius: u32, horizontal: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CompositeUniforms {
    width: u32, height: u32,
    spill_suppression: f32,
    show_matte: u32, invert: u32,
}

const UNIFORM_SIZE: u64 = {
    let a = std::mem::size_of::<AlphaUniforms>();
    let b = std::mem::size_of::<BlurUniforms>();
    let c = std::mem::size_of::<CompositeUniforms>();
    (if a >= b && a >= c { a } else if b >= c { b } else { c }) as u64
};

// ---------------------------------------------------------------------------
// Read-only pipelines — shared without Mutex
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

struct Res {
    pipeline_alpha: wgpu::ComputePipeline,
    pipeline_blur: wgpu::ComputePipeline,
    pipeline_composite: wgpu::ComputePipeline,
    alpha_layout: wgpu::BindGroupLayout,
    blur_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
}

static RES: OnceLock<Res> = OnceLock::new();

fn get_res(device: &wgpu::Device) -> Result<&'static Res, String> {
    if let Some(r) = RES.get() { return Ok(r); }
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("chroma_key"),
        source: super::load_shader(include_str!("../shaders/chroma_key.wgsl")),
    });
    let pa = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("chroma_key_alpha"), layout: None, module: &shader,
        entry_point: Some("compute_alpha"), compilation_options: Default::default(), cache: None,
    });
    let pb = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("chroma_key_blur"), layout: None, module: &shader,
        entry_point: Some("blur"), compilation_options: Default::default(), cache: None,
    });
    let pc = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("chroma_key_composite"), layout: None, module: &shader,
        entry_point: Some("composite"), compilation_options: Default::default(), cache: None,
    });
    let al = pa.get_bind_group_layout(0);
    let bl = pb.get_bind_group_layout(0);
    let cl = pc.get_bind_group_layout(0);
    RES.set(Res { pipeline_alpha: pa, pipeline_blur: pb, pipeline_composite: pc, alpha_layout: al, blur_layout: bl, composite_layout: cl })
        .map_err(|_| "chroma_key: init race".to_string())?;
    RES.get().ok_or_else(|| "chroma_key: init race".to_string())
}

// ---------------------------------------------------------------------------
// Per-thread buffer pool — no Mutex, no contention
// ---------------------------------------------------------------------------

struct Bufs {
    src_buf: wgpu::Buffer,
    #[allow(dead_code)]
    alpha_a: wgpu::Buffer,
    #[allow(dead_code)]
    alpha_b: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    bg_alpha: wgpu::BindGroup,
    bg_blur_a_to_b: wgpu::BindGroup,
    bg_blur_b_to_a: wgpu::BindGroup,
    bg_composite_a: wgpu::BindGroup,
    bg_composite_b: wgpu::BindGroup,
    w: u32, h: u32,
}

thread_local! {
    static BUF_POOL: RefCell<Option<Bufs>> = const { RefCell::new(None) };
}

fn take_or_create_bufs(device: &wgpu::Device, res: &Res, w: u32, h: u32) -> Bufs {
    BUF_POOL.with(|cell| {
        let mut bufs = cell.borrow_mut().take();
        if bufs.as_ref().map_or(true, |b| b.w != w || b.h != h) {
            bufs = Some(create_bufs(device, res, w, h));
        }
        bufs.unwrap()
    })
}

fn return_bufs(bufs: Bufs) {
    let _ = BUF_POOL.try_with(|cell| { *cell.borrow_mut() = Some(bufs); });
}

fn create_bufs(device: &wgpu::Device, res: &Res, w: u32, h: u32) -> Bufs {
    let np = (w * h) as u64;
    let image_size = np * 4;
    let alpha_size = np * 4;
    let ro = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
    let rw = ro | wgpu::BufferUsages::COPY_SRC;
    let st = wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST;
    let src_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("ck_src"), size: image_size, usage: ro, mapped_at_creation: false });
    let alpha_a = device.create_buffer(&wgpu::BufferDescriptor { label: Some("ck_alpha_a"), size: alpha_size, usage: rw, mapped_at_creation: false });
    let alpha_b = device.create_buffer(&wgpu::BufferDescriptor { label: Some("ck_alpha_b"), size: alpha_size, usage: rw, mapped_at_creation: false });
    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("ck_unif"), size: UNIFORM_SIZE, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let dst_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("ck_dst"), size: image_size, usage: rw, mapped_at_creation: false });
    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("ck_stage"), size: image_size, usage: st, mapped_at_creation: false });

    let bg_alpha = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ck_alpha"), layout: &res.alpha_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: src_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: alpha_a.as_entire_binding() },
        ],
    });
    let bg_blur_a_to_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ck_blur_atob"), layout: &res.blur_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: alpha_a.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: alpha_b.as_entire_binding() },
        ],
    });
    let bg_blur_b_to_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ck_blur_btoa"), layout: &res.blur_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: alpha_b.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: alpha_a.as_entire_binding() },
        ],
    });
    let bg_composite_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ck_comp_a"), layout: &res.composite_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: src_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: alpha_a.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: dst_buf.as_entire_binding() },
        ],
    });
    let bg_composite_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ck_comp_b"), layout: &res.composite_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: src_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: alpha_b.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: dst_buf.as_entire_binding() },
        ],
    });

    Bufs { src_buf, alpha_a, alpha_b, uniform_buf, dst_buf, staging_buf, bg_alpha, bg_blur_a_to_b, bg_blur_b_to_a, bg_composite_a, bg_composite_b, w, h }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

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

    let (device, queue) = get_or_init_shared_device()?;
    let res = get_res(device).map_err(|e| { GPU_AVAILABLE.store(false, Ordering::Relaxed); e })?;
    let bufs = take_or_create_bufs(device, res, w, h);

    let image_size = (w * h * 4) as u64;
    let wx = (w + 15) / 16;
    let wy = (h + 15) / 16;

    let threshold = settings.threshold.clamp(0.0, 1.0);
    let edge_softness = settings.edge_softness.clamp(0.0, 1.0);
    let spill_suppression = settings.spill_suppression.clamp(0.0, 1.0);
    let edge_blur = settings.edge_blur.clamp(0.0, 20.0);
    let show_matte = settings.show_matte;
    let key_r = settings.key_color_r.clamp(0.0, 1.0);
    let key_g = settings.key_color_g.clamp(0.0, 1.0);
    let key_b = settings.key_color_b.clamp(0.0, 1.0);
    let key_cb = -0.168736 * key_r - 0.331264 * key_g + 0.5 * key_b + 0.5;
    let key_cr = 0.5 * key_r - 0.418688 * key_g - 0.081312 * key_b + 0.5;
    let threshold_sq = threshold * threshold;
    let soft_end = (threshold + edge_softness).clamp(0.0, 1.0);
    let soft_end_sq = soft_end * soft_end;
    let range_sq = soft_end_sq - threshold_sq;

    queue.write_buffer(&bufs.src_buf, 0, &src[..image_size as usize]);

    // ---- Stage 1: compute_alpha ----
    let alpha_uniforms = AlphaUniforms { width: w, height: h, key_cb, key_cr, threshold_sq, soft_end_sq, range_sq, edge_softness };
    queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&alpha_uniforms));
    {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            pass.set_pipeline(&res.pipeline_alpha);
            pass.set_bind_group(0, &bufs.bg_alpha, &[]);
            pass.dispatch_workgroups(wx, wy, 1);
        }
        queue.submit(std::iter::once(enc.finish()));
    }

    // ---- Stage 2: blur ----
    let blur_radius = edge_blur.round() as u32;
    let blur_iterations: u32 = if blur_radius > 0 { 3 } else { 0 };
    let mut alpha_in_a = true;
    for _ in 0..blur_iterations {
        let h_uniforms = BlurUniforms { width: w, height: h, radius: blur_radius, horizontal: 1 };
        queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&h_uniforms));
        {
            let h_bg = if alpha_in_a { &bufs.bg_blur_a_to_b } else { &bufs.bg_blur_b_to_a };
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
                pass.set_pipeline(&res.pipeline_blur);
                pass.set_bind_group(0, h_bg, &[]);
                pass.dispatch_workgroups(wx, wy, 1);
            }
            queue.submit(std::iter::once(enc.finish()));
        }
        alpha_in_a = !alpha_in_a;

        let v_uniforms = BlurUniforms { width: w, height: h, radius: blur_radius, horizontal: 0 };
        queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&v_uniforms));
        {
            let v_bg = if alpha_in_a { &bufs.bg_blur_a_to_b } else { &bufs.bg_blur_b_to_a };
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
                pass.set_pipeline(&res.pipeline_blur);
                pass.set_bind_group(0, v_bg, &[]);
                pass.dispatch_workgroups(wx, wy, 1);
            }
            queue.submit(std::iter::once(enc.finish()));
        }
        alpha_in_a = !alpha_in_a;
    }

    // ---- Stage 3: composite + copy to staging ----
    let composite_uniforms = CompositeUniforms { width: w, height: h, spill_suppression, show_matte: if show_matte { 1 } else { 0 }, invert: if settings.invert { 1 } else { 0 } };
    queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&composite_uniforms));
    {
        let comp_bg = if alpha_in_a { &bufs.bg_composite_a } else { &bufs.bg_composite_b };
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            pass.set_pipeline(&res.pipeline_composite);
            pass.set_bind_group(0, comp_bg, &[]);
            pass.dispatch_workgroups(wx, wy, 1);
        }
        enc.copy_buffer_to_buffer(&bufs.dst_buf, 0, &bufs.staging_buf, 0, image_size);
        queue.submit(std::iter::once(enc.finish()));
    }

    let result = super::blocking_readback(device, &bufs.staging_buf, image_size, &mut dst[..image_size as usize]);
    return_bufs(bufs);
    result.map(|()| true)
}
