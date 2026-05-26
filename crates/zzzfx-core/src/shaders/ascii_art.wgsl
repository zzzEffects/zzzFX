// zzzFX ASCII Art — GPU compute shader
//
// Architecture:
//   One workgroup invocation = one character cell.
//   Each invocation samples the source region for its cell, computes the
//   average luminance and colour, maps luminance to a character index,
//   then stamps the glyph from a pre-baked atlas (storage buffer) into the output.
//
// Bindings:
//   @binding(0) src_buf   — source RGBA8 frame (storage buffer, u32)
//   @binding(1) uniforms  — Uniforms struct
//   @binding(2) atlas     — glyph atlas (storage buffer, u32, row-major RGBA8)
//   @binding(3) dst_buf   — output RGBA8 frame (storage buffer, u32)

struct Uniforms {
    frame_width: u32,
    frame_height: u32,
    cell_size: u32,
    atlas_cols: u32,
    atlas_cell_w: u32,
    atlas_cell_h: u32,
    atlas_max_w: u32,  // padded width of atlas in pixels
    atlas_max_h: u32,  // height of atlas in pixels
    charset_len: u32,
    brightness: f32,
    contrast: f32,
    invert_luma: u32,
    color_mode: u32,
    bg_alpha: f32,
    _pad: array<u32, 1>,
}

@group(0) @binding(0) var<storage, read>       src_buf: array<u32>;
@group(0) @binding(1) var<uniform>             uniforms: Uniforms;
@group(0) @binding(2) var<storage, read>       atlas: array<u32>;
@group(0) @binding(3) var<storage, read_write> dst_buf: array<u32>;

// Luminance weights (Rec. 709)
const R_WEIGHT: f32 = 0.2126;
const G_WEIGHT: f32 = 0.7152;
const B_WEIGHT: f32 = 0.0722;

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

// Sample atlas at integer (px, py) coordinates
fn sample_atlas(px: u32, py: u32) -> vec4<f32> {
    let idx = py * uniforms.atlas_max_w + px;
    return unpack_rgba(atlas[idx]);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cell_x = gid.x;
    let cell_y = gid.y;
    let cols = (uniforms.frame_width  + uniforms.cell_size - 1u) / uniforms.cell_size;
    let rows = (uniforms.frame_height + uniforms.cell_size - 1u) / uniforms.cell_size;

    if cell_x >= cols || cell_y >= rows {
        return;
    }

    // ── Stage 1: Sample cell average ────────────────────────────────
    let start_x = cell_x * uniforms.cell_size;
    let start_y = cell_y * uniforms.cell_size;
    let end_x = min(start_x + uniforms.cell_size, uniforms.frame_width);
    let end_y = min(start_y + uniforms.cell_size, uniforms.frame_height);

    var sum_r = 0.0f;
    var sum_g = 0.0f;
    var sum_b = 0.0f;
    var sum_luma = 0.0f;
    var count = 0u;

    for (var y = start_y; y < end_y; y += 1u) {
        let row_base = y * uniforms.frame_width;
        for (var x = start_x; x < end_x; x += 1u) {
            let rgba = unpack_rgba(src_buf[row_base + x]);
            sum_r += rgba.r;
            sum_g += rgba.g;
            sum_b += rgba.b;
            sum_luma += R_WEIGHT * rgba.r + G_WEIGHT * rgba.g + B_WEIGHT * rgba.b;
            count += 1u;
        }
    }

    var avg_r = 0.0f;
    var avg_g = 0.0f;
    var avg_b = 0.0f;
    var avg_luma = 0.0f;
    if count > 0u {
        let inv = 1.0 / f32(count);
        avg_r = sum_r * inv;
        avg_g = sum_g * inv;
        avg_b = sum_b * inv;
        avg_luma = sum_luma * inv;
    }

    // ── Stage 2: Brightness / contrast ──────────────────────────────
    let contrast_factor = 1.0 + (uniforms.contrast - 0.5) * 2.0;
    var adjusted = (avg_luma - 0.5) * contrast_factor + 0.5 + (uniforms.brightness - 0.5);
    adjusted = clamp(adjusted, 0.0, 1.0);

    // ── Stage 3: Map luminance → char index ─────────────────────────
    if uniforms.invert_luma != 0u {
        adjusted = 1.0 - adjusted;
    }
    let charset_len_f = f32(uniforms.charset_len);
    let raw_idx = u32(round(adjusted * (charset_len_f - 1.0)));
    let char_idx = min(raw_idx, uniforms.charset_len - 1u);
    let char_idx = uniforms.charset_len - 1u - char_idx;

    // ── Stage 4: Foreground colour ──────────────────────────────────
    var fg_r: f32;
    var fg_g: f32;
    var fg_b: f32;
    if uniforms.color_mode == 0u {
        fg_r = 1.0; fg_g = 1.0; fg_b = 1.0;        // Grayscale
    } else if uniforms.color_mode == 1u {
        fg_r = avg_r; fg_g = avg_g; fg_b = avg_b;    // Colored
    } else {
        fg_r = 0.0; fg_g = 1.0; fg_b = 0.0;         // Green Terminal
    }

    // ── Stage 5: Stamp glyph into output ────────────────────────────
    let atlas_base_x = char_idx * uniforms.cell_size;

    for (var y = start_y; y < end_y; y += 1u) {
        let local_y = y - start_y;
        let row_base = y * uniforms.frame_width;

        for (var x = start_x; x < end_x; x += 1u) {
            let local_x = x - start_x;
            let apx = atlas_base_x + local_x;  // sample atlas pixel
            let apy = local_y;
            let glyph_rgba = sample_atlas(apx, apy);
            let fa = glyph_rgba.a;

            let bg_a = uniforms.bg_alpha;
            let out_r = fg_r * fa;
            let out_g = fg_g * fa;
            let out_b = fg_b * fa;
            let out_a = fa + (1.0 - fa) * bg_a;

            dst_buf[row_base + x] = pack_rgba(vec4<f32>(out_r, out_g, out_b, out_a));
        }
    }
}
