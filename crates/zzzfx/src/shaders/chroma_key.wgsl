// Chroma Key — GPU compute shader
// Entry points: compute_alpha, blur, composite

// ---- Uniforms ----

struct AlphaUniforms {
    width: u32,
    height: u32,
    key_cb: f32,
    key_cr: f32,
    threshold_sq: f32,
    soft_end_sq: f32,
    range_sq: f32,
    edge_softness: f32,
}

struct BlurUniforms {
    width: u32,
    height: u32,
    radius: u32,
    horizontal: u32,
}

struct CompositeUniforms {
    width: u32,
    height: u32,
    spill_suppression: f32,
    show_matte: u32,
    invert: u32,
}

// ---- Color unpack helpers ----

fn unpack1(p: u32) -> f32 { return f32(p & 0xFFu) * 0.003921568627451; }
fn unpack2(p: u32) -> f32 { return f32((p >> 8u) & 0xFFu) * 0.003921568627451; }
fn unpack3(p: u32) -> f32 { return f32((p >> 16u) & 0xFFu) * 0.003921568627451; }
fn unpack4(p: u32) -> f32 { return f32(p >> 24u) * 0.003921568627451; }

fn pack(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let r8 = u32(clamp(r, 0.0, 1.0) * 255.0 + 0.5);
    let g8 = u32(clamp(g, 0.0, 1.0) * 255.0 + 0.5);
    let b8 = u32(clamp(b, 0.0, 1.0) * 255.0 + 0.5);
    let a8 = u32(clamp(a, 0.0, 1.0) * 255.0 + 0.5);
    return r8 | (g8 << 8u) | (b8 << 16u) | (a8 << 24u);
}

// ---- BT.601 YCbCr ----

fn rgb_to_cb(r: f32, g: f32, b: f32) -> f32 {
    return -0.168736 * r - 0.331264 * g + 0.5 * b + 0.5;
}

fn rgb_to_cr(r: f32, g: f32, b: f32) -> f32 {
    return 0.5 * r - 0.418688 * g - 0.081312 * b + 0.5;
}

// ---- Smoothstep ----

fn smoothstep_edge(t: f32) -> f32 {
    return t * t * (3.0 - 2.0 * t);
}

// =================================================================
// Entry point 1: compute_alpha
// =================================================================

@group(0) @binding(0) var<storage, read> src_alpha: array<u32>;
@group(0) @binding(1) var<uniform> a_params: AlphaUniforms;
@group(0) @binding(2) var<storage, read_write> alpha_out: array<f32>;

@compute @workgroup_size(16, 16)
fn compute_alpha(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = a_params.width;
    let h = a_params.height;
    if gid.x >= w || gid.y >= h { return; }
    let i = gid.y * w + gid.x;

    let sp = src_alpha[i];
    let r = unpack1(sp);
    let g = unpack2(sp);
    let b = unpack3(sp);

    let cb = rgb_to_cb(r, g, b);
    let cr = rgb_to_cr(r, g, b);
    let dc = cb - a_params.key_cb;
    let dr = cr - a_params.key_cr;
    let dist_sq = (dc * dc + dr * dr) * 0.5;

    var ka: f32;
    if dist_sq <= a_params.threshold_sq {
        ka = 0.0;
    } else if a_params.edge_softness <= 0.0 || dist_sq >= a_params.soft_end_sq {
        ka = 1.0;
    } else {
        let t = (dist_sq - a_params.threshold_sq) / a_params.range_sq;
        ka = smoothstep_edge(t);
    }
    alpha_out[i] = ka;
}

// =================================================================
// Entry point 2: blur (separable box blur, H or V controlled by uniform)
// =================================================================

@group(0) @binding(0) var<storage, read> blur_src: array<f32>;
@group(0) @binding(1) var<uniform> b_params: BlurUniforms;
@group(0) @binding(2) var<storage, read_write> blur_dst: array<f32>;

@compute @workgroup_size(16, 16)
fn blur(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = b_params.width;
    let h = b_params.height;
    if gid.x >= w || gid.y >= h { return; }

    let r = b_params.radius;
    if b_params.horizontal != 0u {
        // Horizontal blur: sum across row
        let y = gid.y;
        let x0 = select(gid.x - r, 0u, gid.x < r);
        let x1 = min(gid.x + r + 1u, w);
        let row_base = y * w;
        var sum: f32 = 0.0;
        for (var sx = x0; sx < x1; sx++) {
            sum += blur_src[row_base + sx];
        }
        let actual = f32(x1 - x0);
        blur_dst[row_base + gid.x] = sum / actual;
    } else {
        // Vertical blur: sum across column
        let x = gid.x;
        let y0 = select(gid.y - r, 0u, gid.y < r);
        let y1 = min(gid.y + r + 1u, h);
        var sum: f32 = 0.0;
        for (var sy = y0; sy < y1; sy++) {
            sum += blur_src[sy * w + x];
        }
        let actual = f32(y1 - y0);
        blur_dst[gid.y * w + gid.x] = sum / actual;
    }
}

// =================================================================
// Entry point 3: composite
// =================================================================

@group(0) @binding(0) var<storage, read> comp_src: array<u32>;
@group(0) @binding(1) var<storage, read> comp_alpha: array<f32>;
@group(0) @binding(2) var<uniform> c_params: CompositeUniforms;
@group(0) @binding(3) var<storage, read_write> dst: array<u32>;

@compute @workgroup_size(16, 16)
fn composite(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = c_params.width;
    let h = c_params.height;
    if gid.x >= w || gid.y >= h { return; }
    let i = gid.y * w + gid.x;

    let sp = comp_src[i];
    let r = unpack1(sp);
    let g = unpack2(sp);
    let b = unpack3(sp);
    let a = unpack4(sp);

    var key_alpha = comp_alpha[i];
    if c_params.invert != 0u {
        key_alpha = 1.0 - key_alpha;
    }

    if c_params.show_matte != 0u {
        let v = u32(key_alpha * 255.0 + 0.5);
        dst[i] = v | (v << 8u) | (v << 16u) | (255u << 24u);
        return;
    }

    // Spill suppression: desaturate toward luminance
    var out_r = r;
    var out_g = g;
    var out_b = b;
    if c_params.spill_suppression > 0.0 && key_alpha < 1.0 {
        let spill = c_params.spill_suppression * sqrt(1.0 - key_alpha);
        let lum = luminance(r, g, b);
        out_r = clamp(r + (lum - r) * spill, 0.0, 1.0);
        out_g = clamp(g + (lum - g) * spill, 0.0, 1.0);
        out_b = clamp(b + (lum - b) * spill, 0.0, 1.0);
    }

    // Premultiply by key alpha
    let out_a = a * key_alpha;
    dst[i] = pack(out_r * out_a, out_g * out_a, out_b * out_a, out_a);
}
