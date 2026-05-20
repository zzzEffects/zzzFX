// zzzRepeater GPU compositing: per-pixel single-layer blend onto accumulated dst.
// Called once per layer from Rust (multi-pass), matching the CPU path exactly.
// Layer ordering (Above/Below) is handled by the dispatch order on the Rust side.

struct Uniforms {
    width: u32,
    height: u32,
    blend_mode: u32,     // 0-21, same enum as compose.wgsl
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

fn hash(pixel_idx: u32) -> f32 {
    var h = pixel_idx * 0x45d9f3bu;
    h = (h ^ (h >> 16u)) * 0x85ebca6bu;
    h = h ^ (h >> 13u);
    return f32(h) / 4294967295.0;
}

fn luminance(r: f32, g: f32, b: f32) -> f32 {
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

fn blend_channel(mode: u32, base: f32, blend: f32, stroke_alpha: f32, rng: f32) -> f32 {
    switch mode {
        case 0u: { return blend; }                             // Normal
        case 1u: {                                             // Dissolve
            if rng < stroke_alpha { return blend; }
            return base;
        }
        case 2u: { return min(base, blend); }                  // Darken
        case 3u: { return base * blend; }                      // Multiply
        case 4u: {                                              // ColorBurn
            if blend <= 0.0 { return 0.0; }
            return 1.0 - min((1.0 - base) / blend, 1.0);
        }
        case 5u: { return max(base + blend - 1.0, 0.0); }      // LinearBurn
        case 6u: { return min(base + blend, 1.0); }            // Add
        case 7u: { return 1.0 - (1.0 - base) * (1.0 - blend); } // Screen
        case 8u: {                                              // ColorDodge
            if blend >= 1.0 { return 1.0; }
            return min(base / (1.0 - blend), 1.0);
        }
        case 9u: { return min(base + blend, 1.0); }            // LinearDodge
        case 10u: {                                             // Overlay
            if base < 0.5 {
                return 2.0 * base * blend;
            } else {
                return 1.0 - 2.0 * (1.0 - base) * (1.0 - blend);
            }
        }
        case 11u: {                                             // SoftLight
            if blend < 0.5 {
                return base - (1.0 - 2.0 * blend) * base * (1.0 - base);
            } else {
                var d: f32;
                if base < 0.25 {
                    d = ((16.0 * base - 12.0) * base + 4.0) * base;
                } else {
                    d = sqrt(base);
                }
                return base + (2.0 * blend - 1.0) * (d - base);
            }
        }
        case 12u: { return clamp(base + 2.0 * blend - 1.0, 0.0, 1.0); } // LinearLight
        case 13u: {                                             // HardMix
            if base + blend < 1.0 { return 0.0; } else { return 1.0; }
        }
        case 14u: { return abs(base - blend); }                 // Difference
        case 15u: { return base + blend - 2.0 * base * blend; } // Exclusion
        case 16u: { return max(base - blend, 0.0); }            // Subtract
        case 17u: {                                              // Divide
            if blend <= 0.0 { return 1.0; }
            return min(base / blend, 1.0);
        }
        case 18u, 19u, 20u, 21u: { return blend; }
        default: { return blend; }
    }
}

fn is_stencil_or_outline(mode: u32) -> bool {
    return mode == 18u || mode == 19u || mode == 20u || mode == 21u;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= uniforms.width || gid.y >= uniforms.height {
        return;
    }

    let idx = gid.y * uniforms.width + gid.x;

    // Hash-based RNG (matching CPU three-rng pattern)
    let h = hash(idx);
    let rng1 = h;
    let rng2 = hash(idx ^ 0xDEADBEEFu);
    let rng3 = hash(idx ^ 0xCAFEBABEu);

    let stencil = is_stencil_or_outline(uniforms.blend_mode);

    // Read accumulated dst
    let dst_pixel = dst[idx];
    var acc_r = f32(dst_pixel & 0xFFu) / 255.0;
    var acc_g = f32((dst_pixel >> 8u) & 0xFFu) / 255.0;
    var acc_b = f32((dst_pixel >> 16u) & 0xFFu) / 255.0;
    var acc_a = f32(dst_pixel >> 24u) / 255.0;

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
    let pixel = src[src_idx];
    let sr = f32(pixel & 0xFFu) / 255.0;
    let sg = f32((pixel >> 8u) & 0xFFu) / 255.0;
    let sb = f32((pixel >> 16u) & 0xFFu) / 255.0;
    let sa = f32(pixel >> 24u) / 255.0;

    if sa <= 0.0 {
        return; // fully transparent layer pixel — accumulate unchanged
    }

    if acc_a <= 0.0 {
        // First visible content at this pixel — take the layer directly
        let r8 = u32(clamp(sr, 0.0, 1.0) * 255.0 + 0.5);
        let g8 = u32(clamp(sg, 0.0, 1.0) * 255.0 + 0.5);
        let b8 = u32(clamp(sb, 0.0, 1.0) * 255.0 + 0.5);
        let a8 = u32(clamp(sa, 0.0, 1.0) * 255.0 + 0.5);
        dst[idx] = r8 | (g8 << 8u) | (b8 << 16u) | (a8 << 24u);
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
            // Outline: replace color, multiply alpha by stencil
            acc_r = sr;
            acc_g = sg;
            acc_b = sb;
            acc_a = stencil_a * acc_a;
        } else {
            // Stencil: blend color, multiply alpha by stencil
            acc_r = blend_channel(uniforms.blend_mode, acc_r, sr, sa, rng1);
            acc_g = blend_channel(uniforms.blend_mode, acc_g, sg, sa, rng2);
            acc_b = blend_channel(uniforms.blend_mode, acc_b, sb, sa, rng3);
            acc_a = stencil_a * acc_a;
        }
    } else {
        // Normal over-composite: blend then combine with accumulated
        let blended_r = blend_channel(uniforms.blend_mode, acc_r, sr, sa, rng1);
        let blended_g = blend_channel(uniforms.blend_mode, acc_g, sg, sa, rng2);
        let blended_b = blend_channel(uniforms.blend_mode, acc_b, sb, sa, rng3);

        let inv = 1.0 - sa;
        acc_r = clamp(blended_r * sa + acc_r * inv, 0.0, 1.0);
        acc_g = clamp(blended_g * sa + acc_g * inv, 0.0, 1.0);
        acc_b = clamp(blended_b * sa + acc_b * inv, 0.0, 1.0);
        acc_a = clamp(sa + acc_a * inv, 0.0, 1.0);
    }

    // Pack output
    let r8 = u32(clamp(acc_r, 0.0, 1.0) * 255.0 + 0.5);
    let g8 = u32(clamp(acc_g, 0.0, 1.0) * 255.0 + 0.5);
    let b8 = u32(clamp(acc_b, 0.0, 1.0) * 255.0 + 0.5);
    let a8 = u32(clamp(acc_a, 0.0, 1.0) * 255.0 + 0.5);
    dst[idx] = r8 | (g8 << 8u) | (b8 << 16u) | (a8 << 24u);
}
