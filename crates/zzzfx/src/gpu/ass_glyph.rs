//! GPU-accelerated ASS subtitle glyph compositing.
//!
//! Uploads glyph bitmaps + metadata and composites fill, shadow, and outline
//! directly onto the output buffer via a compute shader.

use std::cell::RefCell;
use std::sync::{atomic::AtomicBool, atomic::Ordering, OnceLock};

// ---------------------------------------------------------------------------
// Uniforms + GlyphData (match ass_glyph.wgsl layout)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphUniforms {
    output_width: u32,
    output_height: u32,
    glyph_count: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphGpuData {
    pub glyph_offset: u32,
    pub bitmap_w: u32,
    pub bitmap_h: u32,
    pub pos_x: i32,
    pub pos_y: i32,
    pub fill_color: u32,
    pub outline_color: u32,
    pub shadow_color: u32,
    pub outline_radius: i32,
    pub shadow_dx: f32,
    pub shadow_dy: f32,
    pub flags: u32,
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
        label: Some("ass_glyph"),
        source: super::load_shader(include_str!("../shaders/ass_glyph.wgsl")),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ass_glyph"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let bg_layout = pipeline.get_bind_group_layout(0);
    RES.set(Res { pipeline, bg_layout }).map_err(|_| "ass_glyph: init race".to_string())?;
    RES.get().ok_or_else(|| "ass_glyph: init race".to_string())
}

// ---------------------------------------------------------------------------
// Per-thread buffer pool — no Mutex, no contention
// ---------------------------------------------------------------------------

struct Bufs {
    glyphs_buf: wgpu::Buffer,
    bitmaps_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    dst_buf: wgpu::Buffer,
    staging_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    glyph_capacity: u64,
    bitmap_capacity: u64,
}

thread_local! {
    static BUF_POOL: RefCell<Option<Bufs>> = const { RefCell::new(None) };
}

fn take_or_create_bufs(
    device: &wgpu::Device,
    res: &Res,
    width: u32,
    height: u32,
    glyph_cap: u64,
    bitmap_cap: u64,
) -> Bufs {
    BUF_POOL.with(|cell| {
        let mut bufs = cell.borrow_mut().take();
        if bufs.as_ref().map_or(true, |b| {
            b.width != width || b.height != height || b.glyph_capacity < glyph_cap || b.bitmap_capacity < bitmap_cap
        }) {
            bufs = Some(create_bufs(device, res, width, height, glyph_cap, bitmap_cap));
        }
        bufs.unwrap()
    })
}

fn return_bufs(bufs: Bufs) {
    let _ = BUF_POOL.try_with(|cell| {
        *cell.borrow_mut() = Some(bufs);
    });
}

fn create_bufs(
    device: &wgpu::Device,
    res: &Res,
    width: u32,
    height: u32,
    glyph_cap: u64,
    bitmap_cap: u64,
) -> Bufs {
    let buf_size = (width * height * 4) as u64;
    let glyph_cap = glyph_cap.max(8192);
    let bitmap_cap = bitmap_cap.max(65536);

    let glyphs_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ass_glyphs_meta"),
        size: glyph_cap,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bitmaps_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ass_glyphs_bitmaps"),
        size: bitmap_cap,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ass_glyph_uniforms"),
        size: std::mem::size_of::<GlyphUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let dst_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ass_glyph_dst"),
        size: buf_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ass_glyph_staging"),
        size: buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ass_glyph"),
        layout: &res.bg_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: glyphs_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: bitmaps_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: dst_buf.as_entire_binding() },
        ],
    });

    Bufs { glyphs_buf, bitmaps_buf, uniform_buf, dst_buf, staging_buf, bind_group, width, height, glyph_capacity: glyph_cap, bitmap_capacity: bitmap_cap }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn try_ass_glyph_gpu_composite(
    glyphs: &[GlyphGpuData],
    bitmaps: &[u8],
    output: &mut [u8],
    width: u32,
    height: u32,
) -> Result<bool, String> {
    if !GPU_AVAILABLE.load(Ordering::Relaxed)
        || !super::SHARED_GPU_AVAILABLE.load(Ordering::Relaxed)
        || !super::is_shared_device_ready()
    {
        return Ok(false);
    }
    if glyphs.is_empty() {
        return Ok(true);
    }

    // Validate output buffer size
    let needed = (width * height * 4) as usize;
    if output.len() < needed {
        return Err("output buffer too small".to_string());
    }

    let (device, queue) = super::get_or_init_shared_device()?;
    let res = get_res(device).map_err(|e| { GPU_AVAILABLE.store(false, Ordering::Relaxed); e })?;

    let glyphs_bytes = bytemuck::cast_slice(glyphs);
    let glyph_needed = (glyphs_bytes.len() as u64).next_multiple_of(256);
    let bitmap_needed = if bitmaps.is_empty() { 0 } else { ((bitmaps.len() + 3) / 4 * 4) as u64 };

    let bufs = take_or_create_bufs(device, res, width, height, glyph_needed, bitmap_needed);

    let uniforms = GlyphUniforms {
        output_width: width,
        output_height: height,
        glyph_count: glyphs.len() as u32,
        _pad: 0,
    };

    let dst_size = (width * height * 4) as u64;

    // Upload glyph metadata
    queue.write_buffer(&bufs.glyphs_buf, 0, glyphs_bytes);

    // Convert bitmaps to u32 for WGSL (u8 not valid in storage buffers)
    thread_local! {
        static BITMAP_WORDS: RefCell<Vec<u32>> = RefCell::new(Vec::new());
    }
    if !bitmaps.is_empty() {
        BITMAP_WORDS.with(|bw| {
            let mut bw = bw.borrow_mut();
            bw.clear();
            bw.reserve((bitmaps.len() + 3) / 4);
            bw.extend(
                bitmaps.chunks(4).map(|chunk| {
                    let b0 = chunk.first().copied().unwrap_or(0) as u32;
                    let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
                    let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
                    let b3 = chunk.get(3).copied().unwrap_or(0) as u32;
                    b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
                })
            );
            queue.write_buffer(&bufs.bitmaps_buf, 0, bytemuck::cast_slice(&*bw));
        });
    }

    // Upload output (initial state)
    queue.write_buffer(&bufs.dst_buf, 0, &output[..dst_size as usize]);

    // Upload uniforms
    queue.write_buffer(&bufs.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

    // Dispatch + copy to staging in one encoder + submit
    {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&res.pipeline);
            pass.set_bind_group(0, &bufs.bind_group, &[]);
            pass.dispatch_workgroups(glyphs.len() as u32, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&bufs.dst_buf, 0, &bufs.staging_buf, 0, dst_size);
        queue.submit(std::iter::once(encoder.finish()));
    }

    let result = super::blocking_readback(device, &bufs.staging_buf, dst_size, &mut output[..dst_size as usize]);
    return_bufs(bufs);
    result.map(|()| true)
}
