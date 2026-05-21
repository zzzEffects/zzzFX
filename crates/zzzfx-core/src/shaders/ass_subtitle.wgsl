// zzzFX ASS Subtitle GPU compositor
// Per-pixel: composites a source RGBA buffer onto destination with blend mode.
// One thread per output pixel.

struct Uniforms {
    width: u32,
    height: u32,
    blend_mode: u32,  // 0=Normal, 1=Add, 2=Screen, 3=Multiply, 4=Overlay
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<uniform> uniforms: Uniforms;
@group(0) @binding(2) var<storage, read_write> dst: array<u32>;

fn unpack_rgba8(packed: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(packed & 0xFFu),
        f32((packed >> 8u) & 0xFFu),
        f32((packed >> 16u) & 0xFFu),
        f32(packed >> 24u),
    ) / 255.0;
}

fn pack_rgba8(color: vec4<f32>) -> u32 {
    let r = u32(clamp(color.r, 0.0, 1.0) * 255.0 + 0.5);
    let g = u32(clamp(color.g, 0.0, 1.0) * 255.0 + 0.5);
    let b = u32(clamp(color.b, 0.0, 1.0) * 255.0 + 0.5);
    let a = u32(clamp(color.a, 0.0, 1.0) * 255.0 + 0.5);
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

fn blend_pixel(mode: u32, src: vec4<f32>, dst_in: vec4<f32>) -> vec4<f32> {
    if mode == 0u {
        // Normal (source-over)
        let out_a = src.a + dst_in.a * (1.0 - src.a);
        if out_a < 0.001 {
            return vec4<f32>(0.0);
        }
        let out_r = (src.r + dst_in.r * (1.0 - src.a)) / out_a;
        let out_g = (src.g + dst_in.g * (1.0 - src.a)) / out_a;
        let out_b = (src.b + dst_in.b * (1.0 - src.a)) / out_a;
        return vec4<f32>(out_r, out_g, out_b, out_a);
    }
    if mode == 1u {
        // Add
        return vec4<f32>(
            min(src.r + dst_in.r, 1.0),
            min(src.g + dst_in.g, 1.0),
            min(src.b + dst_in.b, 1.0),
            min(src.a + dst_in.a, 1.0),
        );
    }
    if mode == 2u {
        // Screen
        return vec4<f32>(
            1.0 - (1.0 - src.r) * (1.0 - dst_in.r),
            1.0 - (1.0 - src.g) * (1.0 - dst_in.g),
            1.0 - (1.0 - src.b) * (1.0 - dst_in.b),
            1.0 - (1.0 - src.a) * (1.0 - dst_in.a),
        );
    }
    if mode == 3u {
        // Multiply
        return src * dst_in;
    }
    if mode == 4u {
        // Overlay
        let overlay = |s: f32, d: f32| -> f32 {
            if d < 0.5 { return 2.0 * d * s; }
            return 1.0 - 2.0 * (1.0 - d) * (1.0 - s);
        };
        return vec4<f32>(
            overlay(src.r, dst_in.r),
            overlay(src.g, dst_in.g),
            overlay(src.b, dst_in.b),
            overlay(src.a, dst_in.a),
        );
    }
    // Default: Normal
    let out_a = src.a + dst_in.a * (1.0 - src.a);
    if out_a < 0.001 { return vec4<f32>(0.0); }
    return vec4<f32>(
        (src.r + dst_in.r * (1.0 - src.a)) / out_a,
        (src.g + dst_in.g * (1.0 - src.a)) / out_a,
        (src.b + dst_in.b * (1.0 - src.a)) / out_a,
        out_a,
    );
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= uniforms.width || gid.y >= uniforms.height {
        return;
    }

    let idx = gid.y * uniforms.width + gid.x;
    let src_pixel = unpack_rgba8(src[idx]);
    let dst_pixel = unpack_rgba8(dst[idx]);

    let result = blend_pixel(uniforms.blend_mode, src_pixel, dst_pixel);
    dst[idx] = pack_rgba8(result);
}
