use std::sync::{atomic::AtomicBool, atomic::Ordering, Mutex, OnceLock};

use crate::settings::ambient_light::AmbientLight;

use super::get_or_init_shared_device;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    width: u32, height: u32,
    intensity: f32, light_wrap: f32, ambient_tint: f32, brightness: f32,
    fg_opacity: f32, bg_opacity: f32,
}

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

struct Ctx {
    device: &'static wgpu::Device,
    queue: &'static wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bufs: Bufs,
    bg: Option<wgpu::BindGroup>,
}

struct Bufs {
    fg_buf: wgpu::Buffer,
    bg_buf: wgpu::Buffer,
    local_buf: wgpu::Buffer,
    global_buf: wgpu::Buffer,
    edge_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    w: u32, h: u32,
}

static CTX: OnceLock<Mutex<Ctx>> = OnceLock::new();

pub fn try_ambient_light_gpu_render(
    ambient_local: &[[f32; 3]],
    ambient_global: &[[f32; 3]],
    fg: &[u8], bg: &[u8],
    edge_factor: &[f32],
    settings: &AmbientLight,
    dst: &mut [u8],
    width: usize, height: usize,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed) { return Ok(false); }

    let w = width as u32;
    let h = height as u32;

    let ctx = match get_or_init() {
        Ok(c) => c,
        Err(_) => { GPU_AVAILABLE.store(false, Ordering::Relaxed); return Ok(false); }
    };

    let mut g = match ctx.lock() {
        Ok(g) => g,
        Err(_) => return Ok(false),
    };

    if g.bufs.w != w || g.bufs.h != h {
        g.bufs = create_bufs(g.device, w, h);
        g.bg = None;
    }

    let total = (w * h) as usize;
    let image_size = (total * 4) as u64;
    let wx = (w + 15) / 16;
    let wy = (h + 15) / 16;

    // Convert f32×3 ambient to packed u8 RGBA
    let local_u8: Vec<u8> = ambient_local.iter().flat_map(|c| {
        [ (c[0].clamp(0.0,1.0)*255.0).round() as u8,
          (c[1].clamp(0.0,1.0)*255.0).round() as u8,
          (c[2].clamp(0.0,1.0)*255.0).round() as u8,
          255u8 ]
    }).collect();

    let global_u8: Vec<u8> = ambient_global.iter().flat_map(|c| {
        [ (c[0].clamp(0.0,1.0)*255.0).round() as u8,
          (c[1].clamp(0.0,1.0)*255.0).round() as u8,
          (c[2].clamp(0.0,1.0)*255.0).round() as u8,
          255u8 ]
    }).collect();

    g.queue.write_buffer(&g.bufs.fg_buf, 0, fg);
    g.queue.write_buffer(&g.bufs.bg_buf, 0, bg);
    g.queue.write_buffer(&g.bufs.local_buf, 0, &local_u8);
    g.queue.write_buffer(&g.bufs.global_buf, 0, &global_u8);
    g.queue.write_buffer(&g.bufs.edge_buf, 0, bytemuck::cast_slice(edge_factor));

    let u = Uniforms {
        width: w, height: h,
        intensity: settings.intensity.clamp(0.0, 1.0),
        light_wrap: settings.light_wrap.clamp(0.0, 1.0),
        ambient_tint: settings.ambient_tint.clamp(0.0, 1.0),
        brightness: settings.brightness.clamp(0.0, 2.0),
        fg_opacity: settings.fg_opacity.clamp(0.0, 1.0),
        bg_opacity: settings.bg_opacity.clamp(0.0, 1.0),
    };
    g.queue.write_buffer(&g.bufs.uniform_buf, 0, bytemuck::bytes_of(&u));

    if g.bg.is_none() {
        let layout = g.pipeline.get_bind_group_layout(0);
        g.bg = Some(g.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ambient"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: g.bufs.fg_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: g.bufs.bg_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: g.bufs.local_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: g.bufs.global_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: g.bufs.edge_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: g.bufs.uniform_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: g.bufs.dst_buf.as_entire_binding() },
            ],
        }));
    }

    // Dispatch
    {
        let mut enc = g.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            pass.set_pipeline(&g.pipeline);
            pass.set_bind_group(0, g.bg.as_ref().ok_or("bind group not initialized")?, &[]);
            pass.dispatch_workgroups(wx, wy, 1);
        }
        g.queue.submit(std::iter::once(enc.finish()));
    }

    // Readback
    {
        let mut enc = g.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(&g.bufs.dst_buf, 0, &g.bufs.staging_buf, 0, image_size);
        g.queue.submit(std::iter::once(enc.finish()));
    }

    let _ = g.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });

    let slice = g.bufs.staging_buf.slice(..image_size);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });
    let _ = g.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });

    match rx.recv() {
        Ok(Ok(())) => {
            let mapped = slice.get_mapped_range();
            dst[..image_size as usize].copy_from_slice(&mapped);
            drop(mapped);
            g.bufs.staging_buf.unmap();
            Ok(true)
        }
        _ => {
            g.bufs.staging_buf.unmap();
            Err("staging map failed".to_string())
        }
    }
}

fn get_or_init() -> Result<&'static Mutex<Ctx>, String> {
    if let Some(c) = CTX.get() { return Ok(c); }
    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = CTX.get() { return Ok(c); }

    let (device, queue) = get_or_init_shared_device()?;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ambient_light"),
        source: super::load_shader(include_str!("../shaders/ambient_light.wgsl")),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ambient"), layout: None, module: &shader, entry_point: Some("main"),
        compilation_options: Default::default(), cache: None,
    });
    let bufs = create_bufs(device, 256, 256);
    let _ = CTX.set(Mutex::new(Ctx { device, queue, pipeline, bufs, bg: None }));
    CTX.get().ok_or_else(|| "ambient_light: GPU ctx init race".to_string())
}

fn create_bufs(device: &wgpu::Device, w: u32, h: u32) -> Bufs {
    let wu = w as usize; let hu = h as usize;
    let is = (wu * hu * 4) as u64;
    let fs = (wu * hu * 4) as u64; // f32 = 4 bytes
    let us = std::mem::size_of::<Uniforms>() as u64;
    let ro = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
    let rw = ro | wgpu::BufferUsages::COPY_SRC;
    let st = wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST;
    Bufs {
        fg_buf:      device.create_buffer(&wgpu::BufferDescriptor { label: Some("fg"),     size: is, usage: ro, mapped_at_creation: false }),
        bg_buf:      device.create_buffer(&wgpu::BufferDescriptor { label: Some("bg"),     size: is, usage: ro, mapped_at_creation: false }),
        local_buf:   device.create_buffer(&wgpu::BufferDescriptor { label: Some("local"),  size: is, usage: ro, mapped_at_creation: false }),
        global_buf:  device.create_buffer(&wgpu::BufferDescriptor { label: Some("global"), size: is, usage: ro, mapped_at_creation: false }),
        edge_buf:    device.create_buffer(&wgpu::BufferDescriptor { label: Some("edge"),   size: fs, usage: ro, mapped_at_creation: false }),
        uniform_buf: device.create_buffer(&wgpu::BufferDescriptor { label: Some("unif"),   size: us, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }),
        dst_buf:     device.create_buffer(&wgpu::BufferDescriptor { label: Some("dst"),    size: is, usage: rw, mapped_at_creation: false }),
        staging_buf: device.create_buffer(&wgpu::BufferDescriptor { label: Some("stage"),  size: is, usage: st, mapped_at_creation: false }),
        w, h,
    }
}
