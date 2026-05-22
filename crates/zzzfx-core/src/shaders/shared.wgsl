// Shared WGSL functions used across multiple shader modules.
// Inlined at compile time via include_str! concatenation.

// ---- RNG ----

fn hash(pixel_idx: u32) -> f32 {
    var h = pixel_idx * 0x45d9f3bu;
    h = (h ^ (h >> 16u)) * 0x85ebca6bu;
    h = h ^ (h >> 13u);
    return f32(h) / 4294967295.0;
}

// ---- Color ----

fn luminance(r: f32, g: f32, b: f32) -> f32 {
    return fma(0.2126, r, fma(0.7152, g, 0.0722 * b));
}

fn unpack_rgba8(packed: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(packed & 0xFFu) / 255.0,
        f32((packed >> 8u) & 0xFFu) / 255.0,
        f32((packed >> 16u) & 0xFFu) / 255.0,
        f32(packed >> 24u) / 255.0,
    );
}

fn pack_rgba8(color: vec4<f32>) -> u32 {
    let r = u32(clamp(color.r, 0.0, 1.0) * 255.0 + 0.5);
    let g = u32(clamp(color.g, 0.0, 1.0) * 255.0 + 0.5);
    let b = u32(clamp(color.b, 0.0, 1.0) * 255.0 + 0.5);
    let a = u32(clamp(color.a, 0.0, 1.0) * 255.0 + 0.5);
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

// ---- Blend modes ----

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
    return mode >= 18u && mode <= 21u;
}

// ---- JFA / SDF helpers ----

fn decode_seed(packed: u32) -> vec2<u32> {
    return vec2(packed & 0xFFFFu, (packed >> 16u) & 0xFFFFu);
}

fn is_valid_seed(packed: u32) -> bool {
    return packed != 0xFFFFFFFFu;
}

fn seed_distance(px: u32, py: u32, seed: vec2<u32>, use_sharp: bool) -> f32 {
    let dx = f32(max(px, seed.x) - min(px, seed.x));
    let dy = f32(max(py, seed.y) - min(py, seed.y));
    if use_sharp {
        return dx + dy;
    }
    return sqrt(dx * dx + dy * dy);
}

// ---- Feathering ----

fn gaussian_edge(sigma: f32, center: f32, d: f32) -> f32 {
    if sigma <= 0.0 {
        if d <= center { return 1.0; } else { return 0.0; }
    }
    let x = 1.701 * (d - center) / sigma;
    if x > 10.0 {
        return 0.0;
    }
    if x < -10.0 {
        return 1.0;
    }
    return 1.0 / (1.0 + exp(x));
}

// ---- ASS subtitle blending ----

fn overlay_channel(s: f32, d: f32) -> f32 {
    if d < 0.5 { return 2.0 * d * s; }
    return 1.0 - 2.0 * (1.0 - d) * (1.0 - s);
}

fn blend_pixel(mode: u32, src_px: vec4<f32>, dst_in: vec4<f32>) -> vec4<f32> {
    if mode == 0u {
        // Normal (source-over)
        let out_a = src_px.a + dst_in.a * (1.0 - src_px.a);
        if out_a < 0.001 {
            return vec4<f32>(0.0);
        }
        let inv_out_a = 1.0 / out_a;
        return vec4<f32>(
            fma(dst_in.r, 1.0 - src_px.a, src_px.r) * inv_out_a,
            fma(dst_in.g, 1.0 - src_px.a, src_px.g) * inv_out_a,
            fma(dst_in.b, 1.0 - src_px.a, src_px.b) * inv_out_a,
            out_a,
        );
    }
    if mode == 1u {
        // Add
        return vec4<f32>(
            min(src_px.r + dst_in.r, 1.0),
            min(src_px.g + dst_in.g, 1.0),
            min(src_px.b + dst_in.b, 1.0),
            min(src_px.a + dst_in.a, 1.0),
        );
    }
    if mode == 2u {
        // Screen
        return vec4<f32>(
            1.0 - (1.0 - src_px.r) * (1.0 - dst_in.r),
            1.0 - (1.0 - src_px.g) * (1.0 - dst_in.g),
            1.0 - (1.0 - src_px.b) * (1.0 - dst_in.b),
            1.0 - (1.0 - src_px.a) * (1.0 - dst_in.a),
        );
    }
    if mode == 3u {
        // Multiply
        return src_px * dst_in;
    }
    if mode == 4u {
        // Overlay
        return vec4<f32>(
            overlay_channel(src_px.r, dst_in.r),
            overlay_channel(src_px.g, dst_in.g),
            overlay_channel(src_px.b, dst_in.b),
            overlay_channel(src_px.a, dst_in.a),
        );
    }
    // Default: Normal
    let out_a = src_px.a + dst_in.a * (1.0 - src_px.a);
    if out_a < 0.001 { return vec4<f32>(0.0); }
    let inv_out_a = 1.0 / out_a;
    return vec4<f32>(
        fma(dst_in.r, 1.0 - src_px.a, src_px.r) * inv_out_a,
        fma(dst_in.g, 1.0 - src_px.a, src_px.g) * inv_out_a,
        fma(dst_in.b, 1.0 - src_px.a, src_px.b) * inv_out_a,
        out_a,
    );
}
