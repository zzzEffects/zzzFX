// Halftone effect — single-pass compute shader.
// Shared functions (unpack_rgba8, pack_rgba8, luminance) are prepended from shared.wgsl.

struct Uniforms {
    width: u32,
    height: u32,
    cell_spacing: f32,
    half_cell: f32,
    // cos/sin for Luminance mode and B channel (base angle)
    cos0: f32, sin0: f32,
    // cos/sin for R channel (angle + 15deg)
    cos1: f32, sin1: f32,
    // cos/sin for G channel (angle + 45deg)
    cos2: f32, sin2: f32,
    // cos/sin for B channel (angle + 75deg) is cos3/sin3 — wait, B actually uses the full set.
    // Let me reconsider: B channel uses angle+75. So we need 4 pairs total.
    cos3: f32, sin3: f32,
    // offset
    ax: f32, ay: f32,
    dot_shape: u32,      // 0=Circle, 1=Square, 2=Diamond
    channel_mode: u32,   // 0=Luminance, 1=RGB
    invert: u32,
    contrast_factor: f32,
    smoothness: f32,
    blend: f32,
}

@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<uniform> u: Uniforms;
@group(0) @binding(2) var<storage, read_write> dst: array<u32>;

fn contrast(v: f32) -> f32 {
    return clamp((v - 0.5) * u.contrast_factor + 0.5, 0.0, 1.0);
}

fn dot_coverage(px: f32, py: f32, brightness: f32, cos_a: f32, sin_a: f32) -> f32 {
    // Transform to anchor-relative coords — rotation pivots around (ax, ay)
    let sx = px - u.ax;
    let sy = py - u.ay;
    let rx = sx * cos_a + sy * sin_a;
    let ry = -sx * sin_a + sy * cos_a;

    let cx = round(rx / u.cell_spacing) * u.cell_spacing;
    let cy = round(ry / u.cell_spacing) * u.cell_spacing;

    let dx = rx - cx;
    let dy = ry - cy;

    let dot_radius = (1.0 - brightness) * u.half_cell;

    var dist: f32;
    switch u.dot_shape {
        case 0u: { dist = sqrt(dx * dx + dy * dy); }
        case 1u: { dist = max(abs(dx), abs(dy)); }
        case 2u: { dist = abs(dx) + abs(dy); }
        default: { dist = sqrt(dx * dx + dy * dy); }
    }

    let soft = u.smoothness * u.cell_spacing;
    let inner = dot_radius - soft;
    let outer = dot_radius + soft;
    if dist <= inner {
        return 1.0;
    } else if dist >= outer {
        return 0.0;
    } else {
        let t = (dist - inner) / (outer - inner);
        return 1.0 - t * t * (3.0 - 2.0 * t);
    }
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if x >= u.width || y >= u.height { return; }

    let idx = y * u.width + x;
    let pixel = unpack_rgba8(src[idx]);

    var hr: f32;
    var hg: f32;
    var hb: f32;

    if u.channel_mode == 0u {
        // Luminance mode
        let lum = luminance(pixel.r, pixel.g, pixel.b);
        let bright = contrast(lum);
        let b = select(bright, 1.0 - bright, u.invert != 0u);
        let coverage = dot_coverage(f32(x), f32(y), b, u.cos0, u.sin0);
        let dot = 1.0 - coverage;
        hr = dot; hg = dot; hb = dot;
    } else {
        // RGB mode — per-channel with angle offsets relative to user angle
        let br = contrast(pixel.r);
        let bg = contrast(pixel.g);
        let bb = contrast(pixel.b);
        let ch_r = select(br, 1.0 - br, u.invert != 0u);
        let ch_g = select(bg, 1.0 - bg, u.invert != 0u);
        let ch_b = select(bb, 1.0 - bb, u.invert != 0u);

        let cr = dot_coverage(f32(x), f32(y), ch_r, u.cos1, u.sin1);
        let cg = dot_coverage(f32(x), f32(y), ch_g, u.cos2, u.sin2);
        let cb = dot_coverage(f32(x), f32(y), ch_b, u.cos3, u.sin3);
        hr = 1.0 - cr; hg = 1.0 - cg; hb = 1.0 - cb;
    }

    // Blend with original
    if u.blend > 0.001 {
        hr = hr + (pixel.r - hr) * u.blend;
        hg = hg + (pixel.g - hg) * u.blend;
        hb = hb + (pixel.b - hb) * u.blend;
    }

    dst[idx] = pack_rgba8(vec4<f32>(hr, hg, hb, pixel.a));
}
