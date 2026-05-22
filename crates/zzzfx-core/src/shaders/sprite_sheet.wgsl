// zzzSpriteSheetReader GPU render: single-pass crop + scale + center + displacement.
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
}

@group(0) @binding(0) var<storage, read> sheet: array<u32>;
@group(0) @binding(1) var<uniform> uniforms: Uniforms;
@group(0) @binding(2) var<storage, read_write> dst: array<u32>;

fn sample_sheet(x: u32, y: u32) -> vec4<f32> {
    let cx = clamp(x, 0u, uniforms.sheet_w - 1u);
    let cy = clamp(y, 0u, uniforms.sheet_h - 1u);
    return unpack_rgba8(sheet[cy * uniforms.sheet_w + cx]);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= uniforms.dst_w || gid.y >= uniforms.dst_h {
        return;
    }

    let out_idx = gid.y * uniforms.dst_w + gid.x;

    let out_w = max(1u, u32(round(f32(uniforms.crop_w) * uniforms.scale)));
    let out_h = max(1u, u32(round(f32(uniforms.crop_h) * uniforms.scale)));

    // Signed centering — scaled sprite is always centered; may overflow all sides.
    var offset_x = (i32(uniforms.dst_w) - i32(out_w)) / 2;
    var offset_y = (i32(uniforms.dst_h) - i32(out_h)) / 2;

    // Apply displacement (quantization already done on CPU side)
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

    let sx = u32(gx - offset_x);
    let sy = u32(gy - offset_y);

    if uniforms.filter_mode == 0u {
        // Nearest-neighbor
        let src_x = u32(f32(sx) / uniforms.scale);
        let src_y = u32(f32(sy) / uniforms.scale);

        if src_x < uniforms.crop_w && src_y < uniforms.crop_h {
            let cx = clamp(uniforms.crop_x + src_x, 0u, uniforms.sheet_w - 1u);
            let cy = clamp(uniforms.crop_y + src_y, 0u, uniforms.sheet_h - 1u);
            dst[out_idx] = sheet[cy * uniforms.sheet_w + cx];
        } else {
            dst[out_idx] = 0u;
        }
    } else {
        // Bilinear
        let fx = f32(sx) / uniforms.scale - 0.5;
        let fy = f32(sy) / uniforms.scale - 0.5;

        let ix0 = i32(floor(fx));
        let iy0 = i32(floor(fy));
        let ix1 = ix0 + 1;
        let iy1 = iy0 + 1;

        let wx = fx - f32(ix0);
        let wy = fy - f32(iy0);

        let cx0 = clamp(ix0, 0, i32(uniforms.crop_w) - 1);
        let cy0 = clamp(iy0, 0, i32(uniforms.crop_h) - 1);
        let cx1 = clamp(ix1, 0, i32(uniforms.crop_w) - 1);
        let cy1 = clamp(iy1, 0, i32(uniforms.crop_h) - 1);

        let sx00 = uniforms.crop_x + u32(cx0);
        let sy00 = uniforms.crop_y + u32(cy0);
        let sx10 = uniforms.crop_x + u32(cx1);
        let sy10 = uniforms.crop_y + u32(cy0);
        let sx01 = uniforms.crop_x + u32(cx0);
        let sy01 = uniforms.crop_y + u32(cy1);
        let sx11 = uniforms.crop_x + u32(cx1);
        let sy11 = uniforms.crop_y + u32(cy1);

        let c00 = sample_sheet(sx00, sy00);
        let c10 = sample_sheet(sx10, sy10);
        let c01 = sample_sheet(sx01, sy01);
        let c11 = sample_sheet(sx11, sy11);

        let top = mix(c00, c10, vec4<f32>(wx));
        let bottom = mix(c01, c11, vec4<f32>(wx));
        let color = mix(top, bottom, vec4<f32>(wy));

        dst[out_idx] = pack_rgba8(color);
    }
}
