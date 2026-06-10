use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

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
    let bg_layout = pipeline.get_bind_group_layout(0);
    RES.set(Res { pipeline, bg_layout }).map_err(|_| "midi_display: init race".to_string())?;
    RES.get().ok_or_else(|| "midi_display: init race".to_string())
}

// ---------------------------------------------------------------------------
// Per-thread buffer pool — no Mutex, no contention
// ---------------------------------------------------------------------------

struct Bufs {
    uniform_buf: wgpu::Buffer,
    note_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    output_w: u32,
    output_h: u32,
    note_capacity: u32,
}

thread_local! {
    static BUF_POOL: RefCell<Option<Bufs>> = const { RefCell::new(None) };
}

const MAX_VISIBLE_NOTES: u64 = 8192;

fn take_or_create_bufs(device: &wgpu::Device, res: &Res, w: u32, h: u32, note_count: u32) -> Bufs {
    BUF_POOL.with(|cell| {
        let mut bufs = cell.borrow_mut().take();
        if bufs.as_ref().map_or(true, |b| b.output_w != w || b.output_h != h || note_count > b.note_capacity) {
            bufs = Some(create_bufs(device, res, w, h, note_count));
        }
        bufs.unwrap()
    })
}

fn return_bufs(bufs: Bufs) {
    let _ = BUF_POOL.try_with(|cell| {
        *cell.borrow_mut() = Some(bufs);
    });
}

fn create_bufs(device: &wgpu::Device, res: &Res, w: u32, h: u32, note_hint: u32) -> Bufs {
    let image_size = w as u64 * h as u64 * 4;
    let note_capacity = (note_hint.max(16) as u64).max(MAX_VISIBLE_NOTES);
    let note_buf_size = note_capacity * std::mem::size_of::<NoteGpu>() as u64;

    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("midi_uniforms"),
        size: std::mem::size_of::<MidiDisplayUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let note_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("midi_notes"),
        size: note_buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let dst_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("midi_dst"),
        size: image_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("midi_staging"),
        size: image_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("midi_display"),
        layout: &res.bg_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: note_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: dst_buf.as_entire_binding() },
        ],
    });

    Bufs { uniform_buf, note_buf, dst_buf, staging_buf, bind_group, output_w: w, output_h: h, note_capacity: note_capacity as u32 }
}

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

    let note_count = note_data.len() as u32;
    if note_count == 0 {
        return Ok(false);
    }

    let (device, queue) = super::get_or_init_shared_device()?;
    let res = get_res(device).map_err(|e| { GPU_AVAILABLE.store(false, Ordering::Relaxed); e })?;
    let bufs = take_or_create_bufs(device, res, dst_w, dst_h, note_count);

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

    queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
    queue.write_buffer(&bufs.note_buf, 0, bytemuck::cast_slice(note_data));

    let image_size = dst_w as u64 * dst_h as u64 * 4;

    // Dispatch compute + copy to staging (single encoder)
    {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
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
