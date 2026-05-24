//! Main ASS subtitle rendering pipeline.

use std::sync::Arc;

use ab_glyph::Font;

use crate::settings::ass_subtitle::AssBlendMode;

use super::cache::RenderCache;
use super::composite::{cpu_composite_dirty_rect, direct_composite, direct_composite_max};
use super::font::FontCache;
use super::outline::get_outline_offsets_cached;
use super::parser::{alignment_to_anchor, anchor_to_base_x, anchor_to_base_y, parse_tag_segments};
use super::transform::apply_transforms;
use super::types::*;

// ---------------------------------------------------------------------------
// Public render entry point
// ---------------------------------------------------------------------------

/// Render active subtitles onto an RGBA8 output buffer.
pub fn render_ass_subtitle_frame(
    ass_script: &AssScript,
    time_ms: i64,
    font_cache: &mut FontCache,
    scale: f32,
    position_x: f32,
    position_y: f32,
    font_scale_x: f32,
    font_scale_y: f32,
    blend_mode: AssBlendMode,
    font_override: Option<&str>,
    use_native_size: bool,
    output: &mut [u8],
    output_width: usize,
    output_height: usize,
    cache: &mut RenderCache,
) -> RenderStats {
    let active = active_events(ass_script, time_ms);
    let mut stats = RenderStats {
        events_total: ass_script.events.len(),
        events_active: active.len(),
        ..Default::default()
    };
    if active.is_empty() {
        return stats;
    }

    // Compute PlayRes-to-output scaling factors.
    let res_scale_x = if use_native_size {
        ass_script
            .play_res_x
            .map_or(1.0, |prx| if prx > 0 { output_width as f32 / prx as f32 } else { 1.0 })
    } else {
        1.0
    };
    let res_scale_y = if use_native_size {
        ass_script
            .play_res_y
            .map_or(1.0, |pry| if pry > 0 { output_height as f32 / pry as f32 } else { 1.0 })
    } else {
        1.0
    };

    // Reuse/reallocate temp_buf
    let buf_size = output_width * output_height * 4;
    if cache.temp_buf.len() != buf_size {
        cache.temp_buf.resize(buf_size, 0);
    }

    // Clear previous frame's dirty region (or full clear on first frame)
    if cache.first_frame {
        cache.temp_buf.fill(0);
        cache.first_frame = false;
    } else {
        let prev = cache.prev_dirty;
        if prev.min_x < prev.max_x && prev.min_y < prev.max_y {
            let w = output_width;
            for py in prev.min_y.max(0)..=prev.max_y.min(output_height as i32 - 1) {
                let row_start = py as usize * w * 4 + prev.min_x.max(0) as usize * 4;
                let row_end =
                    py as usize * w * 4 + (prev.max_x.min(output_width as i32 - 1) as usize + 1) * 4;
                cache.temp_buf[row_start..row_end.min(buf_size)].fill(0);
            }
        }
    }
    cache.prev_dirty = DirtyRect::default();

    let mut new_dirty = DirtyRect {
        min_x: output_width as i32,
        min_y: output_height as i32,
        max_x: 0,
        max_y: 0,
    };

    let temp_buf = &mut cache.temp_buf[..buf_size];
    let cache_gen = cache.generation;
    let event_cache = &mut cache.event_cache;
    let font_data_cache = &mut cache.font_data_cache;
    let glyph_cache = &mut cache.glyph_cache;
    let outline_offsets = &mut cache.outline_offsets;

    for ev in &active {
        let style = resolve_style(ass_script, ev);

        // --- Cached event data ---
        // Key: (generation, pointer_addr) — pointer address is stable for the
        // lifetime of the AssScript and discriminating across different events.
        let ev_ptr = *ev as *const OwnedEvent as usize;
        let ev_key = (cache_gen, ev_ptr);

        let cached_ev = event_cache.entry(ev_key).or_insert_with(|| {
            let segments = parse_tag_segments(&ev.text);
            let clean_text: String = segments.iter().map(|s| s.text.as_str()).collect();
            let text_normalized = clean_text.replace("\\n", "\\N");

            // Build char_to_seg mapping
            let mut char_to_seg: Vec<usize> = Vec::with_capacity(clean_text.len());
            for (si, seg) in segments.iter().enumerate() {
                for _ in seg.text.chars() {
                    char_to_seg.push(si);
                }
            }

            // Build merged_base_tags by sequentially walking all segments.
            // This correctly models ASS's cumulative tag semantics: later tags
            // override earlier ones. We also handle named \r resets by resolving
            // the referenced style.
            let merged_base_tags =
                build_merged_base_tags(&segments, &style, ass_script);

            CachedEventData {
                segments,
                clean_text,
                text_normalized,
                char_to_seg,
                merged_base_tags,
            }
        });

        let segments = &cached_ev.segments;
        let clean_text: &str = &cached_ev.clean_text;
        let text_normalized: &str = &cached_ev.text_normalized;
        let char_to_seg = &cached_ev.char_to_seg;
        let merged_base_tags = &cached_ev.merged_base_tags;

        // Apply \t transforms on top of the merged base
        let inline_tags = apply_transforms(
            time_ms,
            ev.start_ms,
            ev.end_ms,
            &merged_base_tags.transforms,
            merged_base_tags,
        );

        // Merge inline tags with style defaults
        let base_fontname = inline_tags
            .fontname
            .as_deref()
            .unwrap_or(&style.fontname);
        let fontname = font_override
            .filter(|s| !s.is_empty())
            .and_then(|ov| {
                if font_cache.find_font(ov).is_some() {
                    Some(ov)
                } else {
                    None
                }
            })
            .unwrap_or(base_fontname);

        let fontsize = inline_tags.fontsize.unwrap_or(style.fontsize);
        let fill_color = inline_tags.primary_color.unwrap_or(style.primary_color);
        let alignment = inline_tags.alignment.unwrap_or(style.alignment);

        let outline_w =
            (inline_tags.xbord.or(inline_tags.bord).unwrap_or(style.outline)) * res_scale_y;
        let _outline_h =
            (inline_tags.ybord.or(inline_tags.bord).unwrap_or(style.outline)) * res_scale_y;
        let shadow_dx =
            (inline_tags.xshad.or(inline_tags.shad).unwrap_or(style.shadow)) * res_scale_x;
        let shadow_dy =
            (inline_tags.yshad.or(inline_tags.shad).unwrap_or(style.shadow)) * res_scale_y;

        let blur_radius = inline_tags.blur.or(inline_tags.be).unwrap_or(0.0) * res_scale_y;
        stats.max_blur_radius = stats.max_blur_radius.max(blur_radius);

        // Clip with precomputed bounding box
        let clip_check: Option<ClipCheck> = inline_tags.clip.as_ref().and_then(|clip| {
            let pts = &clip.points;
            if pts.len() < 4 {
                return None;
            }
            let (mut x1, mut y1) = (f32::MAX, f32::MAX);
            let (mut x2, mut y2) = (f32::MIN, f32::MIN);
            for p in pts {
                let cx = p.0 * res_scale_x * scale + (position_x - 0.5) * output_width as f32;
                let cy = p.1 * res_scale_y * scale + (position_y - 0.5) * output_height as f32;
                x1 = x1.min(cx);
                y1 = y1.min(cy);
                x2 = x2.max(cx);
                y2 = y2.max(cy);
            }
            Some(ClipCheck {
                x1,
                y1,
                x2,
                y2,
                inverse: clip.inverse,
            })
        });

        let outline_color = inline_tags.outline_color.unwrap_or(style.outline_color);
        let shadow_color = inline_tags.back_color.unwrap_or(style.back_color);
        let border_style = style.border_style;

        let tag_sx = inline_tags.scale_x.unwrap_or(style.scale_x) / 100.0;
        let tag_sy = inline_tags.scale_y.unwrap_or(style.scale_y) / 100.0;

        // Fade alpha combined with global \alpha tag
        let fade_alpha = compute_fade_alpha(time_ms, ev.start_ms, ev.end_ms, &inline_tags.fade);
        let global_alpha = inline_tags.alpha.unwrap_or(1.0);
        let effective_alpha = fade_alpha * global_alpha;

        // Movement
        let move_pos = compute_move_pos(time_ms, ev.start_ms, ev.end_ms, &inline_tags.move_);

        stats.text_char_count = clean_text.chars().count();

        // --- Font lookup (cached) ---
        let font_data = if let Some(cached) = font_data_cache.get(&ev_key) {
            stats.font_found = cached.is_some();
            cached.clone()
        } else {
            let text_chars: Vec<char> =
                clean_text.chars().filter(|c| !c.is_whitespace()).collect();
            let result = font_cache
                .find_font_for_chars(&text_chars, fontname)
                .or_else(|| font_cache.find_font(fontname));
            stats.font_found = result.is_some();
            font_data_cache.insert(ev_key, result.clone());
            result
        };
        let Some(font_data) = font_data else {
            continue;
        };

        let Ok(font) = ab_glyph::FontRef::try_from_slice(&*font_data) else {
            continue;
        };
        stats.font_parsed = true;

        let px_scale_x = fontsize * res_scale_y * font_scale_x.max(0.01) * tag_sx * scale;
        let px_scale_y = fontsize * res_scale_y * font_scale_y.max(0.01) * tag_sy * scale;
        let px_scale = ab_glyph::PxScale {
            x: px_scale_x,
            y: px_scale_y,
        };
        let units_per_em = font.units_per_em().unwrap_or(1000.0);

        let ascent = font.ascent_unscaled() * px_scale_y / units_per_em;
        let descent = font.descent_unscaled() * px_scale_y / units_per_em;
        let line_gap = font.line_gap_unscaled() * px_scale_y / units_per_em;
        let line_height = ascent - descent + line_gap;

        let spacing_raw = inline_tags.spacing.unwrap_or(style.spacing);
        let spacing_adv = spacing_raw * res_scale_y * px_scale.x / units_per_em;

        let lines: Vec<&str> = text_normalized.split("\\N").collect();

        // ------------------------------------------------------------------
        // Phase 1: Layout — compute line widths, store per-char glyph info
        // ------------------------------------------------------------------
        struct CharLayout {
            glyph_id: ab_glyph::GlyphId,
            h_adv_px: f32,
        }
        struct LineLayout {
            chars: Vec<CharLayout>,
            width_before_scale: f32,
        }

        let mut max_line_width: f32 = 0.0;
        let mut total_height: f32 = 0.0;
        let mut line_layouts: Vec<LineLayout> = Vec::new();
        let mut global_ci = 0usize;

        for line in &lines {
            let mut line_w: f32 = 0.0;
            let mut char_count = 0usize;
            let mut chars = Vec::new();

            for c in line.chars() {
                let glyph_id = font.glyph_id(c);
                let mut glyph_w =
                    font.h_advance_unscaled(glyph_id) * px_scale.x / units_per_em;
                // Apply per-char bold scale to layout width
                if let Some(seg) = char_to_seg.get(global_ci).and_then(|&si| segments.get(si)) {
                    if seg.tags.bold.unwrap_or(style.bold) {
                        glyph_w *= 1.15;
                    }
                }
                chars.push(CharLayout {
                    glyph_id,
                    h_adv_px: glyph_w,
                });
                line_w += glyph_w;
                char_count += 1;
                global_ci += 1;
            }

            if char_count > 0 {
                line_w += spacing_raw * res_scale_y * px_scale.x / units_per_em
                    * (char_count - 1) as f32;
            }
            max_line_width = max_line_width.max(line_w);
            total_height += line_height;
            line_layouts.push(LineLayout {
                chars,
                width_before_scale: line_w,
            });
        }

        let text_w = max_line_width;
        let text_h = total_height;

        let (align_x, align_y) = alignment_to_anchor(alignment);

        let margin_l = (if ev.margin_l != 0 {
            ev.margin_l
        } else {
            style.margin_l
        }) as f32
            * scale
            * res_scale_x;
        let margin_r = (if ev.margin_r != 0 {
            ev.margin_r
        } else {
            style.margin_r
        }) as f32
            * scale
            * res_scale_x;
        let margin_v = (if ev.margin_v != 0 {
            ev.margin_v
        } else {
            style.margin_v
        }) as f32
            * scale
            * res_scale_y;

        let w = output_width as f32;
        let h = output_height as f32;

        let base_x = match align_x {
            0 => margin_l,
            1 => (w - text_w) / 2.0,
            2 => w - text_w - margin_r,
            _ => (w - text_w) / 2.0,
        } + (position_x - 0.5) * w;

        let base_y = match align_y {
            0 => h - margin_v - text_h,
            1 => (h - text_h) / 2.0,
            2 => margin_v,
            _ => h - margin_v - text_h,
        } + (position_y - 0.5) * h;

        // Apply \pos or \move override
        let (base_x, base_y) = if let Some((px, py)) = inline_tags.pos {
            let anchor_x = px * scale * res_scale_x + (position_x - 0.5) * w;
            let anchor_y = py * scale * res_scale_y + (position_y - 0.5) * h;
            (
                anchor_to_base_x(align_x, anchor_x, text_w),
                anchor_to_base_y(align_y, anchor_y, text_h),
            )
        } else if let Some((mx, my)) = move_pos {
            let anchor_x = mx * scale * res_scale_x + (position_x - 0.5) * w;
            let anchor_y = my * scale * res_scale_y + (position_y - 0.5) * h;
            (
                anchor_to_base_x(align_x, anchor_x, text_w),
                anchor_to_base_y(align_y, anchor_y, text_h),
            )
        } else {
            (base_x, base_y)
        };

        // Precompute outline offset directions (cached, bounded)
        let outline_offsets_vec: Arc<[(f32, f32)]> = if outline_w > 0.0 && border_style == 1 {
            get_outline_offsets_cached(outline_w, blur_radius, outline_offsets)
        } else {
            Arc::new([])
        };

        // ------------------------------------------------------------------
        // Phase 2: Glyph rendering with coverage cache
        // ------------------------------------------------------------------
        let font_ptr = Arc::as_ptr(&font_data) as usize;
        let first_baseline_y = base_y + ascent;
        let mut cursor_y = first_baseline_y;
        let mut char_offset = 0usize;

        for (_line, layout) in lines.iter().zip(line_layouts.iter()) {
            if cursor_y + line_height <= 0.0 || cursor_y >= h {
                cursor_y += line_height;
                char_offset += layout.chars.len();
                continue;
            }

            let line_x = match align_x {
                0 => base_x,
                1 => base_x + (text_w - layout.width_before_scale) / 2.0,
                2 => base_x + text_w - layout.width_before_scale,
                _ => base_x,
            };

            let mut cursor_x = line_x;
            for (char_idx, ch_layout) in layout.chars.iter().enumerate() {
                let global_char_idx = char_offset + char_idx;

                let seg_tags = if global_char_idx < char_to_seg.len() {
                    &segments[char_to_seg[global_char_idx]].tags
                } else {
                    &inline_tags
                };
                let ch_fill_color = seg_tags.primary_color.unwrap_or(fill_color);
                let ch_outline_color = seg_tags.outline_color.unwrap_or(outline_color);
                let ch_bold = seg_tags.bold.unwrap_or(style.bold);
                let ch_italic = seg_tags.italic.unwrap_or(style.italic);

                let use_glyph_id = ch_layout.glyph_id;
                let h_adv_px = ch_layout.h_adv_px;
                let sb_px = font.h_side_bearing_unscaled(use_glyph_id) * px_scale.x / units_per_em;
                let bold_scale = if ch_bold { 1.15 } else { 1.0 };
                let glyph_x = cursor_x + sb_px * bold_scale;
                let italic_shear: f32 = if ch_italic { 0.25 } else { 0.0 };

                if glyph_x + h_adv_px * bold_scale >= 0.0 && glyph_x < w {
                    let use_scale = ab_glyph::PxScale {
                        x: px_scale.x * bold_scale,
                        y: px_scale.y,
                    };
                    stats.glyph_rasterize_attempts += 1;

                    let cache_key = GlyphCacheKey {
                        font_ptr,
                        glyph_id: use_glyph_id.0,
                        scale_x: (use_scale.x * 1000.0) as u32,
                        scale_y: (use_scale.y * 1000.0) as u32,
                        bold_x: (bold_scale * 1000.0) as u32,
                    };

                    if !glyph_cache.contains_key(&cache_key) {
                        // Evict when too large
                        const MAX_GLYPH_CACHE: usize = 2048;
                        if glyph_cache.len() >= MAX_GLYPH_CACHE {
                            glyph_cache.clear();
                        }
                        let glyph = ab_glyph::Glyph {
                            id: use_glyph_id,
                            scale: use_scale,
                            position: ab_glyph::point(0.0, 0.0),
                        };
                        if let Some(outline) = font.outline_glyph(glyph) {
                            let px_bounds = outline.px_bounds();
                            let mut coverage = Vec::new();
                            outline.draw(|gx, gy, cov| {
                                coverage.push((gx, gy, cov));
                            });
                            glyph_cache.insert(
                                cache_key.clone(),
                                CachedGlyph {
                                    px_bounds_min_x: px_bounds.min.x,
                                    px_bounds_min_y: px_bounds.min.y,
                                    coverage,
                                },
                            );
                        }
                    }

                    if let Some(cached) = glyph_cache.get(&cache_key) {
                        stats.glyph_rasterize_ok += 1;
                        let mut local_pixels = 0usize;
                        let slant_offset = |gy: f32| -> f32 { gy * italic_shear };

                        // Shadow pass
                        if shadow_dx != 0.0 || shadow_dy != 0.0 {
                            let sh_color = apply_fade_to_color(shadow_color, effective_alpha);
                            for &(gx, gy, cov) in &cached.coverage {
                                let px = (glyph_x
                                    + (cached.px_bounds_min_x + gx as f32
                                        + slant_offset(gy as f32))
                                    + shadow_dx * scale)
                                    .round() as i32;
                                let py = (cursor_y
                                    + (cached.px_bounds_min_y + gy as f32)
                                    + shadow_dy * scale)
                                    .round() as i32;
                                if px < 0
                                    || py < 0
                                    || px >= output_width as i32
                                    || py >= output_height as i32
                                {
                                    continue;
                                }
                                if clip_reject(px, py, &clip_check) {
                                    continue;
                                }
                                let idx = (py as usize * output_width + px as usize) * 4;
                                direct_composite(temp_buf, idx, sh_color, cov);
                                new_dirty.expand(px, py, 3);
                            }
                        }

                        // Outline passes — use max blending so overlapping
                        // offset stamps don't accumulate alpha additively.
                        if !outline_offsets_vec.is_empty() {
                            let out_color = apply_fade_to_color(ch_outline_color, effective_alpha);
                            for &(ox, oy) in outline_offsets_vec.iter() {
                                for &(gx, gy, cov) in &cached.coverage {
                                    let px = (glyph_x
                                        + (cached.px_bounds_min_x + gx as f32
                                            + slant_offset(gy as f32))
                                        + ox * scale)
                                        .round() as i32;
                                    let py = (cursor_y
                                        + (cached.px_bounds_min_y + gy as f32)
                                        + oy * scale)
                                        .round() as i32;
                                    if px < 0
                                        || py < 0
                                        || px >= output_width as i32
                                        || py >= output_height as i32
                                    {
                                        continue;
                                    }
                                    if clip_reject(px, py, &clip_check) {
                                        continue;
                                    }
                                    let idx =
                                        (py as usize * output_width + px as usize) * 4;
                                    direct_composite_max(temp_buf, idx, out_color, cov);
                                    new_dirty.expand(px, py, 3);
                                }
                            }
                        }

                        // Fill pass
                        let fill = apply_fade_to_color(ch_fill_color, effective_alpha);
                        for &(gx, gy, cov) in &cached.coverage {
                            local_pixels += 1;
                            let px = (glyph_x
                                + (cached.px_bounds_min_x + gx as f32
                                    + slant_offset(gy as f32)))
                                .round() as i32;
                            let py = (cursor_y + (cached.px_bounds_min_y + gy as f32))
                                .round() as i32;
                            if px < 0
                                || py < 0
                                || px >= output_width as i32
                                || py >= output_height as i32
                            {
                                continue;
                            }
                            if clip_reject(px, py, &clip_check) {
                                continue;
                            }
                            let idx = (py as usize * output_width + px as usize) * 4;
                            direct_composite(temp_buf, idx, fill, cov);
                            new_dirty.expand(px, py, 3);
                        }

                        stats.pixels_written += local_pixels;
                    }
                }
                cursor_x += (h_adv_px + spacing_adv) * bold_scale;
            }

            char_offset += layout.chars.len();
            cursor_y += line_height;
        }
    }

    // ------------------------------------------------------------------
    // Phase 3: CPU composite dirty rect onto output
    // ------------------------------------------------------------------
    if let Some(dr) = new_dirty.clamp(output_width as i32, output_height as i32) {
        cpu_composite_dirty_rect(temp_buf, output, output_width, &dr, blend_mode);
    }
    cache.prev_dirty = new_dirty;

    stats
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Return active events at a given time, sorted by layer (higher = later = on top).
fn active_events(ass_script: &AssScript, time_ms: i64) -> Vec<&OwnedEvent> {
    let mut events: Vec<&OwnedEvent> = ass_script
        .events
        .iter()
        .filter(|e| e.start_ms <= time_ms && time_ms < e.end_ms)
        .collect();
    // Sort by layer so higher-layer events render on top.
    // Within the same layer, preserve file order (stable sort).
    events.sort_by_key(|e| e.layer);
    events
}

/// Resolve the style for a dialogue event.
fn resolve_style<'a>(ass_script: &'a AssScript, ev: &OwnedEvent) -> &'a OwnedStyle {
    static DEFAULT_STYLE: std::sync::OnceLock<OwnedStyle> = std::sync::OnceLock::new();
    ass_script
        .styles
        .iter()
        .find(|s| s.name == ev.style_name)
        .or_else(|| ass_script.styles.first())
        .unwrap_or_else(|| DEFAULT_STYLE.get_or_init(OwnedStyle::default))
}

/// Build merged base tags by sequentially walking all segments.
/// This correctly models ASS's cumulative tag semantics: later tags override
/// earlier ones. Handles named `\r` resets by resolving the referenced style.
fn build_merged_base_tags(
    segments: &[TagSegment],
    default_style: &OwnedStyle,
    ass_script: &AssScript,
) -> ParsedTags {
    let mut merged = ParsedTags::default();

    for seg in segments {
        let tags = &seg.tags;

        // Handle \r reset
        if tags.reset {
            if let Some(ref style_name) = tags.reset_style {
                // Named \r: resolve the style and use its values as the new base
                let resolved = ass_script
                    .styles
                    .iter()
                    .find(|s| s.name == *style_name)
                    .unwrap_or(default_style);
                merged.fontname = Some(resolved.fontname.clone());
                merged.fontsize = Some(resolved.fontsize);
                merged.primary_color = Some(resolved.primary_color);
                merged.secondary_color = Some(resolved.secondary_color);
                merged.outline_color = Some(resolved.outline_color);
                merged.back_color = Some(resolved.back_color);
                merged.bold = Some(resolved.bold);
                merged.italic = Some(resolved.italic);
                merged.underline = Some(resolved.underline);
                merged.strikeout = Some(resolved.strikeout);
                merged.scale_x = Some(resolved.scale_x);
                merged.scale_y = Some(resolved.scale_y);
                merged.spacing = Some(resolved.spacing);
                merged.alignment = Some(resolved.alignment);
                merged.bord = Some(resolved.outline);
                merged.xbord = Some(resolved.outline);
                merged.ybord = Some(resolved.outline);
                merged.shad = Some(resolved.shadow);
                merged.xshad = Some(resolved.shadow);
                merged.yshad = Some(resolved.shadow);
                // Clear position/animation overrides from previous tags
                merged.pos = None;
                merged.org = None;
                merged.move_ = None;
                merged.clip = None;
                merged.fade = None;
                merged.karaoke = None;
                merged.frz = None;
                merged.frx = None;
                merged.fry = None;
                merged.fax = None;
                merged.fay = None;
                merged.be = None;
                merged.blur = None;
                merged.alpha = None;
            } else {
                // Bare \r: reset to defaults (clear all overrides)
                // We already handled this in parse_tag_segments, but
                // double-check here
            }
        }

        // Apply this segment's tags on top of accumulated state
        apply_tags_on_top(&mut merged, tags);
    }

    // Fill in gaps from the default style
    merged.fontsize = merged.fontsize.or(Some(default_style.fontsize));
    merged.primary_color = merged.primary_color.or(Some(default_style.primary_color));
    merged.secondary_color = merged.secondary_color.or(Some(default_style.secondary_color));
    merged.outline_color = merged.outline_color.or(Some(default_style.outline_color));
    merged.back_color = merged.back_color.or(Some(default_style.back_color));
    merged.scale_x = merged.scale_x.or(Some(default_style.scale_x));
    merged.scale_y = merged.scale_y.or(Some(default_style.scale_y));
    merged.spacing = merged.spacing.or(Some(default_style.spacing));
    merged.bord = merged.bord.or(Some(default_style.outline));
    merged.xbord = merged.xbord.or(Some(default_style.outline));
    merged.ybord = merged.ybord.or(Some(default_style.outline));
    merged.shad = merged.shad.or(Some(default_style.shadow));
    merged.xshad = merged.xshad.or(Some(default_style.shadow));
    merged.yshad = merged.yshad.or(Some(default_style.shadow));
    merged.bold = merged.bold.or(Some(default_style.bold));
    merged.italic = merged.italic.or(Some(default_style.italic));
    merged.underline = merged.underline.or(Some(default_style.underline));
    merged.strikeout = merged.strikeout.or(Some(default_style.strikeout));
    merged.alignment = merged.alignment.or(Some(default_style.alignment));

    merged
}

/// Apply a ParsedTags on top of another, only setting fields that are Some.
fn apply_tags_on_top(base: &mut ParsedTags, overlay: &ParsedTags) {
    if overlay.fontname.is_some() { base.fontname = overlay.fontname.clone(); }
    if overlay.fontsize.is_some() { base.fontsize = overlay.fontsize; }
    if overlay.bold.is_some() { base.bold = overlay.bold; }
    if overlay.italic.is_some() { base.italic = overlay.italic; }
    if overlay.underline.is_some() { base.underline = overlay.underline; }
    if overlay.strikeout.is_some() { base.strikeout = overlay.strikeout; }
    if overlay.primary_color.is_some() { base.primary_color = overlay.primary_color; }
    if overlay.secondary_color.is_some() { base.secondary_color = overlay.secondary_color; }
    if overlay.outline_color.is_some() { base.outline_color = overlay.outline_color; }
    if overlay.back_color.is_some() { base.back_color = overlay.back_color; }
    if overlay.alpha.is_some() { base.alpha = overlay.alpha; }
    if overlay.scale_x.is_some() { base.scale_x = overlay.scale_x; }
    if overlay.scale_y.is_some() { base.scale_y = overlay.scale_y; }
    if overlay.spacing.is_some() { base.spacing = overlay.spacing; }
    if overlay.alignment.is_some() { base.alignment = overlay.alignment; }
    if overlay.pos.is_some() { base.pos = overlay.pos; }
    if overlay.org.is_some() { base.org = overlay.org; }
    if overlay.move_.is_some() { base.move_ = overlay.move_.clone(); }
    if overlay.frz.is_some() { base.frz = overlay.frz; }
    if overlay.frx.is_some() { base.frx = overlay.frx; }
    if overlay.fry.is_some() { base.fry = overlay.fry; }
    if overlay.fax.is_some() { base.fax = overlay.fax; }
    if overlay.fay.is_some() { base.fay = overlay.fay; }
    if overlay.bord.is_some() { base.bord = overlay.bord; }
    if overlay.shad.is_some() { base.shad = overlay.shad; }
    if overlay.xbord.is_some() { base.xbord = overlay.xbord; }
    if overlay.ybord.is_some() { base.ybord = overlay.ybord; }
    if overlay.xshad.is_some() { base.xshad = overlay.xshad; }
    if overlay.yshad.is_some() { base.yshad = overlay.yshad; }
    if overlay.be.is_some() { base.be = overlay.be; }
    if overlay.blur.is_some() { base.blur = overlay.blur; }
    if overlay.clip.is_some() { base.clip = overlay.clip.clone(); }
    if overlay.fade.is_some() { base.fade = overlay.fade.clone(); }
    if overlay.karaoke.is_some() { base.karaoke = overlay.karaoke.clone(); }
    if overlay.drawing_scale.is_some() { base.drawing_scale = overlay.drawing_scale; }
    // Transforms accumulate
    base.transforms.extend(overlay.transforms.clone());
}

/// Check if a pixel should be rejected by clip.
#[inline]
fn clip_reject(px: i32, py: i32, clip: &Option<ClipCheck>) -> bool {
    match clip {
        Some(cc) => {
            let (px_f, py_f) = (px as f32, py as f32);
            (px_f >= cc.x1 && px_f <= cc.x2 && py_f >= cc.y1 && py_f <= cc.y2) == cc.inverse
        }
        None => false,
    }
}

/// Compute fade alpha for the current time.
fn compute_fade_alpha(
    time_ms: i64,
    ev_start: i64,
    ev_end: i64,
    fade: &Option<FadeData>,
) -> f32 {
    let fade = match fade {
        Some(f) => f,
        None => return 1.0,
    };
    let elapsed = time_ms - ev_start;
    if fade.is_complex {
        // \fade(a1,a2,a3,t1,t2,t3,t4)
        if elapsed <= fade.t1 {
            fade.a1
        } else if elapsed <= fade.t2 {
            let frac = (elapsed - fade.t1) as f32 / (fade.t2 - fade.t1).max(1) as f32;
            fade.a1 + (fade.a2 - fade.a1) * frac
        } else if elapsed <= fade.t3 {
            fade.a2
        } else if elapsed <= fade.t4 {
            let frac = (elapsed - fade.t3) as f32 / (fade.t4 - fade.t3).max(1) as f32;
            fade.a2 + (fade.a3 - fade.a2) * frac
        } else {
            fade.a3
        }
    } else {
        // \fad(t1,t2)
        let dur = ev_end - ev_start;
        let (t1, t2) = if fade.t1 + fade.t2 > dur && dur > 0 {
            let total = (fade.t1 + fade.t2).max(1) as f32;
            (
                (fade.t1 as f32 * dur as f32 / total) as i64,
                (fade.t2 as f32 * dur as f32 / total) as i64,
            )
        } else {
            (fade.t1, fade.t2)
        };
        if elapsed <= t1 {
            elapsed as f32 / t1.max(1) as f32
        } else if t2 > 0 && elapsed >= dur - t2 {
            (dur - elapsed) as f32 / t2.max(1) as f32
        } else {
            1.0
        }
    }
}

/// Compute interpolated position for `\move` animation.
fn compute_move_pos(
    time_ms: i64,
    ev_start: i64,
    ev_end: i64,
    mv: &Option<MoveAnim>,
) -> Option<(f32, f32)> {
    let mv = mv.as_ref()?;
    let t1 = mv.t1.unwrap_or(0);
    let t2 = mv.t2.unwrap_or(ev_end - ev_start);
    let elapsed = time_ms - ev_start;
    if elapsed < t1 {
        return Some((mv.x1, mv.y1));
    }
    if elapsed > t2 {
        return Some((mv.x2, mv.y2));
    }
    let dur = (t2 - t1).max(1);
    let frac = (elapsed - t1) as f32 / dur as f32;
    Some((
        mv.x1 + (mv.x2 - mv.x1) * frac,
        mv.y1 + (mv.y2 - mv.y1) * frac,
    ))
}

fn apply_fade_to_color(color: [f32; 4], fade_alpha: f32) -> [f32; 4] {
    [color[0], color[1], color[2], color[3] * fade_alpha]
}
