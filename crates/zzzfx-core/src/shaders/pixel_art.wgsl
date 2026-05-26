// zzzFX Pixel Art Style — GPU compute shader
//
// Architecture:
//   One workgroup invocation = one output pixel.
//   Each invocation looks up its containing cell, computes the quantized
//   color for that cell (with optional ordered dithering), and writes the result.
//   Grid lines are drawn at cell boundaries.
//
// Bindings:
//   @binding(0) src_buf   — source RGBA8 frame (storage buffer, u32)
//   @binding(1) uniforms  — Uniforms struct
//   @binding(2) dst_buf   — output RGBA8 frame (storage buffer, u32)

struct Uniforms {
    frame_width: u32,
    frame_height: u32,
    pixel_size_w: u32,
    pixel_size_h: u32,
    color_levels: u32,
    dithering: u32,       // 0=None, 1=Ordered
    dither_amount: f32,
    show_grid: u32,        // bool
    grid_thickness: f32,
    grid_color_r: f32,
    grid_color_g: f32,
    grid_color_b: f32,
    grid_color_a: f32,
    contrast: f32,
    saturation: f32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read>       src_buf: array<u32>;
@group(0) @binding(1) var<uniform>             uniforms: Uniforms;
@group(0) @binding(2) var<storage, read_write> dst_buf: array<u32>;

// Luminance weights (Rec. 709)
const R_WEIGHT: f32 = 0.2126;
const G_WEIGHT: f32 = 0.7152;
const B_WEIGHT: f32 = 0.0722;

// 4x4 Bayer matrix
fn bayer_value(x: u32, y: u32) -> f32 {
    let bayer = array<u32, 16>(
        0u,  8u,  2u, 10u,
        12u, 4u, 14u, 6u,
        3u, 11u, 1u, 9u,
        15u, 7u, 13u, 5u,
    );
    let idx = (y % 4u) * 4u + (x % 4u);
    return f32(bayer[idx]) / 16.0;
}

fn unpack_rgba(packed: u32) -> vec4<f32> {
    let r = f32((packed >>  0u) & 0xFFu) / 255.0;
    let g = f32((packed >>  8u) & 0xFFu) / 255.0;
    let b = f32((packed >> 16u) & 0xFFu) / 255.0;
    let a = f32((packed >> 24u) & 0xFFu) / 255.0;
    return vec4<f32>(r, g, b, a);
}

fn pack_rgba(color: vec4<f32>) -> u32 {
    let r = u32(clamp(color.r, 0.0, 1.0) * 255.0 + 0.5);
    let g = u32(clamp(color.g, 0.0, 1.0) * 255.0 + 0.5);
    let b = u32(clamp(color.b, 0.0, 1.0) * 255.0 + 0.5);
    let a = u32(clamp(color.a, 0.0, 1.0) * 255.0 + 0.5);
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let px = gid.x;
    let py = gid.y;
    if px >= uniforms.frame_width || py >= uniforms.frame_height {
        return;
    }

    // Determine which cell this pixel belongs to
    let cell_x = px / uniforms.pixel_size_w;
    let cell_y = py / uniforms.pixel_size_h;
    let cell_start_x = cell_x * uniforms.pixel_size_w;
    let cell_start_y = cell_y * uniforms.pixel_size_h;
    let cell_end_x = min(cell_start_x + uniforms.pixel_size_w, uniforms.frame_width);
    let cell_end_y = min(cell_start_y + uniforms.pixel_size_h, uniforms.frame_height);

    // Compute average color of the cell
    var sum_r = 0.0f;
    var sum_g = 0.0f;
    var sum_b = 0.0f;
    var a_sum = 0.0f;
    var count = 0u;

    for (var y = cell_start_y; y < cell_end_y; y += 1u) {
        let row_base = y * uniforms.frame_width;
        for (var x = cell_start_x; x < cell_end_x; x += 1u) {
            let rgba = unpack_rgba(src_buf[row_base + x]);
            sum_r += rgba.r;
            sum_g += rgba.g;
            sum_b += rgba.b;
            a_sum += rgba.a;
            count += 1u;
        }
    }

    var avg_r = 0.0f;
    var avg_g = 0.0f;
    var avg_b = 0.0f;
    var avg_a = 0.0f;
    if count > 0u {
        let inv = 1.0 / f32(count);
        avg_r = sum_r * inv;
        avg_g = sum_g * inv;
        avg_b = sum_b * inv;
        avg_a = a_sum * inv;
    }

    // Apply contrast: (v - 0.5) * factor + 0.5
    let contrast_factor = 1.0 + (uniforms.contrast - 0.5) * 2.0;
    avg_r = clamp((avg_r - 0.5) * contrast_factor + 0.5, 0.0, 1.0);
    avg_g = clamp((avg_g - 0.5) * contrast_factor + 0.5, 0.0, 1.0);
    avg_b = clamp((avg_b - 0.5) * contrast_factor + 0.5, 0.0, 1.0);

    // Apply saturation: (v - lum) * factor + lum
    let sat_factor = 1.0 + (uniforms.saturation - 0.5) * 2.0;
    let lum = R_WEIGHT * avg_r + G_WEIGHT * avg_g + B_WEIGHT * avg_b;
    avg_r = clamp((avg_r - lum) * sat_factor + lum, 0.0, 1.0);
    avg_g = clamp((avg_g - lum) * sat_factor + lum, 0.0, 1.0);
    avg_b = clamp((avg_b - lum) * sat_factor + lum, 0.0, 1.0);

    // Apply ordered dithering before quantization
    if uniforms.dithering == 1u {
        let bv = bayer_value(cell_x, cell_y);
        let noise = (bv - 0.5) * uniforms.dither_amount;
        avg_r = clamp(avg_r + noise, 0.0, 1.0);
        avg_g = clamp(avg_g + noise, 0.0, 1.0);
        avg_b = clamp(avg_b + noise, 0.0, 1.0);
    }

    // Quantize each channel
    let levels_f = f32(uniforms.color_levels - 1u);
    avg_r = floor(avg_r * levels_f + 0.5) / levels_f;
    avg_g = floor(avg_g * levels_f + 0.5) / levels_f;
    avg_b = floor(avg_b * levels_f + 0.5) / levels_f;

    // Apply grid at right/bottom cell boundaries
    if uniforms.show_grid != 0u {
        let local_x = px - cell_start_x;
        let local_y = py - cell_start_y;
        let cell_w = cell_end_x - cell_start_x;
        let cell_h = cell_end_y - cell_start_y;
        let grid_px_w = uniforms.grid_thickness * f32(uniforms.pixel_size_w);
        let grid_px_h = uniforms.grid_thickness * f32(uniforms.pixel_size_h);

        let is_grid = f32(local_x) >= f32(cell_w) - grid_px_w
                   || f32(local_y) >= f32(cell_h) - grid_px_h;
        if is_grid {
            let ga = uniforms.grid_color_a;
            avg_r = avg_r * (1.0 - ga) + uniforms.grid_color_r * ga;
            avg_g = avg_g * (1.0 - ga) + uniforms.grid_color_g * ga;
            avg_b = avg_b * (1.0 - ga) + uniforms.grid_color_b * ga;
        }
    }

    let idx = py * uniforms.frame_width + px;
    dst_buf[idx] = pack_rgba(vec4<f32>(avg_r, avg_g, avg_b, avg_a));
}
