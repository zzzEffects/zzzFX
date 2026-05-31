// SpriteSheet GPU render: crop + scale + center + displacement + rotation.
// Uses shared.wgsl for unpack_rgba8 and pack_rgba8.

struct Uniforms {
    dst_w: u32,
    dst_h: u32,
    sheet_w: u32,
    sheet_h: u32,
    crop_x: u32,
    crop_y: u32,
    crop_w: u32,
    crop_h: u32,
    scale: f32,
    filter_mode: u32,  // 0 = nearest, 1 = bilinear
    displacement_x: f32,
    displacement_y: f32,
    rotation_enabled: u32, // 0 or 1
    cos_rotation: f32,
    sin_rotation: f32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> sheet: array<u32>;
@group(0) @binding(1) var<uniform> uniforms: Uniforms;
@group(0) @binding(2) var<storage, read_write> dst: array<u32>;

fn sample_sheet(x: u32, y: u32) -> vec4<f32> {
    let cx = clamp(x, 0u, uniforms.sheet_w - 1u);
    let cy = clamp(y, 0u, uniforms.sheet_h - 1u);
    return unpack_rgba8(sheet[cy * uniforms.sheet_w + cx]);
}

fn sample_crop_bilinear(cx: f32, cy: f32) -> vec4<f32> {
    let ix0 = i32(floor(cx - 0.5));
    let iy0 = i32(floor(cy - 0.5));
    let ix1 = ix0 + 1;
    let iy1 = iy0 + 1;
    let wx = (cx - 0.5) - f32(ix0);
    let wy = (cy - 0.5) - f32(iy0);

    let sx0 = clamp(ix0, 0, i32(uniforms.crop_w) - 1);
    let sy0 = clamp(iy0, 0, i32(uniforms.crop_h) - 1);
    let sx1 = clamp(ix1, 0, i32(uniforms.crop_w) - 1);
    let sy1 = clamp(iy1, 0, i32(uniforms.crop_h) - 1);

    let c00 = sample_sheet(uniforms.crop_x + u32(sx0), uniforms.crop_y + u32(sy0));
    let c10 = sample_sheet(uniforms.crop_x + u32(sx1), uniforms.crop_y + u32(sy0));
    let c01 = sample_sheet(uniforms.crop_x + u32(sx0), uniforms.crop_y + u32(sy1));
    let c11 = sample_sheet(uniforms.crop_x + u32(sx1), uniforms.crop_y + u32(sy1));

    let top = mix(c00, c10, vec4<f32>(wx));
    let bottom = mix(c01, c11, vec4<f32>(wx));
    return mix(top, bottom, vec4<f32>(wy));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= uniforms.dst_w || gid.y >= uniforms.dst_h {
        return;
    }
    let out_idx = gid.y * uniforms.dst_w + gid.x;

    // Scaled sprite dimensions
    let base_w = f32(uniforms.crop_w) * uniforms.scale;
    let base_h = f32(uniforms.crop_h) * uniforms.scale;

    // Output dimensions (with rotation expansion)
    var out_w_f = base_w;
    var out_h_f = base_h;
    if uniforms.rotation_enabled != 0u {
        let ac = abs(uniforms.cos_rotation);
        let as_ = abs(uniforms.sin_rotation);
        out_w_f = base_w * ac + base_h * as_;
        out_h_f = base_w * as_ + base_h * ac;
    }
    let out_w = max(1u, u32(ceil(out_w_f)));
    let out_h = max(1u, u32(ceil(out_h_f)));

    // Signed centering
    var offset_x = (i32(uniforms.dst_w) - i32(out_w)) / 2;
    var offset_y = (i32(uniforms.dst_h) - i32(out_h)) / 2;
    offset_x += i32(round(uniforms.displacement_x));
    offset_y += i32(round(uniforms.displacement_y));

    let gx = i32(gid.x);
    let gy = i32(gid.y);
    if gx < offset_x || gx >= offset_x + i32(out_w)
        || gy < offset_y || gy >= offset_y + i32(out_h)
    {
        dst[out_idx] = 0u;
        return;
    }

    let sx_f = f32(gx - offset_x);
    let sy_f = f32(gy - offset_y);

    // Source coordinates in crop-space
    var cx: f32;
    var cy: f32;

    if uniforms.rotation_enabled != 0u {
        // Inverse rotation around center of base (unrotated) sprite
        let dx = sx_f - out_w_f * 0.5;
        let dy = sy_f - out_h_f * 0.5;
        let rsx = dx * uniforms.cos_rotation + dy * uniforms.sin_rotation + base_w * 0.5;
        let rsy = -dx * uniforms.sin_rotation + dy * uniforms.cos_rotation + base_h * 0.5;
        cx = rsx / uniforms.scale;
        cy = rsy / uniforms.scale;
    } else {
        cx = sx_f / uniforms.scale;
        cy = sy_f / uniforms.scale;
    }

    // Check bounds (in crop-space)
    if cx < 0.0 || cx >= f32(uniforms.crop_w) || cy < 0.0 || cy >= f32(uniforms.crop_h) {
        dst[out_idx] = 0u;
        return;
    }

    if uniforms.filter_mode == 0u {
        // Nearest-neighbor
        let src_x = u32(cx);
        let src_y = u32(cy);
        dst[out_idx] = sheet[(uniforms.crop_y + src_y) * uniforms.sheet_w + (uniforms.crop_x + src_x)];
    } else {
        // Bilinear
        let color = sample_crop_bilinear(cx, cy);
        dst[out_idx] = pack_rgba8(color);
    }
}
