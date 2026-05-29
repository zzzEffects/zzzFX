// Shared utilities for Example Effect compute shaders.
// Prefixed to every effect-specific shader by load_shader().

fn byte_to_float(b: u32) -> f32 {
    return f32(b) / 255.0;
}

fn overlay_channel(base: f32, blend: f32) -> f32 {
    if base < 0.5 {
        return 2.0 * base * blend;
    } else {
        return 1.0 - 2.0 * (1.0 - base) * (1.0 - blend);
    }
}

fn luminance(r: f32, g: f32, b: f32) -> f32 {
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
