// MIDI Display compute shader — piano-roll visualization
// shared.wgsl provides: unpack_rgba8, pack_rgba8

struct Uniforms {
    dst_w: u32,
    dst_h: u32,
    note_count: u32,
    bg_r: f32, bg_g: f32, bg_b: f32, bg_a: f32,
    keyboard_start: i32,
    keyboard_size: i32,
    orientation: u32,  // 0=horizontal, 1=vertical
    key_range_min: i32,
    key_range_max: i32,
    pixels_per_key: f32,
    indicator_pos: i32,
    time_axis_offset: i32,
    _pad: u32,
}

struct NoteGpu {
    x: i32, y: i32, w: i32, h: i32,
    corner_radius: f32,
    border_thickness: f32,
    fill_r: f32, fill_g: f32, fill_b: f32, fill_a: f32,
    border_r: f32, border_g: f32, border_b: f32, border_a: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> notes: array<NoteGpu>;
@group(0) @binding(2) var<storage, read_write> dst: array<u32>;

const WHITE_KEY_INDICES: array<u32, 7> = array<u32, 7>(0u, 2u, 4u, 5u, 7u, 9u, 11u);

fn is_white_key(key: u32) -> bool {
    let k = key % 12u;
    for (var i = 0u; i < 7u; i += 1u) {
        if k == WHITE_KEY_INDICES[i] { return true; }
    }
    return false;
}

fn draw_keyboard_horizontal(px: i32, py: i32, out_color: ptr<function, vec4<f32>>) {
    let kw = uniforms.keyboard_size;
    if px >= uniforms.keyboard_start + kw { return; }
    // px is in keyboard area
    let key_count = uniforms.key_range_max - uniforms.key_range_min + 1;
    let ppk = max(uniforms.pixels_per_key, 1.0);
    let key_idx_f = f32(i32(uniforms.dst_h) - 1 - py) / ppk;
    let key_idx = i32(floor(key_idx_f));
    if key_idx < 0 || key_idx >= key_count { return; }
    let key = u32(uniforms.key_range_min + key_idx);
    if key > 127u { return; }

    let key_y0 = i32(f32(key_count - 1 - key_idx) * ppk);
    let key_y1 = key_y0 + i32(ceil(ppk));

    if is_white_key(key) {
        // White key: fill with darker border between keys
        if py >= key_y0 + 1 {
            *out_color = vec4<f32>(1.0, 1.0, 1.0, 0.9);
        } else {
            *out_color = vec4<f32>(0.5, 0.5, 0.5, 0.3);
        }
    } else {
        // Black key: shorter and narrower
        let bkh = max(i32(f32(key_y1 - key_y0) * 0.6), 1);
        let bky0 = key_y0 + ((key_y1 - key_y0) - bkh) / 2;
        let bkw = max(i32(f32(kw) * 0.6), 1);
        if py >= bky0 && py < bky0 + bkh && px < uniforms.keyboard_start + bkw {
            *out_color = vec4<f32>(0.15, 0.15, 0.15, 0.9);
        }
    }
}

fn draw_keyboard_vertical(px: i32, py: i32, out_color: ptr<function, vec4<f32>>) {
    let kh = uniforms.keyboard_size;
    if py >= uniforms.keyboard_start + kh { return; }
    let key_count = uniforms.key_range_max - uniforms.key_range_min + 1;
    let ppk = max(uniforms.pixels_per_key, 1.0);
    let key_idx = i32(floor(f32(px) / ppk));
    if key_idx < 0 || key_idx >= key_count { return; }
    let key = u32(uniforms.key_range_min + key_idx);
    if key > 127u { return; }

    let key_x0 = i32(f32(key_idx) * ppk);
    let key_x1 = key_x0 + i32(ceil(ppk));

    if is_white_key(key) {
        if px >= key_x0 + 1 {
            *out_color = vec4<f32>(1.0, 1.0, 1.0, 0.9);
        } else {
            *out_color = vec4<f32>(0.5, 0.5, 0.5, 0.3);
        }
    } else {
        let bkw = max(i32(f32(key_x1 - key_x0) * 0.6), 1);
        let bkx0 = key_x0 + ((key_x1 - key_x0) - bkw) / 2;
        let bkh = max(i32(f32(kh) * 0.6), 1);
        if px >= bkx0 && px < bkx0 + bkw && py < uniforms.keyboard_start + bkh {
            *out_color = vec4<f32>(0.15, 0.15, 0.15, 0.9);
        }
    }
}

fn rounded_rect_test(px: i32, py: i32, n: NoteGpu) -> vec2<bool> {
    // Quick AABB test
    if px < n.x || px >= n.x + n.w || py < n.y || py >= n.y + n.h {
        return vec2<bool>(false, false);
    }

    let cr = i32(round(n.corner_radius));
    let bt = i32(round(n.border_thickness));

    // Inner bounding box
    let is_fill = px >= n.x + bt && px < n.x + n.w - bt
               && py >= n.y + bt && py < n.y + n.h - bt;

    // If no rounding or pixel is in the inner box (away from corners), return directly
    if cr <= 0 {
        return vec2<bool>(is_fill, true); // true = inside border area
    }

    // Determine corner region
    let in_corner = (px < n.x + cr || px >= n.x + n.w - cr)
                 && (py < n.y + cr || py >= n.y + n.h - cr);

    if !in_corner {
        // In edge region (linear), rounding doesn't affect
        return vec2<bool>(is_fill, true);
    }

    // Corner: compute distance from nearest corner center
    let cx: i32;
    let cy: i32;
    if px < n.x + cr { cx = n.x + cr - 1; } else { cx = n.x + n.w - cr; }
    if py < n.y + cr { cy = n.y + cr - 1; } else { cy = n.y + n.h - cr; }

    let dx = f32(px - cx);
    let dy = f32(py - cy);
    let dist_sq = dx * dx + dy * dy;

    let outer_thresh = n.corner_radius + 0.5;
    let outer_ok = dist_sq <= outer_thresh * outer_thresh;

    let inner_thresh = max(n.corner_radius - n.border_thickness, 0.0) + 0.5;
    let inner_ok = inner_thresh > 0.0 && dist_sq <= inner_thresh * inner_thresh;

    return vec2<bool>(inner_ok, outer_ok);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let px = i32(gid.x);
    let py = i32(gid.y);
    if gid.x >= uniforms.dst_w || gid.y >= uniforms.dst_h {
        return;
    }

    var color = vec4<f32>(uniforms.bg_r, uniforms.bg_g, uniforms.bg_b, uniforms.bg_a);

    // Keyboard area
    if uniforms.keyboard_size > 0 {
        if uniforms.orientation == 0u {
            draw_keyboard_horizontal(px, py, &color);
        } else {
            draw_keyboard_vertical(px, py, &color);
        }
    }

    // Time axis area: check indicator line first
    let in_time_axis = (uniforms.orientation == 0u && px >= uniforms.time_axis_offset)
                    || (uniforms.orientation == 1u && py >= uniforms.time_axis_offset);

    if in_time_axis {
        let on_indicator = (uniforms.orientation == 0u && px == uniforms.indicator_pos)
                        || (uniforms.orientation == 1u && py == uniforms.indicator_pos);
        if on_indicator {
            // Blend white indicator line
            let ind_alpha = 0.6;
            color.r = color.r * (1.0 - ind_alpha) + 1.0 * ind_alpha;
            color.g = color.g * (1.0 - ind_alpha) + 1.0 * ind_alpha;
            color.b = color.b * (1.0 - ind_alpha) + 1.0 * ind_alpha;
            // alpha unchanged (indicator is opaque where drawn)
        }

        // Iterate notes back to front (last = topmost)
        for (var i = uniforms.note_count; i > 0u; i -= 1u) {
            let n = notes[i - 1u];
            let test = rounded_rect_test(px, py, n);
            if test.y {  // inside border area
                var note_color: vec4<f32>;
                if test.x {  // inside fill area
                    note_color = vec4<f32>(n.fill_r, n.fill_g, n.fill_b, n.fill_a);
                } else {  // border ring
                    note_color = vec4<f32>(n.border_r, n.border_g, n.border_b, n.border_a);
                }
                // Source-over blend
                let src_a = clamp(note_color.a, 0.0, 1.0);
                let out_a = src_a + color.a * (1.0 - src_a);
                if out_a > 0.001 {
                    color.r = (note_color.r * src_a + color.r * color.a * (1.0 - src_a)) / out_a;
                    color.g = (note_color.g * src_a + color.g * color.a * (1.0 - src_a)) / out_a;
                    color.b = (note_color.b * src_a + color.b * color.a * (1.0 - src_a)) / out_a;
                    color.a = out_a;
                }
                break;
            }
        }
    }

    let idx = py * i32(uniforms.dst_w) + px;
    dst[u32(idx)] = pack_rgba8(color);
}
