// Standard (ExampleEffect) compute shader.
// Applies: brightness, tint, invert, contrast, saturation, color preset.

struct Uniforms {
    width: u32,
    height: u32,
    brightness: f32,
    tint_r: f32,
    tint_g: f32,
    tint_b: f32,
    invert: u32,
    contrast: f32,
    saturation: f32,
    color_preset: u32,
}

@group(0) @binding(0) var<storage, read>       src: array<u32>;
@group(0) @binding(1) var<uniform>              u: Uniforms;
@group(0) @binding(2) var<storage, read_write>  dst: array<u32>;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = id.x;
    let y = id.y;

    if x >= u.width || y >= u.height {
        return;
    }

    let i = y * u.width + x;
    let packed = src[i];

    var r = byte_to_float((packed >> 0u) & 0xFFu);
    var g = byte_to_float((packed >> 8u) & 0xFFu);
    var b = byte_to_float((packed >> 16u) & 0xFFu);
    let a = (packed >> 24u) & 0xFFu;

    // Brightness
    r *= u.brightness;
    g *= u.brightness;
    b *= u.brightness;

    // Tint (per-channel)
    r *= u.tint_r;
    g *= u.tint_g;
    b *= u.tint_b;

    // Invert
    if u.invert != 0u {
        r = 1.0 - r;
        g = 1.0 - g;
        b = 1.0 - b;
    }

    // Contrast (linear around 0.5)
    if abs(u.contrast - 1.0) > 0.001 {
        let c = u.contrast;
        r = (r - 0.5) * c + 0.5;
        g = (g - 0.5) * c + 0.5;
        b = (b - 0.5) * c + 0.5;
    }

    // Saturation (luminance-preserving)
    if abs(u.saturation - 1.0) > 0.001 {
        let lum = luminance(r, g, b);
        let s = u.saturation;
        r = lum + (r - lum) * s;
        g = lum + (g - lum) * s;
        b = lum + (b - lum) * s;
    }

    // Color preset
    switch u.color_preset {
        case 1u: {
            r = r * 1.15;
            g = g * 0.95;
            b = b * 0.75;
        }
        case 2u: {
            r = r * 0.85;
            g = g * 0.95;
            b = b * 1.15;
        }
        case 3u: {
            let lr = r;
            let lg = g;
            let lb = b;
            r = lr * 0.393 + lg * 0.769 + lb * 0.189;
            g = lr * 0.349 + lg * 0.686 + lb * 0.168;
            b = lr * 0.272 + lg * 0.534 + lb * 0.131;
        }
        default: {}
    }

    r = clamp(r, 0.0, 1.0);
    g = clamp(g, 0.0, 1.0);
    b = clamp(b, 0.0, 1.0);

    let out_r = u32(r * 255.0 + 0.5);
    let out_g = u32(g * 255.0 + 0.5);
    let out_b = u32(b * 255.0 + 0.5);

    dst[i] = (a << 24u) | (out_b << 16u) | (out_g << 8u) | out_r;
}
