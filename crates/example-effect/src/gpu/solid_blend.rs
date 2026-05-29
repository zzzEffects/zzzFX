use std::sync::{atomic::AtomicBool, atomic::Ordering, Mutex, OnceLock};

use crate::settings::solid::SolidColorBlend;

// ---------------------------------------------------------------------------
// Uniforms
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    width: u32,
    height: u32,
    blend_mode: u32,
    blend_amount: f32,
    solid_r: f32,
    solid_g: f32,
    solid_b: f32,
}

// ---------------------------------------------------------------------------
// GPU state
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

struct Ctx {
    device: &'static wgpu::Device,
    queue: &'static wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bufs: Bufs,
    bg: Option<wgpu::BindGroup>,
}

struct Bufs {
    src: wgpu::Buffer,
    uniform: wgpu::Buffer,
    dst: wgpu::Buffer,
    staging: wgpu::Buffer,
    w: u32,
    h: u32,
}

static CTX: OnceLock<Mutex<Ctx>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn try_solid_blend_gpu_render(
    settings: &SolidColorBlend,
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
    let total = (w * h) as usize;

    let ctx = match get_or_init() {
        Ok(c) => c,
        Err(_) => {
            GPU_AVAILABLE.store(false, Ordering::Relaxed);
            return Ok(false);
        }
    };

    let mut g = match ctx.lock() {
        Ok(g) => g,
        Err(_) => return Ok(false),
    };

    if g.bufs.w != w || g.bufs.h != h {
        g.bufs = create_bufs(g.device, w, h);
        g.bg = None;
    }

    let image_size = (total * 4) as u64;

    // Pack u8 RGBA → u32 for the shader
    let src_packed: Vec<u32> = src[..image_size as usize]
        .chunks_exact(4)
        .map(|c| {
            (c[3] as u32) << 24 | (c[2] as u32) << 16 | (c[1] as u32) << 8 | c[0] as u32
        })
        .collect();

    let a = settings.color_a.clamp(0.0, 1.0);
    let uniforms = Uniforms {
        width: w,
        height: h,
        blend_mode: settings.blend_mode as u32,
        blend_amount: a,
        solid_r: settings.color_r.clamp(0.0, 1.0),
        solid_g: settings.color_g.clamp(0.0, 1.0),
        solid_b: settings.color_b.clamp(0.0, 1.0),
    };

    g.queue
        .write_buffer(&g.bufs.src, 0, bytemuck::cast_slice(&src_packed));
    g.queue
        .write_buffer(&g.bufs.uniform, 0, bytemuck::bytes_of(&uniforms));

    if g.bg.is_none() {
        let layout = g.pipeline.get_bind_group_layout(0);
        g.bg = Some(g.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("solid_blend"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: g.bufs.src.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: g.bufs.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: g.bufs.dst.as_entire_binding(),
                },
            ],
        }));
    }

    // Dispatch
    let wx = (w + 15) / 16;
    let wy = (h + 15) / 16;
    {
        let mut enc = g
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&g.pipeline);
            pass.set_bind_group(0, g.bg.as_ref().unwrap(), &[]);
            pass.dispatch_workgroups(wx, wy, 1);
        }
        g.queue.submit(std::iter::once(enc.finish()));
    }

    // Readback
    {
        let mut enc = g
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(&g.bufs.dst, 0, &g.bufs.staging, 0, image_size);
        g.queue.submit(std::iter::once(enc.finish()));
    }

    let _ = g.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });

    let slice = g.bufs.staging.slice(..image_size);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = g.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });

    match rx.recv() {
        Ok(Ok(())) => {
            let mapped = slice.get_mapped_range();
            // Unpack u32 → u8 RGBA
            let dst_u32: &[u32] = bytemuck::cast_slice(&mapped);
            for (i, pixel) in dst_u32.iter().take(total).enumerate() {
                let o = i * 4;
                dst[o] = (pixel & 0xFF) as u8;
                dst[o + 1] = ((pixel >> 8) & 0xFF) as u8;
                dst[o + 2] = ((pixel >> 16) & 0xFF) as u8;
                dst[o + 3] = ((pixel >> 24) & 0xFF) as u8;
            }
            drop(mapped);
            g.bufs.staging.unmap();
            Ok(true)
        }
        _ => Err("staging map failed".to_string()),
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

    let (device, queue) = zzzfx_core::gpu::get_or_init_shared_device()?;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("solid_blend"),
        source: super::load_shader(include_str!("../../shaders/solid_blend.wgsl")),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("solid_blend"),
        layout: None,
        module: &shader,
        entry_point: Some("solid_blend_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bufs = create_bufs(device, 256, 256);
    let _ = CTX.set(Mutex::new(Ctx {
        device,
        queue,
        pipeline,
        bufs,
        bg: None,
    }));
    Ok(CTX.get().unwrap())
}

fn create_bufs(device: &wgpu::Device, w: u32, h: u32) -> Bufs {
    let n = (w * h) as u64;
    let image_size = n * 4;
    let uniform_size = std::mem::size_of::<Uniforms>() as u64;
    let ro = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
    let rw = ro | wgpu::BufferUsages::COPY_SRC;
    let st = wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST;
    Bufs {
        src: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("src"),
            size: image_size,
            usage: ro,
            mapped_at_creation: false,
        }),
        uniform: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform"),
            size: uniform_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        dst: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dst"),
            size: image_size,
            usage: rw,
            mapped_at_creation: false,
        }),
        staging: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging"),
            size: image_size,
            usage: st,
            mapped_at_creation: false,
        }),
        w,
        h,
    }
}
