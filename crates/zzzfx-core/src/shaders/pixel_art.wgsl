// zzzFX Pixel Art Style — GPU compute shader (two-pass)
//
// Architecture:
//   Pass 1 "cell_average": one invocation per CELL.
//     Computes the average RGBA color, applies contrast/saturation/dithering/
//     quantization, and writes the result to the cell_colors buffer.
//
//   Pass 2 "fill": one invocation per OUTPUT PIXEL.
//     Looks up the cell's quantized color from cell_colors, applies the
//     grid overlay (with alpha multiplied by cell alpha), and writes output.
//
// This avoids the redundant per-pixel cell averaging of the single-pass
// approach. For pixel_size=16×16, Pass 1 does the averaging once per cell
// (256× less work), and Pass 2 just reads a pre-computed value.
//
// Bindings:
//   @binding(0) src_buf     — source RGBA8 frame (storage buffer, u32)
//   @binding(1) uniforms    — Uniforms struct
//   @binding(2) dst_buf     — output RGBA8 frame (storage buffer, u32)
//   @binding(3) cell_colors — cell-averaged colors (storage buffer, u32)
//     Size: num_cols × num_rows entries

struct Uniforms {
    frame_width: u32,
    frame_height: u32,
    pixel_size_w: u32,
    pixel_size_h: u32,
    num_cols: u32,
    num_rows: u32,
    color_levels: u32,
    dithering: u32,       // 0=None, 1=Ordered, 2=FloydSteinberg
    dither_amount: f32,
    grid_thickness: f32,
    grid_color_r: f32,
    grid_color_g: f32,
    grid_color_b: f32,
    grid_color_a: f32,
    grid_offset_x: u32,
    grid_offset_y: u32,
    contrast: f32,
    saturation: f32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read>       src_buf:     array<u32>;
@group(0) @binding(1) var<uniform>             uniforms:    Uniforms;
@group(0) @binding(2) var<storage, read_write> dst_buf:     array<u32>;
@group(0) @binding(3) var<storage, read_write> cell_colors: array<u32>;

// 4×4 Bayer matrix for ordered dithering
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

// Helper to compute cell bounds given a grid index, offset, and pixel size
fn cell_bounds(idx: u32, offset: u32, pixel_size: u32, frame_size: u32) -> vec2<u32> {
    if offset > 0u && idx == 0u {
        return vec2<u32>(0u, min(offset, frame_size));
    }
    if offset > 0u {
        let start_x = offset + (idx - 1u) * pixel_size;
        let end_x = min(start_x + pixel_size, frame_size);
        return vec2<u32>(start_x, end_x);
    }
    let start_x = idx * pixel_size;
    let end_x = min(start_x + pixel_size, frame_size);
    return vec2<u32>(start_x, end_x);
}

// ── Pass 1: compute one cell's average color ──────────────────────────────

@compute @workgroup_size(8, 8)
fn cell_average(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cx = gid.x;
    let cy = gid.y;
    if cx >= uniforms.num_cols || cy >= uniforms.num_rows {
        return;
    }

    let xb = cell_bounds(cx, uniforms.grid_offset_x, uniforms.pixel_size_w, uniforms.frame_width);
    let yb = cell_bounds(cy, uniforms.grid_offset_y, uniforms.pixel_size_h, uniforms.frame_height);
    let cell_start_x = xb.x;
    let cell_end_x = xb.y;
    let cell_start_y = yb.x;
    let cell_end_y = yb.y;

    var sum_r = 0.0;
    var sum_g = 0.0;
    var sum_b = 0.0;
    var sum_a = 0.0;
    var count = 0u;

    for (var y = cell_start_y; y < cell_end_y; y += 1u) {
        let row_base = y * uniforms.frame_width;
        for (var x = cell_start_x; x < cell_end_x; x += 1u) {
            let rgba = unpack_rgba8(src_buf[row_base + x]);
            sum_r += rgba.r;
            sum_g += rgba.g;
            sum_b += rgba.b;
            sum_a += rgba.a;
            count += 1u;
        }
    }

    var avg_r = 0.0;
    var avg_g = 0.0;
    var avg_b = 0.0;
    var avg_a = 0.0;
    if count > 0u {
        let inv = 1.0 / f32(count);
        avg_r = sum_r * inv;
        avg_g = sum_g * inv;
        avg_b = sum_b * inv;
        avg_a = sum_a * inv;
    }

    // Apply contrast: (v - 0.5) * factor + 0.5
    let contrast_factor = 1.0 + (uniforms.contrast - 0.5) * 2.0;
    avg_r = clamp((avg_r - 0.5) * contrast_factor + 0.5, 0.0, 1.0);
    avg_g = clamp((avg_g - 0.5) * contrast_factor + 0.5, 0.0, 1.0);
    avg_b = clamp((avg_b - 0.5) * contrast_factor + 0.5, 0.0, 1.0);

    // Apply saturation: (v - lum) * factor + lum
    let sat_factor = 1.0 + (uniforms.saturation - 0.5) * 2.0;
    let lum = luminance(avg_r, avg_g, avg_b);
    avg_r = clamp((avg_r - lum) * sat_factor + lum, 0.0, 1.0);
    avg_g = clamp((avg_g - lum) * sat_factor + lum, 0.0, 1.0);
    avg_b = clamp((avg_b - lum) * sat_factor + lum, 0.0, 1.0);

    // Apply ordered dithering before quantization
    if uniforms.dithering == 1u {
        let bv = bayer_value(cx, cy);
        let noise = (bv - 0.5) * uniforms.dither_amount;
        avg_r = clamp(avg_r + noise, 0.0, 1.0);
        avg_g = clamp(avg_g + noise, 0.0, 1.0);
        avg_b = clamp(avg_b + noise, 0.0, 1.0);
    }

    // Quantize each channel — skip for FloydSteinberg (quantized during CPU diffusion)
    if uniforms.dithering != 2u {
        let levels_f = f32(uniforms.color_levels - 1u);
        avg_r = floor(avg_r * levels_f + 0.5) / levels_f;
        avg_g = floor(avg_g * levels_f + 0.5) / levels_f;
        avg_b = floor(avg_b * levels_f + 0.5) / levels_f;
    }

    // Write to cell_colors buffer
    let cell_idx = cy * uniforms.num_cols + cx;
    cell_colors[cell_idx] = pack_rgba8(vec4<f32>(avg_r, avg_g, avg_b, avg_a));
}

// ── Pass 2: fill destination pixels from cell colors ──────────────────────

@compute @workgroup_size(8, 8)
fn fill(@builtin(global_invocation_id) gid: vec3<u32>) {
    let px = gid.x;
    let py = gid.y;
    if px >= uniforms.frame_width || py >= uniforms.frame_height {
        return;
    }

    // Determine cell column from pixel x
    let (cell_x, cell_start_x, cell_end_x) = if uniforms.grid_offset_x > 0u && px < uniforms.grid_offset_x {
        (0u, 0u, uniforms.grid_offset_x)
    } else if uniforms.grid_offset_x > 0u {
        let xr = px - uniforms.grid_offset_x;
        let c = 1u + xr / uniforms.pixel_size_w;
        let sx = uniforms.grid_offset_x + (c - 1u) * uniforms.pixel_size_w;
        let ex = min(sx + uniforms.pixel_size_w, uniforms.frame_width);
        (c, sx, ex)
    } else {
        let c = px / uniforms.pixel_size_w;
        let sx = c * uniforms.pixel_size_w;
        let ex = min(sx + uniforms.pixel_size_w, uniforms.frame_width);
        (c, sx, ex)
    };

    // Determine cell row from pixel y
    let (cell_y, cell_start_y, cell_end_y) = if uniforms.grid_offset_y > 0u && py < uniforms.grid_offset_y {
        (0u, 0u, uniforms.grid_offset_y)
    } else if uniforms.grid_offset_y > 0u {
        let yr = py - uniforms.grid_offset_y;
        let r = 1u + yr / uniforms.pixel_size_h;
        let sy = uniforms.grid_offset_y + (r - 1u) * uniforms.pixel_size_h;
        let ey = min(sy + uniforms.pixel_size_h, uniforms.frame_height);
        (r, sy, ey)
    } else {
        let r = py / uniforms.pixel_size_h;
        let sy = r * uniforms.pixel_size_h;
        let ey = min(sy + uniforms.pixel_size_h, uniforms.frame_height);
        (r, sy, ey)
    };

    // Look up pre-computed cell color
    let cell_idx = cell_y * uniforms.num_cols + cell_x;
    let packed = cell_colors[cell_idx];
    var color = unpack_rgba8(packed);

    // Apply grid at right/bottom cell boundaries.
    // Grid alpha is multiplied by the cell's average alpha so that
    // transparent regions show a proportionally weaker grid.
    let local_x = px - cell_start_x;
    let local_y = py - cell_start_y;
    let cell_w = cell_end_x - cell_start_x;
    let cell_h = cell_end_y - cell_start_y;
    let grid_px_w = uniforms.grid_thickness * f32(uniforms.pixel_size_w);
    let grid_px_h = uniforms.grid_thickness * f32(uniforms.pixel_size_h);

    let is_grid = f32(local_x) >= f32(cell_w) - grid_px_w
               || f32(local_y) >= f32(cell_h) - grid_px_h;
    if is_grid {
        let ga = uniforms.grid_color_a * color.a;
        color.r = color.r * (1.0 - ga) + uniforms.grid_color_r * ga;
        color.g = color.g * (1.0 - ga) + uniforms.grid_color_g * ga;
        color.b = color.b * (1.0 - ga) + uniforms.grid_color_b * ga;
    }

    let idx = py * uniforms.frame_width + px;
    dst_buf[idx] = pack_rgba8(color);
}
