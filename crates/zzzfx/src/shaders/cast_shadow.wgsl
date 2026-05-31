struct Uniforms {
    width: u32,
    height: u32,
    contact_x: f32,
    contact_y: f32,
    normal_x: f32,
    normal_y: f32,
    axis_x: f32,
    axis_y: f32,
    scale: f32,
    shear_angle: f32,
    shear_amount: f32,
    inv_bbox_perp: f32,
    total_dx: f32,
    total_dy: f32,
    pivot_mode: u32,
    fade: f32,
    shadow_r: f32,
    shadow_g: f32,
    shadow_b: f32,
    shadow_a: f32,
    source_opacity: f32,
    blur_radius: u32,
    horizontal: u32,
    alpha_threshold: f32,
    bbox_min_x: f32,
    bbox_max_x: f32,
    bbox_min_y: f32,
    bbox_max_y: f32,
}

// Global resource declarations — different entry points use different subsets
@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<storage, read_write> alpha_a: array<f32>;
@group(0) @binding(2) var<storage, read_write> alpha_b: array<f32>;
@group(0) @binding(3) var<uniform> params: Uniforms;
@group(0) @binding(4) var<storage, read_write> dst: array<u32>;

// ---------------------------------------------------------------------------
// project — inverse-transform each output pixel to sample source alpha
// ---------------------------------------------------------------------------

@compute @workgroup_size(16, 16)
fn project(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = params.width;
    let h = params.height;
    if gid.x >= w || gid.y >= h { return; }
    let i = gid.y * w + gid.x;

    // Inverse displacement
    let px = f32(gid.x) - params.total_dx + 0.5;
    let py = f32(gid.y) - params.total_dy + 0.5;

    // Position relative to contact point
    let rx = px - params.contact_x;
    let ry = py - params.contact_y;

    // Decompose into axis components
    let perp_out = rx * params.normal_x + ry * params.normal_y;
    let along_out = rx * params.axis_x + ry * params.axis_y;

    // Inverse scale
    if params.scale < 0.001 { return; }
    let inv_scale = 1.0 / params.scale;
    let perp_src = perp_out * inv_scale;
    var along_src = along_out * inv_scale;

    // 2D shear based on edge_dist
    let dist_ratio = perp_src / max(params.bbox_max_y - params.bbox_min_y, 1.0);
    let shear_dim = min(f32(params.width), f32(params.height)) * 0.5;
    let sx_s = params.shear_amount * dist_ratio * shear_dim * cos(params.shear_angle);
    let sy_s = params.shear_amount * dist_ratio * shear_dim * sin(params.shear_angle);

    // Remove shear from output then recompute
    let rx_adj = rx - sx_s;
    let ry_adj = ry - sy_s;
    let perp_out_adj = rx_adj * params.normal_x + ry_adj * params.normal_y;
    let along_out_adj = rx_adj * params.axis_x + ry_adj * params.axis_y;
    let perp_src2 = perp_out_adj * inv_scale;
    let along_src2 = along_out_adj * inv_scale;

    // Reconstruct source position
    let sx = params.contact_x + along_src2 * params.axis_x + perp_src2 * params.normal_x;
    let sy = params.contact_y + along_src2 * params.axis_y + perp_src2 * params.normal_y;

    // Update edge_dist for bounds check
    let edge_dist = perp_src2;

    // Check bounds
    if edge_dist <= 0.0 || sx < params.bbox_min_x - 1.0 || sx > params.bbox_max_x + 1.0
        || sy < params.bbox_min_y - 1.0 || sy > params.bbox_max_y + 1.0 {
        return;
    }

    // Bilinear sample source alpha
    let fx = sx - floor(sx);
    let fy = sy - floor(sy);
    let ix0 = i32(floor(sx));
    let iy0 = i32(floor(sy));
    let ix1 = ix0 + 1;
    let iy1 = iy0 + 1;

    let v00 = read_alpha(ix0, iy0, w, h);
    let v10 = read_alpha(ix1, iy0, w, h);
    let v01 = read_alpha(ix0, iy1, w, h);
    let v11 = read_alpha(ix1, iy1, w, h);

    let top = v00 + (v10 - v00) * fx;
    let bot = v01 + (v11 - v01) * fx;
    var a = top + (bot - top) * fy;

    // Fade
    let fade_factor = max(0.0, 1.0 - params.fade * edge_dist * params.inv_bbox_perp);
    alpha_a[i] = max(alpha_a[i], a * fade_factor);
}

fn read_alpha(x: i32, y: i32, w: u32, h: u32) -> f32 {
    if x < 0 || x >= i32(w) || y < 0 || y >= i32(h) { return 0.0; }
    let idx = u32(y) * w + u32(x);
    let a = f32((src[idx] >> 24) & 0xFFu) / 255.0;
    return select(0.0, a, a >= params.alpha_threshold);
}

// ---------------------------------------------------------------------------
// blur — separable box blur (H or V) on alpha buffer
// ---------------------------------------------------------------------------

@compute @workgroup_size(16, 16)
fn blur(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = params.width;
    let h = params.height;
    if gid.x >= w || gid.y >= h { return; }
    let i = gid.y * w + gid.x;
    let radius = i32(params.blur_radius);
    if radius <= 0 { return; }

    var sum = 0.0;
    var count = 0u;

    if params.horizontal != 0u {
        let x0 = max(i32(gid.x) - radius, 0);
        let x1 = min(i32(gid.x) + radius + 1, i32(w));
        for (var k = x0; k < x1; k++) {
            sum += alpha_a[gid.y * w + u32(k)];
            count += 1u;
        }
    } else {
        let y0 = max(i32(gid.y) - radius, 0);
        let y1 = min(i32(gid.y) + radius + 1, i32(h));
        for (var k = y0; k < y1; k++) {
            sum += alpha_a[u32(k) * w + gid.x];
            count += 1u;
        }
    }
    alpha_b[i] = sum / f32(count);
}

// ---------------------------------------------------------------------------
// composite — color + source-over-shadow
// ---------------------------------------------------------------------------

@compute @workgroup_size(16, 16)
fn composite(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = params.width;
    let h = params.height;
    if gid.x >= w || gid.y >= h { return; }
    let i = gid.y * w + gid.x;

    let alpha = alpha_a[i];
    let sh_alpha = alpha * params.shadow_a;

    let src_rgba = unpack_rgba8(src[i]);
    var sa = src_rgba.a * params.source_opacity;

    let inv = 1.0 - sa;

    var out_r = src_rgba.r * sa + params.shadow_r * sh_alpha * inv;
    var out_g = src_rgba.g * sa + params.shadow_g * sh_alpha * inv;
    var out_b = src_rgba.b * sa + params.shadow_b * sh_alpha * inv;
    var out_a = sa + sh_alpha * inv;

    dst[i] = pack_rgba8(vec4(out_r, out_g, out_b, out_a));
}
