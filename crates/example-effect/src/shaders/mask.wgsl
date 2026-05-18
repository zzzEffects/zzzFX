// Stage 1+2: Binary mask + edge detection + JFA seed initialization.
// Output: packed u32 seed buffer (seed_x | seed_y << 16), sentinel = 0xFFFFFFFF.

struct Uniforms {
    width: u32,
    height: u32,
    alpha_threshold: f32,
    // padding to 16-byte alignment
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<uniform> uniforms: Uniforms;
@group(0) @binding(2) var<storage, read_write> seeds: array<u32>;

fn is_inside(alpha: f32) -> bool {
    return alpha >= uniforms.alpha_threshold;
}

fn has_outside_neighbor(x: u32, y: u32, w: u32, h: u32) -> bool {
    let idx = y * w + x;
    // Check 4-neighbors: if any neighbor is outside, this is an edge
    if x > 0u && !is_inside(f32((src[idx - 1u] >> 24u) & 0xFFu) / 255.0) {
        return true;
    }
    if x + 1u < w && !is_inside(f32((src[idx + 1u] >> 24u) & 0xFFu) / 255.0) {
        return true;
    }
    if y > 0u && !is_inside(f32((src[idx - w] >> 24u) & 0xFFu) / 255.0) {
        return true;
    }
    if y + 1u < h && !is_inside(f32((src[idx + w] >> 24u) & 0xFFu) / 255.0) {
        return true;
    }
    return false;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= uniforms.width || gid.y >= uniforms.height {
        return;
    }

    let idx = gid.y * uniforms.width + gid.x;
    let pixel = src[idx];
    let alpha = f32((pixel >> 24u) & 0xFFu) / 255.0;
    let inside = is_inside(alpha);

    if inside && has_outside_neighbor(gid.x, gid.y, uniforms.width, uniforms.height) {
        // Edge pixel: store own coordinates as seed
        seeds[idx] = gid.x | (gid.y << 16u);
    } else {
        // Non-edge: sentinel
        seeds[idx] = 0xFFFFFFFFu;
    }
}
