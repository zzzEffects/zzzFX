// Single JFA pass at a given step_size.
// Each pixel samples 8 neighbors at offsets (dx*step, dy*step) for dx,dy in {-1,0,1} excluding (0,0).

struct Uniforms {
    width: u32,
    height: u32,
    step: u32,
    use_sharp_corners: u32,
}

@group(0) @binding(0) var<storage, read> in_seeds: array<u32>;
@group(0) @binding(1) var<uniform> uniforms: Uniforms;
@group(0) @binding(2) var<storage, read_write> out_seeds: array<u32>;

// F3: Compile-time constant offsets instead of runtime array construction
const OFFSETS: array<vec2<i32>, 8> = array<vec2<i32>, 8>(
    vec2(-1, -1),   // top-left
    vec2( 0, -1),   // top
    vec2( 1, -1),   // top-right
    vec2(-1,  0),   // left
    vec2( 1,  0),   // right
    vec2(-1,  1),   // bottom-left
    vec2( 0,  1),   // bottom
    vec2( 1,  1),   // bottom-right
);

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= uniforms.width || gid.y >= uniforms.height {
        return;
    }

    let idx = gid.y * uniforms.width + gid.x;
    let step = uniforms.step;
    let w = uniforms.width;
    let h = uniforms.height;

    var best_seed_packed = in_seeds[idx];
    var best_dist: f32 = 1e10;

    if is_valid_seed(best_seed_packed) {
        let s = decode_seed(best_seed_packed);
        best_dist = seed_distance(gid.x, gid.y, s, uniforms.use_sharp_corners != 0u);
    }

    for (var i = 0u; i < 8u; i++) {
        let dx = OFFSETS[i].x;
        let dy = OFFSETS[i].y;
        let nx = i32(gid.x) + dx * i32(step);
        let ny = i32(gid.y) + dy * i32(step);

        if nx < 0 || nx >= i32(w) || ny < 0 || ny >= i32(h) {
            continue;
        }

        let n_idx = u32(ny) * w + u32(nx);
        let n_packed = in_seeds[n_idx];

        if !is_valid_seed(n_packed) {
            continue;
        }

        let s = decode_seed(n_packed);
        let d = seed_distance(gid.x, gid.y, s, uniforms.use_sharp_corners != 0u);

        if d < best_dist {
            best_dist = d;
            best_seed_packed = n_packed;
        }
    }

    out_seeds[idx] = best_seed_packed;
}
