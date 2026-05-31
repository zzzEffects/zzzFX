use std::sync::{atomic::AtomicBool, atomic::Ordering, Mutex, OnceLock};

use crate::settings::cast_shadow::CastShadow;

use super::get_or_init_shared_device;

// ---------------------------------------------------------------------------
// Uniforms (must match WGSL struct layout exactly)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    width: u32,
    height: u32,
    contact_x: f32,
    contact_y: f32,
    normal_x: f32,
    normal_y: f32,
    axis_x: f32,
    axis_y: f32,
    scale: f32,
    shear_angle: f32,
    shear_amount: f32,
    inv_bbox_perp: f32,
    total_dx: f32,
    total_dy: f32,
    pivot_mode: u32,
    fade: f32,
    shadow_r: f32,
    shadow_g: f32,
    shadow_b: f32,
    shadow_a: f32,
    source_opacity: f32,
    blur_radius: u32,
    horizontal: u32,
    alpha_threshold: f32,
    bbox_min_x: f32,
    bbox_max_x: f32,
    bbox_min_y: f32,
    bbox_max_y: f32,
}

const UNIFORM_SIZE: u64 = std::mem::size_of::<Uniforms>() as u64;

// ---------------------------------------------------------------------------
// GPU state
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

struct Bufs {
    w: u32,
    h: u32,
    src: wgpu::Buffer,
    alpha_a: wgpu::Buffer,
    alpha_b: wgpu::Buffer,
    dst: wgpu::Buffer,
    staging: wgpu::Buffer,
    uniform: wgpu::Buffer,
}

struct Ctx {
    device: &'static wgpu::Device,
    queue: &'static wgpu::Queue,
    pipeline_project: wgpu::ComputePipeline,
    pipeline_blur: wgpu::ComputePipeline,
    pipeline_composite: wgpu::ComputePipeline,
    bufs: Bufs,
    bg_project: Option<wgpu::BindGroup>,
    bg_blur_h: Option<wgpu::BindGroup>,
    bg_blur_v: Option<wgpu::BindGroup>,
    bg_composite: Option<wgpu::BindGroup>,
}

static CTX: OnceLock<Mutex<Ctx>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

fn get_or_init() -> Result<std::sync::MutexGuard<'static, Ctx>, String> {
    let mutex = CTX.get_or_init(|| Mutex::new(create_ctx()));
    let guard = mutex
        .lock()
        .map_err(|_| "ctx lock poisoned".to_string())?;
    Ok(guard)
}

fn create_ctx() -> Ctx {
    let (device, queue) = get_or_init_shared_device().expect("shared device");
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("cast_shadow"),
        source: super::load_shader(include_str!("../shaders/cast_shadow.wgsl")),
    });
    let pipeline_project = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("cs_project"),
        layout: None,
        module: &shader,
        entry_point: Some("project"),
        compilation_options: Default::default(),
        cache: None,
    });
    let pipeline_blur = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("cs_blur"),
        layout: None,
        module: &shader,
        entry_point: Some("blur"),
        compilation_options: Default::default(),
        cache: None,
    });
    let pipeline_composite = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("cs_composite"),
        layout: None,
        module: &shader,
        entry_point: Some("composite"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bufs = create_bufs(device, 256, 256);

    let layout_project = pipeline_project.get_bind_group_layout(0);
    let layout_blur = pipeline_blur.get_bind_group_layout(0);
    let layout_composite = pipeline_composite.get_bind_group_layout(0);

    let bg_project = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cs_project_bg"),
        layout: &layout_project,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: bufs.src.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: bufs.alpha_a.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: bufs.uniform.as_entire_binding(),
            },
        ],
    }));

    // Blur H: alpha_a → alpha_b
    let bg_blur_h = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cs_blur_h_bg"),
        layout: &layout_blur,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 1,
                resource: bufs.alpha_a.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: bufs.alpha_b.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: bufs.uniform.as_entire_binding(),
            },
        ],
    }));

    // Blur V: alpha_b → alpha_a
    let bg_blur_v = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cs_blur_v_bg"),
        layout: &layout_blur,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 1,
                resource: bufs.alpha_b.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: bufs.alpha_a.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: bufs.uniform.as_entire_binding(),
            },
        ],
    }));

    let bg_composite = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cs_composite_bg"),
        layout: &layout_composite,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: bufs.src.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: bufs.alpha_a.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: bufs.uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: bufs.dst.as_entire_binding(),
            },
        ],
    }));

    Ctx {
        device,
        queue,
        pipeline_project,
        pipeline_blur,
        pipeline_composite,
        bufs,
        bg_project,
        bg_blur_h,
        bg_blur_v,
        bg_composite,
    }
}

fn create_bufs(device: &wgpu::Device, w: u32, h: u32) -> Bufs {
    let count = (w * h) as u64;
    let u32_size = count * 4;
    let f32_size = count * 4;
    Bufs {
        w,
        h,
        src: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cs_src"),
            size: u32_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        alpha_a: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cs_alpha_a"),
            size: f32_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        alpha_b: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cs_alpha_b"),
            size: f32_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        dst: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cs_dst"),
            size: u32_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }),
        staging: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cs_staging"),
            size: u32_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }),
        uniform: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cs_uniform"),
            size: UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn try_cast_shadow_gpu_render(
    shadow: &CastShadow,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed) {
        return Ok(false);
    }
    if shadow.is_identity() {
        return Ok(false); // let CPU handle trivial cases
    }

    let w = width as u32;
    let h = height as u32;

    let result: Result<bool, String> = (|| {
        let mut ctx = get_or_init()?;

        // Recreate buffers if dimensions changed
        if ctx.bufs.w != w || ctx.bufs.h != h {
            ctx.bufs = create_bufs(ctx.device, w, h);
            // Invalidate cached bind groups
            ctx.bg_project = None;
            ctx.bg_blur_h = None;
            ctx.bg_blur_v = None;
            ctx.bg_composite = None;
        }

        let dev = ctx.device;
        let queue = ctx.queue;

        // Upload source (RGBA8 → packed u32)
        let src_u32: Vec<u32> = src
            .chunks_exact(4)
            .map(|p| {
                (p[0] as u32) | ((p[1] as u32) << 8) | ((p[2] as u32) << 16) | ((p[3] as u32) << 24)
            })
            .collect();
        queue.write_buffer(&ctx.bufs.src, 0, bytemuck::cast_slice(&src_u32));

        // Compute all axes based on pivot mode
        let axes = compute_axes(shadow, src, width, height);

        let blur_radius = (shadow.softness.clamp(0.0, 1.0)
            * f32::min(width as f32, height as f32)
            * 0.08)
            .ceil() as u32;

        let base_uniforms = Uniforms {
            width: w,
            height: h,
            contact_x: 0.0,
            contact_y: 0.0,
            normal_x: 0.0,
            normal_y: 0.0,
            axis_x: 0.0,
            axis_y: 0.0,
            scale: shadow.scale.clamp(0.1, 3.0),
            shear_angle: 0.0,
            shear_amount: 0.5,
            inv_bbox_perp: 0.0,
            total_dx: (shadow.offset_x.clamp(0.0, 1.0) - 0.5) * width as f32,
            total_dy: (shadow.offset_y.clamp(0.0, 1.0) - 0.5) * height as f32,
            pivot_mode: shadow.pivot_mode as u32,
            fade: shadow.fade.clamp(0.0, 1.0),
            shadow_r: shadow.shadow_color_r.clamp(0.0, 1.0),
            shadow_g: shadow.shadow_color_g.clamp(0.0, 1.0),
            shadow_b: shadow.shadow_color_b.clamp(0.0, 1.0),
            shadow_a: shadow.shadow_color_a.clamp(0.0, 1.0),
            
            source_opacity: shadow.source_opacity.clamp(0.0, 1.0),
            blur_radius,
            horizontal: 0,
            alpha_threshold: shadow.alpha_threshold.clamp(0.0, 1.0),
            bbox_min_x: 0.0,
            bbox_max_x: 0.0,
            bbox_min_y: 0.0,
            bbox_max_y: 0.0,
        };

        // Create bind groups if needed (lazy, after possible buffer recreation)
        let layout_project = ctx.pipeline_project.get_bind_group_layout(0);
        let layout_blur = ctx.pipeline_blur.get_bind_group_layout(0);
        let layout_composite = ctx.pipeline_composite.get_bind_group_layout(0);

        if ctx.bg_project.is_none() {
            ctx.bg_project = Some(dev.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cs_project_bg"),
                layout: &layout_project,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: ctx.bufs.src.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: ctx.bufs.alpha_a.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: ctx.bufs.uniform.as_entire_binding() },
                ],
            }));
        }
        if ctx.bg_blur_h.is_none() {
            ctx.bg_blur_h = Some(dev.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cs_blur_h_bg"),
                layout: &layout_blur,
                entries: &[
                    wgpu::BindGroupEntry { binding: 1, resource: ctx.bufs.alpha_a.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: ctx.bufs.alpha_b.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: ctx.bufs.uniform.as_entire_binding() },
                ],
            }));
        }
        if ctx.bg_blur_v.is_none() {
            ctx.bg_blur_v = Some(dev.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cs_blur_v_bg"),
                layout: &layout_blur,
                entries: &[
                    wgpu::BindGroupEntry { binding: 1, resource: ctx.bufs.alpha_b.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: ctx.bufs.alpha_a.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: ctx.bufs.uniform.as_entire_binding() },
                ],
            }));
        }
        if ctx.bg_composite.is_none() {
            ctx.bg_composite = Some(dev.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cs_composite_bg"),
                layout: &layout_composite,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: ctx.bufs.src.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: ctx.bufs.alpha_a.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: ctx.bufs.uniform.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: ctx.bufs.dst.as_entire_binding() },
                ],
            }));
        }

        let wx = (w + 15) / 16;
        let wy = (h + 15) / 16;

        // Zero alpha_a before accumulation (for multi-axis modes)
        let alpha_size = (w * h * 4) as u64;
        {
            let zeros = vec![0u8; alpha_size as usize];
            queue.write_buffer(&ctx.bufs.alpha_a, 0, &zeros);
        }

        // Pass 1: project (once per axis, accumulating via max)
        for axis in &axes {
            let mut axis_uniforms = base_uniforms;
            axis_uniforms.contact_x = axis.contact_x;
            axis_uniforms.contact_y = axis.contact_y;
            axis_uniforms.normal_x = axis.nx;
            axis_uniforms.normal_y = axis.ny;
            axis_uniforms.axis_x = axis.ax;
            axis_uniforms.axis_y = axis.ay;
            axis_uniforms.shear_angle = shadow.shear_angle.clamp(0.0, 360.0).to_radians();
            axis_uniforms.shear_amount = shadow.shear_amount.clamp(0.0, 1.0);
            axis_uniforms.inv_bbox_perp = if axis.bbox_perp > 0.0 { 1.0 / axis.bbox_perp } else { 0.0 };
            axis_uniforms.bbox_min_x = axis.bbox_min_x;
            axis_uniforms.bbox_max_x = axis.bbox_max_x;
            axis_uniforms.bbox_min_y = axis.bbox_min_y;
            axis_uniforms.bbox_max_y = axis.bbox_max_y;
            queue.write_buffer(&ctx.bufs.uniform, 0, bytemuck::bytes_of(&axis_uniforms));

            let mut encoder =
                dev.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("cs_project") });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("cs_project"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&ctx.pipeline_project);
                pass.set_bind_group(0, ctx.bg_project.as_ref().unwrap(), &[]);
                pass.dispatch_workgroups(wx, wy, 1);
            }
            queue.submit(std::iter::once(encoder.finish()));
        }

        // Restore full uniforms for blur/composite (using last axis's params)
        queue.write_buffer(&ctx.bufs.uniform, 0, bytemuck::bytes_of(&base_uniforms));

        // Pass 2: blur (separable H + V)
        if blur_radius > 0 {
            // Update uniform with horizontal flag
            let mut blur_uniforms_h = base_uniforms;
            blur_uniforms_h.horizontal = 1;
            queue.write_buffer(&ctx.bufs.uniform, 0, bytemuck::bytes_of(&blur_uniforms_h));

            let mut encoder =
                dev.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("cs_blur_h") });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("cs_blur_h"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&ctx.pipeline_blur);
                pass.set_bind_group(0, ctx.bg_blur_h.as_ref().unwrap(), &[]);
                pass.dispatch_workgroups(wx, wy, 1);
            }
            queue.submit(std::iter::once(encoder.finish()));

            // Vertical pass
            let mut blur_uniforms_v = base_uniforms;
            blur_uniforms_v.horizontal = 0;
            queue.write_buffer(&ctx.bufs.uniform, 0, bytemuck::bytes_of(&blur_uniforms_v));

            let mut encoder =
                dev.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("cs_blur_v") });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("cs_blur_v"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&ctx.pipeline_blur);
                pass.set_bind_group(0, ctx.bg_blur_v.as_ref().unwrap(), &[]);
                pass.dispatch_workgroups(wx, wy, 1);
            }
            queue.submit(std::iter::once(encoder.finish()));
        }

        // Pass 3: composite
        {
            let mut encoder =
                dev.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("cs_composite") });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("cs_composite"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&ctx.pipeline_composite);
                pass.set_bind_group(0, ctx.bg_composite.as_ref().unwrap(), &[]);
                pass.dispatch_workgroups(wx, wy, 1);
            }
            queue.submit(std::iter::once(encoder.finish()));
        }

        // Readback
        let buf_size = (w * h * 4) as u64;
        {
            let mut encoder =
                dev.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("cs_readback") });
            encoder.copy_buffer_to_buffer(&ctx.bufs.dst, 0, &ctx.bufs.staging, 0, buf_size);
            queue.submit(std::iter::once(encoder.finish()));
        }

        let staging_slice = ctx.bufs.staging.slice(..buf_size);
        let (tx, rx) = std::sync::mpsc::channel();
        staging_slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = dev.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        match rx.recv() {
            Ok(Ok(())) => {
                let mapped = staging_slice.get_mapped_range();
                let staging_u32: &[u32] = bytemuck::cast_slice(&mapped);
                for (i, &packed) in staging_u32.iter().enumerate() {
                    let o = i * 4;
                    dst[o] = (packed & 0xFF) as u8;
                    dst[o + 1] = ((packed >> 8) & 0xFF) as u8;
                    dst[o + 2] = ((packed >> 16) & 0xFF) as u8;
                    dst[o + 3] = ((packed >> 24) & 0xFF) as u8;
                }
                drop(mapped);
                ctx.bufs.staging.unmap();
                Ok(true)
            }
            _ => {
                GPU_AVAILABLE.store(false, Ordering::Relaxed);
                Err("staging map failed".to_string())
            }
        }
    })();

    match result {
        Ok(true) => Ok(true),
        Ok(false) => Ok(false),
        Err(_) => {
            GPU_AVAILABLE.store(false, Ordering::Relaxed);
            Ok(false)
        }
    }
}

// ---------------------------------------------------------------------------
// GPU axis data (matches CPU AxisData)
// ---------------------------------------------------------------------------

struct GpuAxis {
    contact_x: f32,
    contact_y: f32,
    nx: f32,
    ny: f32,
    ax: f32,
    ay: f32,
    bbox_perp: f32,
    bbox_min_x: f32,
    bbox_max_x: f32,
    bbox_min_y: f32,
    bbox_max_y: f32,
}

// ---------------------------------------------------------------------------
// Compute axes for all pivot modes
// ---------------------------------------------------------------------------

fn compute_axes(
    shadow: &CastShadow,
    src: &[u8],
    width: usize,
    height: usize,
) -> Vec<GpuAxis> {
    let pivot_angle = shadow.pivot_angle.clamp(0.0, 360.0);
    let wf = width as f32;
    let hf = height as f32;

    match shadow.pivot_mode {
        crate::settings::cast_shadow::PivotMode::AutoSingle => {
            let bbox = find_bbox(src, width, height, shadow.alpha_threshold.clamp(0.0, 1.0));
            compute_single_axis(bbox, pivot_angle, wf, hf)
                .into_iter()
                .collect()
        }
        crate::settings::cast_shadow::PivotMode::AutoMulti => {
            let components = find_components_gpu(src, width, height, shadow.alpha_threshold.clamp(0.0, 1.0));
            if components.is_empty() {
                let bbox = None;
                compute_single_axis(bbox, pivot_angle, wf, hf)
                    .into_iter()
                    .collect()
            } else {
                components
                    .into_iter()
                    .filter_map(|comp| compute_single_axis(Some(comp), pivot_angle, wf, hf))
                    .collect()
            }
        }
        crate::settings::cast_shadow::PivotMode::ManualSingle => {
            let mo_x = (shadow.manual_center_x.clamp(0.0, 1.0) - 0.5) * wf;
            let mo_y = (shadow.manual_center_y.clamp(0.0, 1.0) - 0.5) * hf;
            compute_axis_manual(pivot_angle, wf, hf, mo_x, mo_y)
                .into_iter()
                .collect()
        }
    }
}

// ---------------------------------------------------------------------------
// Single axis from a bbox (or None = frame center for manual mode)
// ---------------------------------------------------------------------------

fn compute_single_axis(
    bbox: Option<(u32, u32, u32, u32)>,
    pivot_angle: f32,
    wf: f32,
    hf: f32,
) -> Option<GpuAxis> {
    match bbox {
        Some((min_x, max_x, min_y, max_y)) => {
            let min_xf = min_x as f32;
            let max_xf = max_x as f32;
            let min_yf = min_y as f32;
            let max_yf = max_y as f32;
            let bcx = (min_xf + max_xf) * 0.5;
            let bcy = (min_yf + max_yf) * 0.5;
            let hw = (max_xf - min_xf) * 0.5;
            let hh = (max_yf - min_yf) * 0.5;
            let bbox_w = max_xf - min_xf;
            let bbox_h = max_yf - min_yf;

            let rad = pivot_angle.to_radians();
            let dx = rad.sin();
            let dy = -rad.cos();
            let tx = if dx.abs() < 1e-6 { f32::MAX } else if dx > 0.0 { hw / dx } else { -hw / dx };
            let ty = if dy.abs() < 1e-6 { f32::MAX } else if dy > 0.0 { hh / dy } else { -hh / dy };
            let t = tx.min(ty);
            let cx = bcx + t * dx;
            let cy = bcy + t * dy;
            let nx = -dx;
            let ny = -dy;
            let ax = ny;
            let ay = -nx;
            let bp = bbox_w * nx.abs() + bbox_h * ny.abs();
            Some(GpuAxis {
                contact_x: cx, contact_y: cy,
                nx, ny, ax, ay,
                bbox_perp: bp,
                bbox_min_x: min_xf, bbox_max_x: max_xf,
                bbox_min_y: min_yf, bbox_max_y: max_yf,
            })
        }
        None => {
            compute_axis_manual(pivot_angle, wf, hf, 0.0, 0.0)
        }
    }
}

fn compute_axis_manual(
    pivot_angle: f32,
    wf: f32,
    hf: f32,
    offset_x: f32,
    offset_y: f32,
) -> Option<GpuAxis> {
    let rad = pivot_angle.to_radians();
    let dx = rad.sin();
    let dy = -rad.cos();
    let nx = -dx;
    let ny = -dy;
    let ax = ny;
    let ay = -nx;
    let bp = wf * nx.abs() + hf * ny.abs();
    Some(GpuAxis {
        contact_x: wf * 0.5 + offset_x,
        contact_y: hf * 0.5 + offset_y,
        nx, ny, ax, ay,
        bbox_perp: bp,
        bbox_min_x: 0.0, bbox_max_x: wf,
        bbox_min_y: 0.0, bbox_max_y: hf,
    })
}

// ---------------------------------------------------------------------------
// Find full-frame bbox
// ---------------------------------------------------------------------------

fn find_bbox(src: &[u8], width: usize, height: usize, threshold: f32) -> Option<(u32, u32, u32, u32)> {
    let mut min_x = u32::MAX;
    let mut max_x = 0u32;
    let mut min_y = u32::MAX;
    let mut max_y = 0u32;
    let mut found = false;
    for y in 0..height {
        for x in 0..width {
            let a = src[(y * width + x) * 4 + 3] as f32 / 255.0;
            if a >= threshold {
                found = true;
                min_x = min_x.min(x as u32);
                max_x = max_x.max(x as u32);
                min_y = min_y.min(y as u32);
                max_y = max_y.max(y as u32);
            }
        }
    }
    if found { Some((min_x, max_x, min_y, max_y)) } else { None }
}

// ---------------------------------------------------------------------------
// BFS connected-component detection (GPU-side, same as CPU find_components)
// ---------------------------------------------------------------------------

fn find_components_gpu(
    src: &[u8],
    width: usize,
    height: usize,
    threshold: f32,
) -> Vec<(u32, u32, u32, u32)> {
    let total = width * height;
    let threshold_u8 = (threshold * 255.0).ceil() as u8;
    let mut labels = vec![0u32; total];
    let mut components = Vec::new();
    let mut queue = Vec::with_capacity(1024);

    let mut comp_idx = 1u32;
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if labels[idx] != 0 || src[idx * 4 + 3] < threshold_u8 {
                continue;
            }
            labels[idx] = comp_idx;
            queue.clear();
            queue.push((x, y));
            let mut head = 0usize;
            let mut min_x = x as u32;
            let mut max_x = x as u32;
            let mut min_y = y as u32;
            let mut max_y = y as u32;

            while head < queue.len() {
                let (cx, cy) = queue[head];
                head += 1;
                if cx > 0 {
                    let nidx = cy * width + (cx - 1);
                    if labels[nidx] == 0 && src[nidx * 4 + 3] >= threshold_u8 {
                        labels[nidx] = comp_idx;
                        queue.push((cx - 1, cy));
                        min_x = min_x.min((cx - 1) as u32);
                    }
                }
                if cx + 1 < width {
                    let nidx = cy * width + (cx + 1);
                    if labels[nidx] == 0 && src[nidx * 4 + 3] >= threshold_u8 {
                        labels[nidx] = comp_idx;
                        queue.push((cx + 1, cy));
                        max_x = max_x.max((cx + 1) as u32);
                    }
                }
                if cy > 0 {
                    let nidx = (cy - 1) * width + cx;
                    if labels[nidx] == 0 && src[nidx * 4 + 3] >= threshold_u8 {
                        labels[nidx] = comp_idx;
                        queue.push((cx, cy - 1));
                        min_y = min_y.min((cy - 1) as u32);
                    }
                }
                if cy + 1 < height {
                    let nidx = (cy + 1) * width + cx;
                    if labels[nidx] == 0 && src[nidx * 4 + 3] >= threshold_u8 {
                        labels[nidx] = comp_idx;
                        queue.push((cx, cy + 1));
                        max_y = max_y.max((cy + 1) as u32);
                    }
                }
            }
            components.push((min_x, max_x, min_y, max_y));
            comp_idx += 1;
        }
    }
    components
}
