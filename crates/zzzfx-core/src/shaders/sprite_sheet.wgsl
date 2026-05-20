// zzzSpriteSheetReader GPU render: single-pass crop + scale + center.
// One thread per output pixel.

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
}

@group(0) @binding(0) var<storage, read> sheet: array<u32>;
@group(0) @binding(1) var<uniform> uniforms: Uniforms;
@group(0) @binding(2) var<storage, read_write> dst: array<u32>;

fn sample_sheet(x: u32, y: u32) -> vec4<f32> {
    let cx = clamp(x, 0u, uniforms.sheet_w - 1u);
    let cy = clamp(y, 0u, uniforms.sheet_h - 1u);
    let pixel = sheet[cy * uniforms.sheet_w + cx];
    return vec4<f32>(
        f32(pixel & 0xFFu),
        f32((pixel >> 8u) & 0xFFu),
        f32((pixel >> 16u) & 0xFFu),
        f32(pixel >> 24u),
    ) / 255.0;
}

fn pack_rgba8(color: vec4<f32>) -> u32 {
    let r = u32(clamp(color.r, 0.0, 1.0) * 255.0 + 0.5);
    let g = u32(clamp(color.g, 0.0, 1.0) * 255.0 + 0.5);
    let b = u32(clamp(color.b, 0.0, 1.0) * 255.0 + 0.5);
    let a = u32(clamp(color.a, 0.0, 1.0) * 255.0 + 0.5);
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= uniforms.dst_w || gid.y >= uniforms.dst_h {
        return;
    }

    let out_idx = gid.y * uniforms.dst_w + gid.x;

    // Scaled output dimensions (match CPU: round(cw * scale).max(1))
    let out_w = max(1u, u32(round(f32(uniforms.crop_w) * uniforms.scale)));
    let out_h = max(1u, u32(round(f32(uniforms.crop_h) * uniforms.scale)));

    // Center the scaled sprite in the output buffer
    let offset_x = if uniforms.dst_w >= out_w { (uniforms.dst_w - out_w) / 2u } else { 0u };
    let offset_y = if uniforms.dst_h >= out_h { (uniforms.dst_h - out_h) / 2u } else { 0u };

    // Check if this output pixel is within the scaled sprite region
    if gid.x < offset_x || gid.x >= offset_x + out_w
        || gid.y < offset_y || gid.y >= offset_y + out_h
    {
        dst[out_idx] = 0u;
        return;
    }

    // Position within the scaled sprite
    let sx = gid.x - offset_x;
    let sy = gid.y - offset_y;

    if uniforms.filter_mode == 0u {
        // Nearest-neighbor
        let src_x = u32(f32(sx) / uniforms.scale);
        let src_y = u32(f32(sy) / uniforms.scale);

        let sheet_x = uniforms.crop_x + src_x;
        let sheet_y = uniforms.crop_y + src_y;

        // Clamp to sheet bounds (out-of-bounds = transparent: shader sample
        // is clamped, but if src_x >= crop_w, the pixel is conceptually
        // outside the crop — we treat as transparent via the pre-fill)
        if src_x < uniforms.crop_w && src_y < uniforms.crop_h {
            let cx = clamp(sheet_x, 0u, uniforms.sheet_w - 1u);
            let cy = clamp(sheet_y, 0u, uniforms.sheet_h - 1u);
            dst[out_idx] = sheet[cy * uniforms.sheet_w + cx];
        } else {
            dst[out_idx] = 0u;
        }
    } else {
        // Bilinear: map output pixel center back to fractional crop-space coords
        let fx = f32(sx) / uniforms.scale - 0.5;
        let fy = f32(sy) / uniforms.scale - 0.5;

        let ix0 = i32(floor(fx));
        let iy0 = i32(floor(fy));
        let ix1 = ix0 + 1;
        let iy1 = iy0 + 1;

        let wx = fx - f32(ix0);
        let wy = fy - f32(iy0);

        // Clamp integer coords to crop bounds
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
