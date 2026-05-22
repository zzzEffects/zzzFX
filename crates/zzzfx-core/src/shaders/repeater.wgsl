// zzzRepeater GPU compositing: per-pixel single-layer blend onto accumulated dst.
// Uses shared.wgsl for hash, blend_channel, pack_rgba8, etc.

struct Uniforms {
    width: u32,
    height: u32,
    blend_mode: u32,     // 0-21
    center_x: f32,
    center_y: f32,
    offset_x: f32,
    offset_y: f32,
    cos_a: f32,
    sin_a: f32,
}

@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<uniform> uniforms: Uniforms;
@group(0) @binding(2) var<storage, read_write> dst: array<u32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= uniforms.width || gid.y >= uniforms.height {
        return;
    }

    let idx = gid.y * uniforms.width + gid.x;

    let rng1 = hash(idx);
    let rng2 = hash(idx ^ 0xDEADBEEFu);
    let rng3 = hash(idx ^ 0xCAFEBABEu);

    let stencil = is_stencil_or_outline(uniforms.blend_mode);

    // Read accumulated dst
    let acc = unpack_rgba8(dst[idx]);
    var acc_r = acc.r;
    var acc_g = acc.g;
    var acc_b = acc.b;
    var acc_a = acc.a;

    // Inverse transform: output pixel → source pixel
    let cx = f32(gid.x) - uniforms.center_x;
    let cy = f32(gid.y) - uniforms.center_y;

    let rx = cx * uniforms.cos_a - cy * uniforms.sin_a;
    let ry = cx * uniforms.sin_a + cy * uniforms.cos_a;

    let sx_f = rx - uniforms.offset_x + uniforms.center_x;
    let sy_f = ry - uniforms.offset_y + uniforms.center_y;

    // Nearest-neighbor clamp
    let sx = clamp(u32(round(sx_f)), 0u, uniforms.width - 1u);
    let sy = clamp(u32(round(sy_f)), 0u, uniforms.height - 1u);

    // Sample layer pixel
    let src_idx = sy * uniforms.width + sx;
    let src_color = unpack_rgba8(src[src_idx]);
    let sr = src_color.r;
    let sg = src_color.g;
    let sb = src_color.b;
    let sa = src_color.a;

    if sa <= 0.0 {
        return;
    }

    if acc_a <= 0.0 {
        // First visible content — take the layer directly
        dst[idx] = pack_rgba8(vec4<f32>(sr, sg, sb, sa));
        return;
    }

    if stencil {
        var stencil_a: f32;
        switch uniforms.blend_mode {
            case 18u: { stencil_a = sa; }                          // StencilAlpha
            case 19u: { stencil_a = sa * luminance(sr, sg, sb); }  // StencilLuma
            case 20u: { stencil_a = sa; }                          // OutlineAlpha
            case 21u: { stencil_a = sa * luminance(sr, sg, sb); }  // OutlineLuma
            default:  { stencil_a = sa; }
        }

        if uniforms.blend_mode == 20u || uniforms.blend_mode == 21u {
            acc_r = sr;
            acc_g = sg;
            acc_b = sb;
            acc_a = stencil_a * acc_a;
        } else {
            acc_r = blend_channel(uniforms.blend_mode, acc_r, sr, sa, rng1);
            acc_g = blend_channel(uniforms.blend_mode, acc_g, sg, sa, rng2);
            acc_b = blend_channel(uniforms.blend_mode, acc_b, sb, sa, rng3);
            acc_a = stencil_a * acc_a;
        }
    } else {
        let blended_r = blend_channel(uniforms.blend_mode, acc_r, sr, sa, rng1);
        let blended_g = blend_channel(uniforms.blend_mode, acc_g, sg, sa, rng2);
        let blended_b = blend_channel(uniforms.blend_mode, acc_b, sb, sa, rng3);

        let inv = 1.0 - sa;
        acc_r = clamp(blended_r * sa + acc_r * inv, 0.0, 1.0);
        acc_g = clamp(blended_g * sa + acc_g * inv, 0.0, 1.0);
        acc_b = clamp(blended_b * sa + acc_b * inv, 0.0, 1.0);
        acc_a = clamp(sa + acc_a * inv, 0.0, 1.0);
    }

    // F1: Use built-in pack4xU8
    dst[idx] = pack_rgba8(vec4<f32>(acc_r, acc_g, acc_b, acc_a));
}
