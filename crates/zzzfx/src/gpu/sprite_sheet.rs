use std::cell::RefCell;
use std::sync::{atomic::AtomicBool, atomic::Ordering, OnceLock};

// ---------------------------------------------------------------------------
// GPU-side struct matching sprite_sheet.wgsl
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SpriteSheetUniforms {
    dst_w: u32,
    dst_h: u32,
    sheet_w: u32,
    sheet_h: u32,
    crop_x: u32,
    crop_y: u32,
    crop_w: u32,
    crop_h: u32,
    scale: f32,
    filter_mode: u32, // 0 = nearest, 1 = bilinear
    displacement_x: f32,
    displacement_y: f32,
    rotation_enabled: u32, // 0 or 1
    cos_rotation: f32,
    sin_rotation: f32,
    _pad: [u32; 1], // pad to 16-byte alignment
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
        label: Some("sprite_sheet"),
        source: super::load_shader(include_str!("../shaders/sprite_sheet.wgsl")),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("sprite_sheet"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bg_layout = pipeline.get_bind_group_layout(0);
    RES.set(Res { pipeline, bg_layout }).map_err(|_| "sprite_sheet: init race".to_string())?;
    RES.get().ok_or_else(|| "sprite_sheet: init race".to_string())
}

// ---------------------------------------------------------------------------
// Per-thread buffer pool — no Mutex, no contention
// ---------------------------------------------------------------------------

struct Bufs {
    sheet_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    output_w: u32,
    output_h: u32,
    sheet_size: u64,
}

thread_local! {
    static BUF_POOL: RefCell<Option<Bufs>> = const { RefCell::new(None) };
}

fn take_or_create_bufs(device: &wgpu::Device, res: &Res, output_w: u32, output_h: u32, sheet_size: u64) -> Bufs {
    BUF_POOL.with(|cell| {
        let mut bufs = cell.borrow_mut().take();
        if bufs.as_ref().map_or(true, |b| b.output_w != output_w || b.output_h != output_h || b.sheet_size < sheet_size) {
            bufs = Some(create_bufs(device, res, output_w, output_h, sheet_size));
        }
        bufs.unwrap()
    })
}

fn return_bufs(bufs: Bufs) {
    let _ = BUF_POOL.try_with(|cell| {
        *cell.borrow_mut() = Some(bufs);
    });
}

fn create_bufs(device: &wgpu::Device, res: &Res, output_w: u32, output_h: u32, sheet_size: u64) -> Bufs {
    let image_size = output_w as u64 * output_h as u64 * 4;

    let sheet_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sheet"),
        size: sheet_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uniforms"),
        size: std::mem::size_of::<SpriteSheetUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let dst_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("dst"),
        size: image_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("staging"),
        size: image_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sprite_sheet"),
        layout: &res.bg_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: sheet_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: dst_buf.as_entire_binding() },
        ],
    });

    Bufs { sheet_buf, uniform_buf, dst_buf, staging_buf, bind_group, output_w, output_h, sheet_size }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// GPU render for normal mode (crop + scale + center + displacement + rotation).
pub fn try_sprite_sheet_gpu_render(
    crop_rect: (u32, u32, u32, u32),
    sheet_rgba: &[u8],
    sheet_w: u32,
    sheet_h: u32,
    scale: f32,
    filter_mode: u32,
    displacement_x: f32,
    displacement_y: f32,
    _displacement_pixel_based: bool,
    rotation_deg: f32,
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
) -> Result<bool, String> {
    gpu_render_impl(crop_rect, sheet_rgba, sheet_w, sheet_h, scale, filter_mode, displacement_x, displacement_y, rotation_deg, dst, dst_w, dst_h)
}

/// GPU render for selection mode (full sheet scaling + centering + displacement).
pub fn try_selection_mode_gpu_render(
    sheet_rgba: &[u8],
    sheet_w: u32,
    sheet_h: u32,
    fit_scale: f32,
    displacement_x: f32,
    displacement_y: f32,
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
) -> Result<bool, String> {
    gpu_render_impl((0, 0, sheet_w, sheet_h), sheet_rgba, sheet_w, sheet_h, fit_scale, 0, displacement_x, displacement_y, 0.0, dst, dst_w, dst_h)
}

fn gpu_render_impl(
    crop_rect: (u32, u32, u32, u32),
    sheet_rgba: &[u8],
    sheet_w: u32,
    sheet_h: u32,
    scale: f32,
    filter_mode: u32,
    displacement_x: f32,
    displacement_y: f32,
    rotation_deg: f32,
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed) {
        return Ok(false);
    }

    let (cx, cy, cw, ch) = crop_rect;

    let sheet_byte_size = sheet_w as u64 * sheet_h as u64 * 4;

    // Sanity check: reject zero or absurdly large dimensions
    if dst_w == 0 || dst_h == 0 || sheet_w == 0 || sheet_h == 0 || dst_w > 16384 || dst_h > 16384 {
        return Ok(false);
    }

    let (device, queue) = super::get_or_init_shared_device()?;
    let res = get_res(device).map_err(|e| { GPU_AVAILABLE.store(false, Ordering::Relaxed); e })?;
    let bufs = take_or_create_bufs(device, res, dst_w, dst_h, sheet_byte_size);

    let (rot_enabled, cos_r, sin_r) = if rotation_deg != 0.0 {
        let rad = rotation_deg as f64 * std::f64::consts::PI / 180.0;
        (1u32, rad.cos() as f32, rad.sin() as f32)
    } else {
        (0u32, 1.0f32, 0.0f32)
    };
    let uniforms = SpriteSheetUniforms {
        dst_w, dst_h, sheet_w, sheet_h,
        crop_x: cx, crop_y: cy, crop_w: cw, crop_h: ch,
        scale, filter_mode,
        displacement_x, displacement_y,
        rotation_enabled: rot_enabled,
        cos_rotation: cos_r,
        sin_rotation: sin_r,
        _pad: [0],
    };

    // Always upload sheet and uniforms
    queue.write_buffer(&bufs.sheet_buf, 0, sheet_rgba);
    queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    let image_size = dst_w as u64 * dst_h as u64 * 4;

    // Single encoder: compute dispatch + copy to staging
    {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None, timestamp_writes: None,
            });
            pass.set_pipeline(&res.pipeline);
            pass.set_bind_group(0, &bufs.bind_group, &[]);
            pass.dispatch_workgroups((dst_w + 15) / 16, (dst_h + 15) / 16, 1);
        }
        encoder.copy_buffer_to_buffer(&bufs.dst_buf, 0, &bufs.staging_buf, 0, image_size);
        queue.submit(std::iter::once(encoder.finish()));
    }

    let result = super::blocking_readback(device, &bufs.staging_buf, image_size, &mut dst[..image_size as usize]);
    return_bufs(bufs);
    result.map(|()| true)
}
