// MultiTone effect — single-pass compute shader for None / Ordered dithering.
// Floyd-Steinberg falls back to CPU (not GPU-friendly due to serial error diffusion).
// Shared functions (unpack_rgba8, pack_rgba8, luminance) are prepended from shared.wgsl.

// 4x4 Bayer matrix
const BAYER: array<f32, 16> = array<f32, 16>(
     0.0/16.0,  8.0/16.0,  2.0/16.0, 10.0/16.0,
    12.0/16.0,  4.0/16.0, 14.0/16.0,  6.0/16.0,
     3.0/16.0, 11.0/16.0,  1.0/16.0,  9.0/16.0,
    15.0/16.0,  7.0/16.0, 13.0/16.0,  5.0/16.0,
);

struct Uniforms {
    width: u32,
    height: u32,
    levels_i: u32,         // floor'd tone_levels (2..32)
    levels_f: f32,         // (levels_i - 1) as f32
    mode: u32,             // 0=PerChannel, 1=Luminance
    dithering: u32,        // 0=None, 1=Ordered (2=FS not supported, falls back)
    dither_amount: f32,
    edge_softness: f32,
    preserve_lum: u32,
    // Color mapping
    color_map_enabled: u32,
    shadow_r: f32, shadow_g: f32, shadow_b: f32,
    midtone_r: f32, midtone_g: f32, midtone_b: f32,
    highlight_r: f32, highlight_g: f32, highlight_b: f32,
    midtone_pos: f32,
    cm_blend: f32,
}

@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<uniform> u: Uniforms;
@group(0) @binding(2) var<storage, read_write> dst: array<u32>;

fn quantize_channel(v: f32) -> f32 {
    if u.edge_softness <= 0.001 {
        return floor(v * u.levels_f + 0.5) / u.levels_f;
    }
    // Soft quantization
    let scaled = v * u.levels_f;
    let lower = floor(scaled);
    let frac = scaled - lower;
    let sw = u.edge_softness * 0.5;
    var t: f32;
    if frac < sw {
        t = 0.0;
    } else if frac > 1.0 - sw {
        t = 1.0;
    } else {
        let s = (frac - sw) / (1.0 - 2.0 * sw);
        t = s * s * (3.0 - 2.0 * s);
    }
    return (lower + t) / u.levels_f;
}

fn apply_color_map(qr: f32, qg: f32, qb: f32) -> vec3<f32> {
    if u.color_map_enabled == 0u || u.cm_blend >= 0.999 {
        return vec3<f32>(qr, qg, qb);
    }
    let lum = luminance(qr, qg, qb);
    let mp = clamp(u.midtone_pos, 0.001, 0.999);

    var cr: f32;
    var cg: f32;
    var cb: f32;
    if lum <= mp {
        let t = lum / mp;
        cr = u.shadow_r + (u.midtone_r - u.shadow_r) * t;
        cg = u.shadow_g + (u.midtone_g - u.shadow_g) * t;
        cb = u.shadow_b + (u.midtone_b - u.shadow_b) * t;
    } else {
        let t = (lum - mp) / (1.0 - mp);
        cr = u.midtone_r + (u.highlight_r - u.midtone_r) * t;
        cg = u.midtone_g + (u.highlight_g - u.midtone_g) * t;
        cb = u.midtone_b + (u.highlight_b - u.midtone_b) * t;
    }

    if u.cm_blend <= 0.001 {
        return vec3<f32>(cr, cg, cb);
    }
    return vec3<f32>(
        cr + (qr - cr) * u.cm_blend,
        cg + (qg - cg) * u.cm_blend,
        cb + (qb - cb) * u.cm_blend,
    );
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if x >= u.width || y >= u.height { return; }

    let idx = y * u.width + x;
    let pixel = unpack_rgba8(src[idx]);

    // Ordered dithering noise
    if u.dithering == 1u {
        let bayer = BAYER[(y % 4u) * 4u + (x % 4u)];
        let noise = (bayer - 0.5) * u.dither_amount;
        // Apply noise before quantization (WGSL doesn't allow mutating local vars easily — we'll inline)
    }

    var r = pixel.r;
    var g = pixel.g;
    var b = pixel.b;

    if u.dithering == 1u {
        let bayer = BAYER[(y % 4u) * 4u + (x % 4u)];
        let noise = (bayer - 0.5) * u.dither_amount;
        r = clamp(r + noise, 0.0, 1.0);
        g = clamp(g + noise, 0.0, 1.0);
        b = clamp(b + noise, 0.0, 1.0);
    }

    var qr: f32;
    var qg: f32;
    var qb: f32;

    if u.mode == 0u {
        // PerChannel
        qr = quantize_channel(r);
        qg = quantize_channel(g);
        qb = quantize_channel(b);
        if u.preserve_lum != 0u {
            let orig_lum = luminance(r, g, b);
            let q_lum = luminance(qr, qg, qb);
            if q_lum > 0.001 {
                let ratio = orig_lum / q_lum;
                qr = clamp(qr * ratio, 0.0, 1.0);
                qg = clamp(qg * ratio, 0.0, 1.0);
                qb = clamp(qb * ratio, 0.0, 1.0);
            }
        }
    } else {
        // Luminance
        let lum = luminance(r, g, b);
        let q_lum = quantize_channel(lum);
        if lum > 0.001 {
            let ratio = q_lum / lum;
            qr = clamp(r * ratio, 0.0, 1.0);
            qg = clamp(g * ratio, 0.0, 1.0);
            qb = clamp(b * ratio, 0.0, 1.0);
        } else {
            qr = q_lum; qg = q_lum; qb = q_lum;
        }
    }

    let color = apply_color_map(qr, qg, qb);
    dst[idx] = pack_rgba8(vec4<f32>(color.x, color.y, color.z, pixel.a));
}
