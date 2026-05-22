// Stage 4+5: Stroke composition with all blend modes.
// Decodes JFA seeds, computes stroke alpha, fills color, blends with source.
// Uses shared.wgsl for hash, blend_channel, pack_rgba8, gaussian_edge, decode_seed, etc.

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
    edge_blend: f32,
    source_opacity: f32,
    stroke_position: u32,   // 0=Outer, 1=Inner, 2=Center
    fill_mode: u32,         // 0=SolidColor, 1=DistanceGradient, 2=Gradient, 3=SourceColorExtension
    blend_mode: u32,        // 0-21
    use_sharp_corners: u32,
    grad_start_x: f32,
    grad_start_y: f32,
    grad_end_x: f32,
    grad_end_y: f32,
    grad_start_r: f32,
    grad_start_g: f32,
    grad_start_b: f32,
    _pad0: u32,
    grad_end_r: f32,
    grad_end_g: f32,
    grad_end_b: f32,
    _pad1: u32,
}

@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<uniform> uniforms: Uniforms;
@group(0) @binding(2) var<storage, read> seeds: array<u32>;
@group(0) @binding(3) var<storage, read_write> dst: array<u32>;

fn is_inside(alpha: f32) -> bool {
    return alpha >= uniforms.alpha_threshold;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= uniforms.width || gid.y >= uniforms.height {
        return;
    }

    let idx = gid.y * uniforms.width + gid.x;
    let pixel = src[idx];
    let src_color = unpack_rgba8(pixel);
    let src_r = src_color.r;
    let src_g = src_color.g;
    let src_b = src_color.b;
    let src_a = src_color.a;
    let inside = is_inside(src_a);

    // Compute distance from JFA seed
    var d: f32 = 1e10;
    let seed_packed = seeds[idx];
    if is_valid_seed(seed_packed) {
        d = seed_distance(gid.x, gid.y, decode_seed(seed_packed), uniforms.use_sharp_corners != 0u);
    }

    // Stroke alpha from distance
    let blend_range = uniforms.edge_blend / 2.0;
    let lower_bound = uniforms.alpha_threshold - blend_range;
    let upper_bound = uniforms.alpha_threshold + blend_range;
    var stroke_alpha_local: f32;
    let sigma = uniforms.feather_px / 3.0;
    switch uniforms.stroke_position {
        case 0u: { // Outer
            if blend_range <= 0.0 {
                if inside {
                    stroke_alpha_local = 0.0;
                } else {
                    stroke_alpha_local = gaussian_edge(sigma, uniforms.stroke_width_px, d);
                }
            } else if src_a <= lower_bound {
                stroke_alpha_local = gaussian_edge(sigma, uniforms.stroke_width_px, d);
            } else if src_a >= upper_bound {
                stroke_alpha_local = 0.0;
            } else {
                stroke_alpha_local = gaussian_edge(sigma, uniforms.stroke_width_px, d) * (1.0 - src_a);
            }
        }
        case 1u: { // Inner
            if blend_range <= 0.0 {
                if !inside {
                    stroke_alpha_local = 0.0;
                } else {
                    stroke_alpha_local = gaussian_edge(sigma, uniforms.stroke_width_px, d);
                }
            } else if src_a <= lower_bound {
                stroke_alpha_local = 0.0;
            } else if src_a >= upper_bound {
                stroke_alpha_local = gaussian_edge(sigma, uniforms.stroke_width_px, d);
            } else {
                stroke_alpha_local = gaussian_edge(sigma, uniforms.stroke_width_px, d) * (1.0 - src_a);
            }
        }
        case 2u: { // Center
            let half_w = uniforms.stroke_width_px * 0.5;
            if blend_range <= 0.0 {
                stroke_alpha_local = gaussian_edge(sigma, half_w, d);
            } else if src_a <= lower_bound || src_a >= upper_bound {
                stroke_alpha_local = 0.0;
            } else {
                stroke_alpha_local = gaussian_edge(sigma, half_w, d) * (1.0 - src_a);
            }
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
                    let src_idx = seed_coord.y * uniforms.width + seed_coord.x;
                    let edge_color = unpack_rgba8(src[src_idx]);
                    sr = edge_color.r;
                    sg = edge_color.g;
                    sb = edge_color.b;
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

        let rng0 = hash(idx);
        let rng1 = hash(idx ^ 0xDEADBEEFu);
        let rng2 = hash(idx ^ 0xCAFEBABEu);

        let blended_r = blend_channel(uniforms.blend_mode, src_r, sr, sa, rng0);
        let blended_g = blend_channel(uniforms.blend_mode, src_g, sg, sa, rng1);
        let blended_b = blend_channel(uniforms.blend_mode, src_b, sb, sa, rng2);

        if is_stencil_or_outline(uniforms.blend_mode) {
            var stencil_a: f32;
            switch uniforms.blend_mode {
                case 18u: { stencil_a = sa; }                          // StencilAlpha
                case 19u: { stencil_a = sa * luminance(sr, sg, sb); }  // StencilLuma
                case 20u: { stencil_a = sa; }                          // OutlineAlpha
                case 21u: { stencil_a = sa * luminance(sr, sg, sb); }  // OutlineLuma
                default: { stencil_a = sa; }
            }

            if uniforms.blend_mode == 20u || uniforms.blend_mode == 21u {
                out_r = sr;
                out_g = sg;
                out_b = sb;
                out_a = stencil_a * uniforms.source_opacity;
            } else {
                out_r = blended_r;
                out_g = blended_g;
                out_b = blended_b;
                out_a = stencil_a * uniforms.source_opacity;
            }
        } else {
            let inv = 1.0 - sa;
            out_r = clamp(fma(blended_r, sa, src_r * uniforms.source_opacity * inv), 0.0, 1.0);
            out_g = clamp(fma(blended_g, sa, src_g * uniforms.source_opacity * inv), 0.0, 1.0);
            out_b = clamp(fma(blended_b, sa, src_b * uniforms.source_opacity * inv), 0.0, 1.0);
            out_a = clamp(fma(sa, 1.0, src_a * uniforms.source_opacity * inv), 0.0, 1.0);
        }
    } else {
        out_r = src_r * uniforms.source_opacity;
        out_g = src_g * uniforms.source_opacity;
        out_b = src_b * uniforms.source_opacity;
        out_a = src_a * uniforms.source_opacity;
    }

    // F1: Use built-in pack4xU8
    dst[idx] = pack_rgba8(vec4<f32>(out_r, out_g, out_b, out_a));
}
