// Ambient Light Fusion — GPU composite shader (single pass, no loops)

struct Uniforms {
    width: u32,
    height: u32,
    intensity: f32,
    light_wrap: f32,
    ambient_tint: f32,
    brightness: f32,
    fg_opacity: f32,
    bg_opacity: f32,
}

@group(0) @binding(0) var<storage, read> fg: array<u32>;
@group(0) @binding(1) var<storage, read> bg: array<u32>;
@group(0) @binding(2) var<storage, read> ambient_local: array<u32>;
@group(0) @binding(3) var<storage, read> ambient_global: array<u32>;
@group(0) @binding(4) var<storage, read> edge_factor: array<f32>;
@group(0) @binding(5) var<uniform> params: Uniforms;
@group(0) @binding(6) var<storage, read_write> dst: array<u32>;

fn unpack1(p: u32) -> f32 { return f32(p & 0xFFu) * 0.003921568627451; }     // /255
fn unpack2(p: u32) -> f32 { return f32((p >> 8u) & 0xFFu) * 0.003921568627451; }
fn unpack3(p: u32) -> f32 { return f32((p >> 16u) & 0xFFu) * 0.003921568627451; }
fn unpack4(p: u32) -> f32 { return f32(p >> 24u) * 0.003921568627451; }

fn pack(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let r8 = u32(clamp(r, 0.0, 1.0) * 255.0 + 0.5);
    let g8 = u32(clamp(g, 0.0, 1.0) * 255.0 + 0.5);
    let b8 = u32(clamp(b, 0.0, 1.0) * 255.0 + 0.5);
    let a8 = u32(clamp(a, 0.0, 1.0) * 255.0 + 0.5);
    return r8 | (g8 << 8u) | (b8 << 16u) | (a8 << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = params.width;
    let h = params.height;
    if gid.x >= w || gid.y >= h { return; }
    let i = gid.y * w + gid.x;

    // --- Foreground ---
    let fg_p = fg[i];
    var fg_r = unpack1(fg_p);
    var fg_g = unpack2(fg_p);
    var fg_b = unpack3(fg_p);
    var fg_a = unpack4(fg_p) * params.fg_opacity;

    // Background-only: pass through
    if fg_a <= 0.0 {
        let bg_a = unpack4(bg[i]) * params.bg_opacity;
        if params.bg_opacity >= 1.0 {
            dst[i] = bg[i];
        } else {
            let bg_p = bg[i];
            dst[i] = pack(unpack1(bg_p), unpack2(bg_p), unpack3(bg_p), bg_a);
        }
        return;
    }

    let ef = edge_factor[i];
    let la = 1.0 - ef;  // light_amount: 1=edge, 0=interior
    let inten = params.intensity;
    let lw = params.light_wrap;
    let at = params.ambient_tint;
    let br = params.brightness;

    // --- Light wrap from local ambient ---
    let loc_p = ambient_local[i];
    let loc_r = unpack1(loc_p) * br;
    let loc_g = unpack2(loc_p) * br;
    let loc_b = unpack3(loc_p) * br;
    let loc_lum = 0.2126 * loc_r + 0.7152 * loc_g + 0.0722 * loc_b;

    // --- Tint from global ambient ---
    let glb_p = ambient_global[i];
    let glb_r = unpack1(glb_p) * br;
    let glb_g = unpack2(glb_p) * br;
    let glb_b = unpack3(glb_p) * br;

    // --- Ambient tint ---
    let tint = la * at * inten;
    let fg_lum = 0.2126 * fg_r + 0.7152 * fg_g + 0.0722 * fg_b;
    let glb_lum = 0.2126 * glb_r + 0.7152 * glb_g + 0.0722 * glb_b;
    let glb_lum_s = max(glb_lum, 0.001);
    fg_r = fg_r + (fg_lum * (glb_r / glb_lum_s) - fg_r) * tint;
    fg_g = fg_g + (fg_lum * (glb_g / glb_lum_s) - fg_g) * tint;
    fg_b = fg_b + (fg_lum * (glb_b / glb_lum_s) - fg_b) * tint;

    // --- Light wrap ---
    let wrap = la * lw * inten;
    let bg_gate = clamp(loc_lum * 2.0, 0.0, 1.0);
    fg_r = clamp(fg_r + loc_r * wrap * bg_gate, 0.0, 1.0);
    fg_g = clamp(fg_g + loc_g * wrap * bg_gate, 0.0, 1.0);
    fg_b = clamp(fg_b + loc_b * wrap * bg_gate, 0.0, 1.0);

    // --- OVER composite ---
    let bg_p = bg[i];
    let bg_r = unpack1(bg_p);
    let bg_g = unpack2(bg_p);
    let bg_b = unpack3(bg_p);
    let bg_a = unpack4(bg_p) * params.bg_opacity;
    let inv = 1.0 - fg_a;

    dst[i] = pack(
        fg_r * fg_a + bg_r * inv,
        fg_g * fg_a + bg_g * inv,
        fg_b * fg_a + bg_b * inv,
        fg_a + bg_a * inv,
    );
}
