// zzzFX ASS Subtitle GPU compositor
// Uses shared.wgsl for unpack_rgba8, pack_rgba8, and blend_pixel.

struct Uniforms {
    width: u32,
    height: u32,
    blend_mode: u32,  // 0=Normal, 1=Add, 2=Screen, 3=Multiply, 4=Overlay
    _pad: u32,
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
    let src_pixel = unpack_rgba8(src[idx]);
    let dst_pixel = unpack_rgba8(dst[idx]);

    let result = blend_pixel(uniforms.blend_mode, src_pixel, dst_pixel);
    dst[idx] = pack_rgba8(result);
}
