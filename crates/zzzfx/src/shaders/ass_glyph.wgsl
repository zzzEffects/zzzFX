// ASS Glyph compositing compute shader.
// Each workgroup processes one glyph: fill + shadow + outline in a single pass.
// Uses direct_composite (source-over) for fill/shadow and direct_composite_max
// for outline to avoid additive accumulation at overlapping samples.

struct GlyphGpuData {
    glyph_offset: u32,   // Byte offset into bitmaps buffer
    bitmap_w: u32,
    bitmap_h: u32,
    pos_x: i32,
    pos_y: i32,
    fill_color: u32,     // packed RGBA8
    outline_color: u32,
    shadow_color: u32,
    outline_radius: i32,
    shadow_dx: f32,
    shadow_dy: f32,
    flags: u32,          // bit0=has_fill, bit1=has_outline, bit2=has_shadow
}

struct GlyphUniforms {
    output_width: u32,
    output_height: u32,
    glyph_count: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> u: GlyphUniforms;
@group(0) @binding(1) var<storage, read> glyphs: array<GlyphGpuData>;
@group(0) @binding(2) var<storage, read> bitmaps: array<u32>;
@group(0) @binding(3) var<storage, read_write> output: array<u32>;

// --- direct composite helpers (mirror CPU composite.rs logic) ---

fn direct_composite_base(out_idx: u32, color: vec4<f32>, srca: f32) {
    let dst = unpack_rgba8(output[out_idx]);
    let out_a = srca + dst.a * (1.0 - srca);
    if out_a < 0.001 { return; }
    let inv_out_a = 1.0 / out_a;
    let result = vec4<f32>(
        fma(dst.r, 1.0 - srca, color.r) * inv_out_a,
        fma(dst.g, 1.0 - srca, color.g) * inv_out_a,
        fma(dst.b, 1.0 - srca, color.b) * inv_out_a,
        out_a,
    );
    output[out_idx] = pack_rgba8(result);
}

/// Max-alpha composite for outline samples. If destination alpha is already
/// >= source alpha, skip. Prevents additive accumulation.
fn direct_composite_max(out_idx: u32, color: vec4<f32>, srca: f32) {
    let dst_a = f32(output[out_idx] >> 24u) / 255.0;
    if dst_a >= srca { return; }

    let result = vec4<f32>(
        color.r * srca,
        color.g * srca,
        color.b * srca,
        srca,
    );
    output[out_idx] = pack_rgba8(result);
}

// --- main ---

@compute @workgroup_size(8, 8)
fn main(
    @builtin(workgroup_id) wgid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
) {
    let gi = wgid.x;
    if gi >= u.glyph_count { return; }
    let g = glyphs[gi];

    let lx = lid.x;
    let ly = lid.y;
    if lx >= g.bitmap_w || ly >= g.bitmap_h { return; }

    let bm_idx = g.glyph_offset + ly * g.bitmap_w + lx;
    // Unpack u8 from u32 array (4 bytes per u32, little-endian)
    let word_idx = bm_idx / 4u;
    let byte_in_word = bm_idx % 4u;
    let word = bitmaps[word_idx];
    let shift = byte_in_word * 8u;
    let alpha_u8 = (word >> shift) & 0xFFu;
    if alpha_u8 == 0u { return; }

    let coverage = f32(alpha_u8) / 255.0;
    let has_fill = (g.flags & 1u) != 0u;
    let has_outline = (g.flags & 2u) != 0u;
    let has_shadow = (g.flags & 4u) != 0u;

    let fill_c = unpack_rgba8(g.fill_color);
    let outline_c = unpack_rgba8(g.outline_color);
    let shadow_c = unpack_rgba8(g.shadow_color);

    let out_x = g.pos_x + i32(lx);
    let out_y = g.pos_y + i32(ly);
    let out_idx = u32(out_y) * u.output_width + u32(out_x);
    let in_bounds = out_x >= 0 && out_y >= 0
        && out_x < i32(u.output_width) && out_y < i32(u.output_height);

    // Fill pass
    if has_fill && in_bounds {
        direct_composite_base(out_idx, fill_c, coverage * fill_c.a);
    }

    // Shadow pass
    if has_shadow && shadow_c.a > 0.0 {
        let sx = out_x + i32(round(g.shadow_dx));
        let sy = out_y + i32(round(g.shadow_dy));
        if sx >= 0 && sy >= 0 && sx < i32(u.output_width) && sy < i32(u.output_height) {
            let sidx = u32(sy) * u.output_width + u32(sx);
            let srca = coverage * shadow_c.a;
            direct_composite_base(sidx, shadow_c, srca);
        }
    }

    // Outline pass — expand glyph pixels by circular radius
    if has_outline && outline_c.a > 0.0 {
        let r2 = f32(g.outline_radius * g.outline_radius);
        let srca = coverage * outline_c.a;
        for (var dy = -g.outline_radius; dy <= g.outline_radius; dy++) {
            for (var dx = -g.outline_radius; dx <= g.outline_radius; dx++) {
                if f32(dx * dx + dy * dy) > r2 { continue; }
                let ox = out_x + dx;
                let oy = out_y + dy;
                if ox < 0 || oy < 0 || ox >= i32(u.output_width) || oy >= i32(u.output_height) { continue; }
                let oidx = u32(oy) * u.output_width + u32(ox);
                direct_composite_max(oidx, outline_c, srca);
            }
        }
    }
}
