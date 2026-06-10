use std::cell::RefCell;
use std::sync::{atomic::AtomicBool, atomic::Ordering, OnceLock};

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

struct Res {
    pipeline: wgpu::ComputePipeline,
    bg_layout: wgpu::BindGroupLayout,
}

static RES: OnceLock<Res> = OnceLock::new();

fn get_res(device: &wgpu::Device) -> Result<&'static Res, String> {
    if let Some(r) = RES.get() { return Ok(r); }
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ambient_light"),
        source: super::load_shader(include_str!("../shaders/ambient_light.wgsl")),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ambient"), layout: None, module: &shader, entry_point: Some("main"),
        compilation_options: Default::default(), cache: None,
    });
    let bg_layout = pipeline.get_bind_group_layout(0);
    RES.set(Res { pipeline, bg_layout }).map_err(|_| "ambient_light: init race".to_string())?;
    RES.get().ok_or_else(|| "ambient_light: init race".to_string())
}

struct Bufs {
    fg_buf: wgpu::Buffer, bg_buf: wgpu::Buffer,
    local_buf: wgpu::Buffer, global_buf: wgpu::Buffer,
    edge_buf: wgpu::Buffer, uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer, staging_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
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
    let is = (w * h * 4) as u64;
    let fs = (w * h * 4) as u64;
    let us = std::mem::size_of::<Uniforms>() as u64;
    let ro = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
    let rw = ro | wgpu::BufferUsages::COPY_SRC;
    let st = wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST;
    let uu = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
    let fg = device.create_buffer(&wgpu::BufferDescriptor { label: Some("fg"), size: is, usage: ro, mapped_at_creation: false });
    let bg = device.create_buffer(&wgpu::BufferDescriptor { label: Some("bg"), size: is, usage: ro, mapped_at_creation: false });
    let local = device.create_buffer(&wgpu::BufferDescriptor { label: Some("local"), size: is, usage: ro, mapped_at_creation: false });
    let global = device.create_buffer(&wgpu::BufferDescriptor { label: Some("global"), size: is, usage: ro, mapped_at_creation: false });
    let edge = device.create_buffer(&wgpu::BufferDescriptor { label: Some("edge"), size: fs, usage: ro, mapped_at_creation: false });
    let uniform = device.create_buffer(&wgpu::BufferDescriptor { label: Some("unif"), size: us, usage: uu, mapped_at_creation: false });
    let dst = device.create_buffer(&wgpu::BufferDescriptor { label: Some("dst"), size: is, usage: rw, mapped_at_creation: false });
    let staging = device.create_buffer(&wgpu::BufferDescriptor { label: Some("stage"), size: is, usage: st, mapped_at_creation: false });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ambient"), layout: &res.bg_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: fg.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: bg.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: local.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: global.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 4, resource: edge.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 5, resource: uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 6, resource: dst.as_entire_binding() },
        ],
    });
    Bufs { fg_buf: fg, bg_buf: bg, local_buf: local, global_buf: global, edge_buf: edge, uniform_buf: uniform, dst_buf: dst, staging_buf: staging, bind_group, w, h }
}

pub fn try_ambient_light_gpu_render(
    ambient_local: &[[f32; 3]], ambient_global: &[[f32; 3]],
    fg: &[u8], bg: &[u8], edge_factor: &[f32],
    settings: &AmbientLight, dst: &mut [u8],
    width: usize, height: usize,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed) { return Ok(false); }
    let w = width as u32; let h = height as u32;

    let (device, queue) = get_or_init_shared_device()?;
    let res = get_res(device).map_err(|e| { GPU_AVAILABLE.store(false, Ordering::Relaxed); e })?;
    let bufs = take_or_create_bufs(device, res, w, h);

    let image_size = (w * h * 4) as u64;
    let wx = (w + 15) / 16; let wy = (h + 15) / 16;

    let local_u8: Vec<u8> = ambient_local.iter().flat_map(|c| {
        [(c[0].clamp(0.0,1.0)*255.0).round() as u8, (c[1].clamp(0.0,1.0)*255.0).round() as u8,
         (c[2].clamp(0.0,1.0)*255.0).round() as u8, 255u8]
    }).collect();
    let global_u8: Vec<u8> = ambient_global.iter().flat_map(|c| {
        [(c[0].clamp(0.0,1.0)*255.0).round() as u8, (c[1].clamp(0.0,1.0)*255.0).round() as u8,
         (c[2].clamp(0.0,1.0)*255.0).round() as u8, 255u8]
    }).collect();

    queue.write_buffer(&bufs.fg_buf, 0, &fg[..(image_size as usize).min(fg.len())]);
    queue.write_buffer(&bufs.bg_buf, 0, &bg[..(image_size as usize).min(bg.len())]);
    queue.write_buffer(&bufs.local_buf, 0, &local_u8);
    queue.write_buffer(&bufs.global_buf, 0, &global_u8);
    queue.write_buffer(&bufs.edge_buf, 0, bytemuck::cast_slice(edge_factor));

    let u = Uniforms {
        width: w, height: h,
        intensity: settings.intensity.clamp(0.0, 1.0),
        light_wrap: settings.light_wrap.clamp(0.0, 1.0),
        ambient_tint: settings.ambient_tint.clamp(0.0, 1.0),
        brightness: settings.brightness.clamp(0.0, 2.0),
        fg_opacity: settings.fg_opacity.clamp(0.0, 1.0),
        bg_opacity: settings.bg_opacity.clamp(0.0, 1.0),
    };
    queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&u));

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
        pass.set_pipeline(&res.pipeline);
        pass.set_bind_group(0, &bufs.bind_group, &[]);
        pass.dispatch_workgroups(wx, wy, 1);
    }
    enc.copy_buffer_to_buffer(&bufs.dst_buf, 0, &bufs.staging_buf, 0, image_size);
    queue.submit(std::iter::once(enc.finish()));

    let result = super::blocking_readback(device, &bufs.staging_buf, image_size, &mut dst[..image_size as usize]);
    return_bufs(bufs);
    result.map(|()| true)
}
