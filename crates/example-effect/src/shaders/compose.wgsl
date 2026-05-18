// Stage 4+5: Stroke composition with all blend modes.
// Decodes JFA seeds, computes stroke alpha, fills color, blends with source.

struct Uniforms {
    width: u32,
    height: u32,
    max_dim: f32,
    stroke_width_px: f32,
    feather_px: f32,
    stroke_a: f32,
    stroke_r: f32,
    stroke_g: f32,
    stroke_b: f32,
    alpha_threshold: f32,
    source_opacity: f32,
    stroke_position: u32,   // 0=Outer, 1=Inner, 2=Center
    fill_mode: u32,         // 0=SolidColor, 1=DistanceGradient, 2=Gradient, 3=SourceColorExtension
    blend_mode: u32,        // 0-21
    use_sharp_corners: u32,
    // Gradient params
    grad_start_x: f32,
    grad_start_y: f32,
    grad_end_x: f32,
    grad_end_y: f32,
    grad_start_r: f32,
    grad_start_g: f32,
    grad_start_b: f32,
    _pad0: u32,             // was grad_start_a in Rust, unused in compose
    grad_end_r: f32,
    grad_end_g: f32,
    grad_end_b: f32,
    _pad1: u32,             // was grad_end_a in Rust, unused in compose
}

@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<uniform> uniforms: Uniforms;
@group(0) @binding(2) var<storage, read> seeds: array<u32>;
@group(0) @binding(3) var<storage, read_write> dst: array<u32>;

// ---- helpers ----

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

fn luminance(r: f32, g: f32, b: f32) -> f32 {
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

fn hash(pixel_idx: u32) -> f32 {
    var h = pixel_idx * 0x45d9f3bu;
    h = (h ^ (h >> 16u)) * 0x85ebca6bu;
    h = h ^ (h >> 13u);
    return f32(h) / 4294967295.0;
}

fn is_stencil_or_outline(mode: u32) -> bool {
    // StencilAlpha=18, StencilLuma=19, OutlineAlpha=20, OutlineLuma=21
    return mode == 18u || mode == 19u || mode == 20u || mode == 21u;
}

fn blend_channel(mode: u32, base: f32, blend: f32, stroke_alpha: f32, rng: f32) -> f32 {
    switch mode {
        case 0u: { return blend; }                             // Normal
        case 1u: {                                             // Dissolve
            if rng < stroke_alpha { return blend; }
            return base;
        }
        case 2u: { return min(base, blend); }                  // Darken
        case 3u: { return base * blend; }                       // Multiply
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
        case 9u: { return min(base + blend, 1.0); }            // LinearDodge (= Add)
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
        // StencilAlpha, StencilLuma, OutlineAlpha, OutlineLuma — pass through blend value
        case 18u, 19u, 20u, 21u: { return blend; }
        default: { return blend; }
    }
}

fn is_inside(alpha: f32) -> bool {
    return alpha >= uniforms.alpha_threshold;
}

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

// ---- main ----

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= uniforms.width || gid.y >= uniforms.height {
        return;
    }

    let idx = gid.y * uniforms.width + gid.x;
    let pixel = src[idx];
    let src_r = f32(pixel & 0xFFu) / 255.0;
    let src_g = f32((pixel >> 8u) & 0xFFu) / 255.0;
    let src_b = f32((pixel >> 16u) & 0xFFu) / 255.0;
    let src_a = f32(pixel >> 24u) / 255.0;
    let inside = is_inside(src_a);

    // Compute distance from JFA seed
    var d: f32 = 1e10;
    let seed_packed = seeds[idx];
    if is_valid_seed(seed_packed) {
        d = seed_distance(gid.x, gid.y, decode_seed(seed_packed), uniforms.use_sharp_corners != 0u);
    }

    // Stroke alpha from distance
    var stroke_alpha_local: f32;
    let sigma = uniforms.feather_px / 3.0;
    switch uniforms.stroke_position {
        case 0u: { // Outer
            if inside {
                stroke_alpha_local = 0.0;
            } else {
                stroke_alpha_local = gaussian_edge(sigma, uniforms.stroke_width_px, d);
            }
        }
        case 1u: { // Inner
            if !inside {
                stroke_alpha_local = 0.0;
            } else {
                stroke_alpha_local = gaussian_edge(sigma, uniforms.stroke_width_px, d);
            }
        }
        case 2u: { // Center
            let half_w = uniforms.stroke_width_px * 0.5;
            stroke_alpha_local = gaussian_edge(sigma, half_w, d);
        }
        default: {
            stroke_alpha_local = 0.0;
        }
    }

    let sa = stroke_alpha_local * uniforms.stroke_a;

    var out_r = src_r;
    var out_g = src_g;
    var out_b = src_b;
    var out_a = src_a;

    if sa > 0.0 {
        // Determine stroke color from fill mode
        var sr: f32;
        var sg: f32;
        var sb: f32;

        switch uniforms.fill_mode {
            case 0u: { // SolidColor
                sr = uniforms.stroke_r;
                sg = uniforms.stroke_g;
                sb = uniforms.stroke_b;
            }
            case 1u: { // DistanceGradient
                let gx = uniforms.grad_start_x * f32(uniforms.width);
                let gy = uniforms.grad_start_y * f32(uniforms.height);
                let dx = f32(gid.x) - gx;
                let dy = f32(gid.y) - gy;
                let dist = sqrt(dx * dx + dy * dy);
                let max_dist = sqrt(f32(uniforms.width) * f32(uniforms.width) + f32(uniforms.height) * f32(uniforms.height));
                let t = clamp(dist / max_dist, 0.0, 1.0);
                sr = uniforms.grad_start_r + t * (uniforms.grad_end_r - uniforms.grad_start_r);
                sg = uniforms.grad_start_g + t * (uniforms.grad_end_g - uniforms.grad_start_g);
                sb = uniforms.grad_start_b + t * (uniforms.grad_end_b - uniforms.grad_start_b);
            }
            case 2u: { // Gradient (linear)
                let dx = uniforms.grad_end_x - uniforms.grad_start_x;
                let dy = uniforms.grad_end_y - uniforms.grad_start_y;
                let len_sq = dx * dx + dy * dy;
                let gx = uniforms.grad_start_x * f32(uniforms.width);
                let gy = uniforms.grad_start_y * f32(uniforms.height);
                let px = f32(gid.x) - gx;
                let py = f32(gid.y) - gy;
                var t: f32;
                if len_sq > 0.0 {
                    t = clamp((px * dx + py * dy) / len_sq, 0.0, 1.0);
                } else {
                    t = 0.0;
                }
                sr = uniforms.grad_start_r + t * (uniforms.grad_end_r - uniforms.grad_start_r);
                sg = uniforms.grad_start_g + t * (uniforms.grad_end_g - uniforms.grad_start_g);
                sb = uniforms.grad_start_b + t * (uniforms.grad_end_b - uniforms.grad_start_b);
            }
            case 3u: { // SourceColorExtension
                if is_valid_seed(seed_packed) {
                    let seed_coord = decode_seed(seed_packed);
                    let src_idx = (seed_coord.y * uniforms.width + seed_coord.x);
                    let edge_pixel = src[src_idx];
                    sr = f32(edge_pixel & 0xFFu) / 255.0;
                    sg = f32((edge_pixel >> 8u) & 0xFFu) / 255.0;
                    sb = f32((edge_pixel >> 16u) & 0xFFu) / 255.0;
                } else {
                    sr = uniforms.stroke_r;
                    sg = uniforms.stroke_g;
                    sb = uniforms.stroke_b;
                }
            }
            default: {
                sr = uniforms.stroke_r;
                sg = uniforms.stroke_g;
                sb = uniforms.stroke_b;
            }
        }

        // Blend with source
        let rng0 = hash(idx);
        let rng1 = hash(idx ^ 0xDEADBEEFu);
        let rng2 = hash(idx ^ 0xCAFEBABEu);

        let blended_r = blend_channel(uniforms.blend_mode, src_r, sr, sa, rng0);
        let blended_g = blend_channel(uniforms.blend_mode, src_g, sg, sa, rng1);
        let blended_b = blend_channel(uniforms.blend_mode, src_b, sb, sa, rng2);

        if is_stencil_or_outline(uniforms.blend_mode) {
            // Enums: StencilAlpha=18, StencilLuma=19, OutlineAlpha=20, OutlineLuma=21
            var stencil_a: f32;
            switch uniforms.blend_mode {
                case 18u: { stencil_a = sa; }                          // StencilAlpha
                case 19u: { stencil_a = sa * luminance(sr, sg, sb); }  // StencilLuma
                case 20u: { stencil_a = sa; }                          // OutlineAlpha
                case 21u: { stencil_a = sa * luminance(sr, sg, sb); }  // OutlineLuma
                default: { stencil_a = sa; }
            }

            if uniforms.blend_mode == 20u || uniforms.blend_mode == 21u {
                // Outline: replace image with stroke color, use computed alpha
                out_r = sr;
                out_g = sg;
                out_b = sb;
                out_a = stencil_a;
            } else {
                // Stencil
                out_r = blended_r;
                out_g = blended_g;
                out_b = blended_b;
                out_a = stencil_a;
            }
        } else {
            // Normal blending: over-composite
            let inv = 1.0 - sa;
            out_r = clamp(blended_r * sa + src_r * inv, 0.0, 1.0);
            out_g = clamp(blended_g * sa + src_g * inv, 0.0, 1.0);
            out_b = clamp(blended_b * sa + src_b * inv, 0.0, 1.0);
            out_a = src_a;
        }
    }

    // Apply source opacity
    out_a = out_a * uniforms.source_opacity;

    // Pack output
    let r8 = u32(clamp(out_r, 0.0, 1.0) * 255.0 + 0.5);
    let g8 = u32(clamp(out_g, 0.0, 1.0) * 255.0 + 0.5);
    let b8 = u32(clamp(out_b, 0.0, 1.0) * 255.0 + 0.5);
    let a8 = u32(clamp(out_a, 0.0, 1.0) * 255.0 + 0.5);
    dst[idx] = r8 | (g8 << 8u) | (b8 << 16u) | (a8 << 24u);
}
