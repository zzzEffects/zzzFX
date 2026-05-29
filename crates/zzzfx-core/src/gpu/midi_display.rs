use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------------------
// GPU structs (must match WGSL layout exactly)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MidiDisplayUniforms {
    pub dst_w: u32,
    pub dst_h: u32,
    pub note_count: u32,
    pub bg_r: f32, pub bg_g: f32, pub bg_b: f32, pub bg_a: f32,
    pub keyboard_start: i32,
    pub keyboard_size: i32,
    pub orientation: u32,
    pub key_range_min: i32,
    pub key_range_max: i32,
    pub pixels_per_key: f32,
    pub indicator_pos: i32,
    pub time_axis_offset: i32,
    pub _pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NoteGpu {
    pub x: i32, pub y: i32, pub w: i32, pub h: i32,
    pub corner_radius: f32,
    pub border_thickness: f32,
    pub fill_r: f32, pub fill_g: f32, pub fill_b: f32, pub fill_a: f32,
    pub border_r: f32, pub border_g: f32, pub border_b: f32, pub border_a: f32,
}

// ---------------------------------------------------------------------------
// GPU context
// ---------------------------------------------------------------------------

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(true);

struct GpuContext {
    device: &'static wgpu::Device,
    queue: &'static wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bufs: GpuBuffers,
    bind_group: Option<wgpu::BindGroup>,
}

struct GpuBuffers {
    uniform_buf: wgpu::Buffer,
    note_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    output_w: u32,
    output_h: u32,
    note_capacity: u32,
}

static GPU_CTX: OnceLock<Mutex<GpuContext>> = OnceLock::new();

const MAX_VISIBLE_NOTES: u64 = 8192;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn try_midi_display_gpu_render(
    dst_w: u32,
    dst_h: u32,
    bg_r: f32, bg_g: f32, bg_b: f32, bg_a: f32,
    keyboard_start: i32,
    keyboard_size: i32,
    orientation: u32,
    key_range_min: i32,
    key_range_max: i32,
    pixels_per_key: f32,
    indicator_pos: i32,
    time_axis_offset: i32,
    note_data: &[NoteGpu],
    dst: &mut [u8],
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed) {
        return Ok(false);
    }

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

    let note_count = note_data.len() as u32;
    if note_count == 0 {
        return Ok(false); // nothing to render, let CPU handle simple background
    }

    // Recreate buffers if dimensions or note count changed
    if guard.bufs.output_w != dst_w || guard.bufs.output_h != dst_h || note_count > guard.bufs.note_capacity
    {
        guard.bufs = create_buffers(guard.device, dst_w, dst_h, note_count);
        guard.bind_group = None;
    }

    let uniforms = MidiDisplayUniforms {
        dst_w, dst_h,
        note_count,
        bg_r, bg_g, bg_b, bg_a,
        keyboard_start, keyboard_size,
        orientation,
        key_range_min, key_range_max,
        pixels_per_key,
        indicator_pos,
        time_axis_offset,
        _pad: 0,
    };

    // Upload data
    guard.queue.write_buffer(&guard.bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
    guard.queue.write_buffer(&guard.bufs.note_buf, 0, bytemuck::cast_slice(note_data));

    // Create or reuse bind group
    if guard.bind_group.is_none() {
        let layout = guard.pipeline.get_bind_group_layout(0);
        guard.bind_group = Some(guard.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("midi_display"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: guard.bufs.uniform_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: guard.bufs.note_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: guard.bufs.dst_buf.as_entire_binding() },
            ],
        }));
    }

    let image_size = dst_w as u64 * dst_h as u64 * 4;

    // Dispatch compute + copy to staging
    {
        let mut encoder = guard.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&guard.pipeline);
            pass.set_bind_group(0, guard.bind_group.as_ref().unwrap(), &[]);
            pass.dispatch_workgroups((dst_w + 15) / 16, (dst_h + 15) / 16, 1);
        }
        encoder.copy_buffer_to_buffer(&guard.bufs.dst_buf, 0, &guard.bufs.staging_buf, 0, image_size);
        guard.queue.submit(std::iter::once(encoder.finish()));
    }

    // Blocking readback
    let staging = &guard.bufs.staging_buf;
    staging.slice(..image_size).map_async(wgpu::MapMode::Read, |_| {});
    let _ = guard.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_millis(100)),
    });
    let mapped = staging.slice(..image_size).get_mapped_range();
    let buf_size = image_size as usize;
    if buf_size <= dst.len() {
        dst[..buf_size].copy_from_slice(&mapped);
    }
    drop(mapped);
    staging.unmap();

    Ok(true)
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

fn get_or_init_gpu() -> Result<&'static Mutex<GpuContext>, String> {
    if let Some(ctx) = GPU_CTX.get() {
        return Ok(ctx);
    }
    static INIT_LOCK: Mutex<()> = Mutex::new(());
    let _guard = INIT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ctx) = GPU_CTX.get() {
        return Ok(ctx);
    }

    let (device, queue) = super::get_or_init_shared_device()?;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("midi_display"),
        source: super::load_shader(include_str!("../shaders/midi_display.wgsl")),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("midi_display"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bufs = create_buffers(device, 256, 256, 16);

    let _ = GPU_CTX.set(Mutex::new(GpuContext {
        device,
        queue,
        pipeline,
        bufs,
        bind_group: None,
    }));
    Ok(GPU_CTX.get().unwrap())
}

fn create_buffers(device: &wgpu::Device, w: u32, h: u32, note_hint: u32) -> GpuBuffers {
    let image_size = w as u64 * h as u64 * 4;
    let note_capacity = (note_hint.max(16) as u64).max(MAX_VISIBLE_NOTES);
    GpuBuffers {
        uniform_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("midi_uniforms"),
            size: std::mem::size_of::<MidiDisplayUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        note_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("midi_notes"),
            size: note_capacity * std::mem::size_of::<NoteGpu>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        dst_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("midi_dst"),
            size: image_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }),
        staging_buf: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("midi_staging"),
            size: image_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        output_w: w,
        output_h: h,
        note_capacity: note_capacity as u32,
    }
}
