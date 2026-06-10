//! Main ASS subtitle rendering pipeline.
//!
//! Uses oximedia-subtitle's `TextLayoutEngine` (fontdue-backed) for text layout
//! and glyph rasterization, then composites onto an RGBA8 output buffer.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use oximedia_subtitle::font::Font as OxiFont;
use oximedia_subtitle::style::{Alignment, FontStyle, FontWeight, SubtitleStyle};
use oximedia_subtitle::text::{TextLayout, TextLayoutEngine};

use crate::settings::ass_subtitle::AssBlendMode;

use super::cache::{
    FontEngineKey, LayoutCacheKey,
    MAX_EVENT_CACHE, MAX_FONT_ENGINES, MAX_TEXT_LAYOUTS,
    RenderCache,
};
use super::composite::{cpu_composite_dirty_rect, direct_composite, direct_composite_max};
use super::font::FontCache;
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

    // When NOT using native size, center the PlayRes viewport in the output buffer.
    // The PlayRes coordinate system is rendered at 1:1 pixels, centered.
    let (center_off_x, center_off_y) = if use_native_size {
        (0.0, 0.0)
    } else {
        let prx = ass_script.play_res_x.unwrap_or(output_width as u32) as f32;
        let pry = ass_script.play_res_y.unwrap_or(output_height as u32) as f32;
        ((output_width as f32 - prx) * 0.5, (output_height as f32 - pry) * 0.5)
    };

    // Outline/shadow scale: always matches the text scaling so outlines stay
    // proportional to the glyphs they surround.
    let outline_res_scale = res_scale_y;

    // Effective viewport dimensions for alignment calculations.
    // When use_native_size=true, the entire output is the viewport.
    // When use_native_size=false, the PlayRes viewport is centered in the output.
    let viewport_w = if use_native_size { output_width as f32 } else { ass_script.play_res_x.unwrap_or(output_width as u32) as f32 };
    let viewport_h = if use_native_size { output_height as f32 } else { ass_script.play_res_y.unwrap_or(output_height as u32) as f32 };

    // Reuse/reallocate temp_buf
    let buf_size = output_width * output_height * 4;
    if cache.temp_buf.len() != buf_size {
        cache.temp_buf.resize(buf_size, 0);
    }
    // Shrink if significantly over-allocated after a resolution decrease
    if cache.temp_buf.capacity() > buf_size * 2 {
        cache.temp_buf.shrink_to(buf_size);
    }

    // Clear previous frame's dirty region (or full clear on first frame)
    if cache.first_frame {
        cache.temp_buf.fill(0);
        cache.first_frame = false;
    } else {
        let prev = cache.prev_dirty;
        // Clamp all dirty rect bounds to current output dimensions
        // (VEGAS Pro can change resolution between frames, e.g. preview vs full)
        let px1 = prev.min_x.max(0).min(output_width as i32 - 1);
        let px2 = prev.max_x.max(0).min(output_width as i32 - 1);
        let py1 = prev.min_y.max(0).min(output_height as i32 - 1);
        let py2 = prev.max_y.max(0).min(output_height as i32 - 1);
        if px1 < px2 && py1 < py2 {
            let w = output_width;
            for py in py1..=py2 {
                let row_start = py as usize * w * 4 + px1 as usize * 4;
                let row_end = py as usize * w * 4 + (px2 as usize + 1) * 4;
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

    // Only attempt GPU paths if the shared device was ALREADY initialized
    // by another effect — never trigger GPU init from the ASS subtitle renderer
    // (creating a wgpu device inside VEGAS Pro causes D3D12 conflicts).
    #[cfg(feature = "gpu")]
    let gpu_ready = crate::gpu::is_shared_device_ready();
    #[cfg(not(feature = "gpu"))]
    let gpu_ready = false;

    for ev in &active {
        let style = resolve_style(ass_script, ev);

        // --- Cached event data ---
        let ev_ptr = *ev as *const OwnedEvent as usize;
        let ev_key = (cache_gen, ev_ptr);

        RenderCache::evict_if_full(event_cache, MAX_EVENT_CACHE, &ev_key);

        let cached_ev = event_cache.entry(ev_key).or_insert_with(|| {
            let segments = parse_tag_segments(&ev.text);
            let clean_text: String = segments.iter().map(|s| s.text.as_str()).collect();
            // Convert ASS \N and \n to actual newlines for the layout engine
            let text_for_layout = clean_text.replace("\\N", "\n").replace("\\n", "\n");

            // Build char_to_seg mapping aligned with text_for_layout (not clean_text).
            // text_for_layout may differ because \N (2 chars) becomes \n (1 char).
            // We track position in clean_text to map each text_for_layout char to the
            // correct segment.
            let mut char_to_seg: Vec<usize> = Vec::with_capacity(text_for_layout.len());
            let mut ci = 0usize; // position in clean_text
            for tc in text_for_layout.chars() {
                if tc == '\n' {
                    // This newline replaced a \N (2 chars in clean_text). Skip past it.
                    ci += 2;
                    continue;
                }
                // Find which segment ci maps to by cumulative segment text lengths
                let mut seg_idx = 0usize;
                let mut cum = 0usize;
                for (si, seg) in segments.iter().enumerate() {
                    cum += seg.text.chars().count();
                    if ci < cum { seg_idx = si; break; }
                }
                char_to_seg.push(seg_idx);
                ci += 1;
            }

            // Cache non-whitespace chars for font coverage checks
            let text_chars: Vec<char> = clean_text.chars().filter(|c| !c.is_whitespace()).collect();

            // Fast hash of clean_text for layout cache keying
            let text_hash = hash_str(&clean_text);

            let merged_base_tags = build_merged_base_tags(&segments, style, ass_script);
            let karaoke_syllables = build_karaoke_syllables(&segments);

            CachedEventData {
                segments,
                clean_text,
                text_for_layout,
                char_to_seg,
                merged_base_tags,
                text_chars,
                text_hash,
                karaoke_syllables,
            }
        });

        let segments = &cached_ev.segments;
        let clean_text: &str = &cached_ev.clean_text;
        let text_for_layout: &str = &cached_ev.text_for_layout;
        let char_to_seg = &cached_ev.char_to_seg;
        let merged_base_tags = &cached_ev.merged_base_tags;
        let karaoke_syllables = &cached_ev.karaoke_syllables;

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
                if font_cache.find_font(ov, false, false).is_some() {
                    Some(ov)
                } else {
                    None
                }
            })
            .unwrap_or(base_fontname);

        let fontsize = inline_tags.fontsize.unwrap_or(style.fontsize);
        let bold = inline_tags.bold.unwrap_or(style.bold);
        let italic = inline_tags.italic.unwrap_or(style.italic);
        let fill_color = inline_tags.primary_color.unwrap_or(style.primary_color);
        let alignment = inline_tags.alignment.unwrap_or(style.alignment);

        let outline_w = inline_tags.xbord.or(inline_tags.bord).unwrap_or(style.outline) * outline_res_scale;
        let shadow_dx =
            (inline_tags.xshad.or(inline_tags.shad).unwrap_or(style.shadow)) * outline_res_scale * scale;
        let shadow_dy =
            (inline_tags.yshad.or(inline_tags.shad).unwrap_or(style.shadow)) * outline_res_scale * scale;

        let blur_radius = inline_tags.blur.or(inline_tags.be).unwrap_or(0.0) * outline_res_scale;
        stats.max_blur_radius = stats.max_blur_radius.max(blur_radius);

        // Clip with precomputed bounding box
        let clip_check: Option<ClipCheck> = inline_tags.clip.as_ref().and_then(|clip| {
            let pts = &clip.points;
            if pts.len() < 4 {
                return None;
            }
            let clip_scale = clip.scale.unwrap_or(1.0);
            let (mut x1, mut y1) = (f32::MAX, f32::MAX);
            let (mut x2, mut y2) = (f32::MIN, f32::MIN);
            for p in pts {
                let cx = p.0 * clip_scale * res_scale_x * scale + center_off_x + (position_x - 0.5) * output_width as f32;
                let cy = p.1 * clip_scale * res_scale_y * scale + center_off_y + (position_y - 0.5) * output_height as f32;
                x1 = x1.min(cx);
                y1 = y1.min(cy);
                x2 = x2.max(cx);
                y2 = y2.max(cy);
            }
            Some(ClipCheck { x1, y1, x2, y2, inverse: clip.inverse })
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

        // --- Font lookup (uses cached text_chars) ---
        let font_data = font_cache
            .find_font_for_chars(&cached_ev.text_chars, fontname, bold, italic)
            .or_else(|| font_cache.find_font(fontname, bold, italic));
        stats.font_found = font_data.is_some();
        let Some(font_data) = font_data else {
            continue;
        };

        // Compute effective pixel size
        let px_size = fontsize * res_scale_y * font_scale_y.max(0.01) * tag_sy * scale;

        // Capture pointer identity before font_data is potentially consumed
        let font_data_ptr = Arc::as_ptr(&font_data) as usize;

        // Get or create cached TextLayoutEngine
        let eng_key = FontEngineKey {
            data_ptr: font_data_ptr,
            size_bucket: (px_size * 2.0) as u32,
        };
        RenderCache::evict_if_full(&mut cache.font_engines, MAX_FONT_ENGINES, &eng_key);
        let layout_engine = match cache.font_engines.entry(eng_key) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                // unwrap_or_clone avoids cloning when Arc has a single reference
                match OxiFont::from_bytes(Arc::unwrap_or_clone(font_data)) {
                    Ok(oxi_font) => e.insert(TextLayoutEngine::new(oxi_font)),
                    Err(_) => {
                        // Corrupt system font — skip this event
                        continue;
                    }
                }
            }
        };
        stats.font_parsed = true;

        // Build oximedia SubtitleStyle for layout
        // 0 disables width-based wrapping — ASS uses \N for explicit line breaks only.
        let max_width = 0u32;

        // Layout text via oximedia — check cache when no layout-affecting transforms
        let layout_animates = has_layout_affecting_transforms(&merged_base_tags.transforms);

        // Check whether font properties vary across segments.
        // If so, lay out each homogeneous group separately and merge glyphs.
        let font_groups = group_segments_by_font(segments);
        let use_per_group = font_groups.len() > 1 && !layout_animates;

        let layout: TextLayout;
        if !use_per_group {
            let oxi_style = SubtitleStyle {
                font_size: px_size,
                font_weight: if bold { FontWeight::Bold } else { FontWeight::Normal },
                font_style: if italic { FontStyle::Italic } else { FontStyle::Normal },
                alignment: Alignment::Left,
                ..SubtitleStyle::default()
            };

            let layout_key = if layout_animates {
                None
            } else {
                Some(LayoutCacheKey {
                    event_ptr: ev_ptr,
                    size_bucket: (px_size * 2.0) as u32,
                    text_hash: cached_ev.text_hash,
                    font_data_ptr,
                })
            };

            layout = if let Some(ref key) = layout_key {
                if let Some(cached) = cache.text_layouts.get(key) {
                    cached.clone()
                } else {
                    let l = layout_engine.layout(text_for_layout, &oxi_style, max_width)
                        .unwrap_or_else(|_| TextLayout::new());
                    RenderCache::evict_if_full(&mut cache.text_layouts, MAX_TEXT_LAYOUTS, key);
                    cache.text_layouts.insert(key.clone(), l.clone());
                    l
                }
            } else {
                layout_engine.layout(text_for_layout, &oxi_style, max_width)
                    .unwrap_or_else(|_| TextLayout::new())
            };
        } else {
            // Per-group layout: each group gets its own SubtitleStyle.
            // Glyphs from all groups are merged into a single TextLayout
            // with adjusted X positions, so the rest of the pipeline is unchanged.
            let mut merged_lines: Vec<oximedia_subtitle::text::TextLine> = Vec::new();
            let mut total_w: f32 = 0.0;
            let mut total_h: f32 = 0.0;
            let mut x_cursor: f32 = 0.0;
            let mut y_cursor: f32 = 0.0;

            // Compute max first-line ascent across groups for vertical alignment
            let mut baseline_hint: Option<f32> = None;

            for &(g_start, g_end) in &font_groups {
                let group_text: String = segments[g_start..g_end]
                    .iter().map(|s| s.text.as_str()).collect();
                let group_tfl = group_text.replace("\\N", "\n").replace("\\n", "\n");

                let group_tags = &segments[g_start].tags;
                let g_bold = group_tags.bold.unwrap_or(style.bold);
                let g_italic = group_tags.italic.unwrap_or(style.italic);

                let g_oxi_style = SubtitleStyle {
                    font_size: px_size,
                    font_weight: if g_bold { FontWeight::Bold }
                                 else { FontWeight::Normal },
                    font_style: if g_italic { FontStyle::Italic }
                                else { FontStyle::Normal },
                    alignment: Alignment::Left,
                    ..SubtitleStyle::default()
                };

                let g_layout = layout_engine.layout(&group_tfl, &g_oxi_style, max_width)
                    .unwrap_or_else(|_| TextLayout::new());

                // Record first-line ascent for Y adjustment
                if baseline_hint.is_none() {
                    baseline_hint = g_layout.lines.first().map(|l| l.baseline);
                }

                // Check if group text ends with \n — reset X, advance Y
                let ends_with_newline = group_tfl.ends_with('\n');

                // Shift glyphs and add to merged lines
                for (_li, line) in g_layout.lines.iter().enumerate() {
                    let mut shifted_glyphs: Vec<oximedia_subtitle::text::PositionedGlyph> = Vec::new();
                    for glyph in &line.glyphs {
                        let mut g = glyph.clone();
                        g.x += x_cursor;
                        g.y += y_cursor;
                        shifted_glyphs.push(g);
                    }
                    merged_lines.push(oximedia_subtitle::text::TextLine {
                        glyphs: shifted_glyphs,
                        width: line.width,
                        height: line.height,
                        baseline: line.baseline,
                    });
                }

                // Update cursors
                if ends_with_newline {
                    x_cursor = 0.0;
                    y_cursor += g_layout.height;
                } else {
                    x_cursor += g_layout.width;
                }
                total_w = total_w.max(x_cursor); // take the widest "row"
                total_h = total_h.max(y_cursor + g_layout.height);
            }

            layout = TextLayout {
                lines: merged_lines,
                width: total_w,
                height: total_h,
            };
        }

        if layout.is_empty() {
            continue;
        }

        // oximedia's fontdue-based layout places glyph y relative to the
        // first line's baseline. We offset per-line using each line's own baseline
        // so multi-line text (e.g. from \N) positions correctly.
        let fsx = font_scale_x.max(0.01);
        let text_w = layout.width * tag_sx * fsx;
        let text_h = layout.height * tag_sy;

        let (align_x, align_y) = alignment_to_anchor(alignment);

        let margin_l = (if ev.margin_l != 0 { ev.margin_l } else { style.margin_l }) as f32
            * scale * res_scale_x;
        let margin_r = (if ev.margin_r != 0 { ev.margin_r } else { style.margin_r }) as f32
            * scale * res_scale_x;
        let margin_v = (if ev.margin_v != 0 { ev.margin_v } else { style.margin_v }) as f32
            * scale * res_scale_y;

        let out_w = output_width as f32;
        let out_h = output_height as f32;

        let base_x = match align_x {
            0 => center_off_x + margin_l,
            1 => center_off_x + (viewport_w - text_w) / 2.0,
            2 => center_off_x + viewport_w - text_w - margin_r,
            _ => center_off_x + (viewport_w - text_w) / 2.0,
        } + (position_x - 0.5) * out_w;

        let base_y = match align_y {
            0 => center_off_y + viewport_h - margin_v - text_h,
            1 => center_off_y + (viewport_h - text_h) / 2.0,
            2 => center_off_y + margin_v,
            _ => center_off_y + viewport_h - margin_v - text_h,
        } + (position_y - 0.5) * out_h;

        // Apply \pos or \move override
        let (base_x, base_y) = if let Some((px, py)) = inline_tags.pos {
            let anchor_x = center_off_x + px * scale * res_scale_x + (position_x - 0.5) * out_w;
            let anchor_y = center_off_y + py * scale * res_scale_y + (position_y - 0.5) * out_h;
            (anchor_to_base_x(align_x, anchor_x, text_w), anchor_to_base_y(align_y, anchor_y, text_h))
        } else if let Some((mx, my)) = move_pos {
            let anchor_x = center_off_x + mx * scale * res_scale_x + (position_x - 0.5) * out_w;
            let anchor_y = center_off_y + my * scale * res_scale_y + (position_y - 0.5) * out_h;
            (anchor_to_base_x(align_x, anchor_x, text_w), anchor_to_base_y(align_y, anchor_y, text_h))
        } else {
            (base_x, base_y)
        };

        // ------------------------------------------------------------------
        // Glyph rendering — GPU first (only if device already initialized), CPU fallback
        // ------------------------------------------------------------------
        let outline_radius = (outline_w * scale).round() as i32;
        let has_shadow = (shadow_dx.abs() > 0.01 || shadow_dy.abs() > 0.01) && shadow_color[3] > 0.0;
        let has_outline = outline_radius > 0 && border_style == 1 && outline_color[3] > 0.0;
        let has_fill = fill_color[3] > 0.0;

        #[cfg(feature = "gpu")]
        let sh_color = apply_fade_to_color(shadow_color, effective_alpha);
        #[cfg(feature = "gpu")]
        let out_color = apply_fade_to_color(outline_color, effective_alpha);
        #[cfg(feature = "gpu")]
        let fill = apply_fade_to_color(fill_color, effective_alpha);

        let gpu_glyph_ok;

        #[cfg(feature = "gpu")]
        {
            let mut ok = false;
            if gpu_ready {
                cache.glyph_gpu_data_buf.clear();
                cache.bitmap_bytes_buf.clear();
                let glyph_gpu_data = &mut cache.glyph_gpu_data_buf;
                let bitmap_bytes = &mut cache.bitmap_bytes_buf;
                for line in &layout.lines {
                    let line_ascent = line.baseline;
                    for glyph in &line.glyphs {
                        if glyph.width == 0 || glyph.height == 0 { continue; }
                        let gx = (base_x + glyph.x * tag_sx * fsx).round() as i32;
                        let gy = (base_y + line_ascent + glyph.y).round() as i32;
                        let mut flags = 0u32;
                        if has_fill { flags |= 1; }
                        if has_outline { flags |= 2; }
                        if has_shadow { flags |= 4; }
                        let offset = bitmap_bytes.len() as u32;
                        bitmap_bytes.extend_from_slice(&glyph.bitmap);
                        glyph_gpu_data.push(crate::gpu::ass_glyph::GlyphGpuData {
                            glyph_offset: offset,
                            bitmap_w: glyph.width as u32,
                            bitmap_h: glyph.height as u32,
                            pos_x: gx,
                            pos_y: gy,
                            fill_color: pack_rgba8(fill),
                            outline_color: pack_rgba8(out_color),
                            shadow_color: pack_rgba8(sh_color),
                            outline_radius,
                            shadow_dx,
                            shadow_dy,
                            flags,
                        });
                    }
                }
                ok = crate::gpu::ass_glyph::try_ass_glyph_gpu_composite(
                    glyph_gpu_data, bitmap_bytes, temp_buf,
                    output_width as u32, output_height as u32,
                ).unwrap_or(false);
                if ok {
                    stats.pixels_written += glyph_gpu_data.iter()
                        .map(|g| (g.bitmap_w * g.bitmap_h) as usize)
                        .sum::<usize>();
                    for g in glyph_gpu_data.iter() {
                        let pad = g.outline_radius.max(g.shadow_dx.abs().ceil() as i32).max(g.shadow_dy.abs().ceil() as i32) + 1;
                        new_dirty.expand(g.pos_x, g.pos_y, pad);
                        new_dirty.expand(g.pos_x + g.bitmap_w as i32, g.pos_y + g.bitmap_h as i32, pad);
                    }
                }
            }
            gpu_glyph_ok = ok;
        }
        #[cfg(not(feature = "gpu"))]
        {
            gpu_glyph_ok = false;
        }

        if !gpu_glyph_ok {
            // CPU fallback with per-character styling from segments
            let mut char_offset = 0usize;
            for line in &layout.lines {
                let line_ascent = line.baseline;
                for glyph in &line.glyphs {
                    let gx = (base_x + glyph.x * tag_sx * fsx).round() as i32;
                    let gy = (base_y + line_ascent + glyph.y).round() as i32;
                    if glyph.width == 0 || glyph.height == 0 {
                        char_offset += 1; // always advance for every character in text_for_layout
                        continue;
                    }

                    // Per-character colors from override tag segments.
                    // Use unfaded base colors to avoid double-fade; apply fade once at the end.
                    let seg_idx = char_to_seg.get(char_offset).copied().unwrap_or(0);
                    let seg_tags = segments.get(seg_idx).map(|s| &s.tags).unwrap_or(&inline_tags);
                    let base_fill = seg_tags.primary_color.unwrap_or(fill_color);
                    let base_outline = seg_tags.outline_color.unwrap_or(outline_color);
                    let base_shadow = seg_tags.back_color.unwrap_or(shadow_color);

                    let (ch_fill, ch_outline) = if !karaoke_syllables.is_empty() {
                        let secondary_fill = seg_tags.secondary_color
                            .or(inline_tags.secondary_color)
                            .unwrap_or(style.secondary_color);
                        compute_karaoke_color(
                            char_offset,
                            time_ms,
                            ev.start_ms,
                            karaoke_syllables,
                            base_fill,
                            secondary_fill,
                            base_outline,
                            secondary_fill,
                        )
                    } else {
                        (base_fill, base_outline)
                    };

                    let ch_fill_color = apply_fade_to_color(ch_fill, effective_alpha);
                    let ch_outline_color = apply_fade_to_color(ch_outline, effective_alpha);
                    let ch_shadow_color = apply_fade_to_color(base_shadow, effective_alpha);

                    stats.pixels_written += composite_glyph_all_layers(
                        temp_buf, &glyph.bitmap, glyph.width, glyph.height,
                        gx, gy,
                        if has_fill { Some(ch_fill_color) } else { None },
                        if has_outline { Some((ch_outline_color, outline_radius)) } else { None },
                        if has_shadow { Some((ch_shadow_color, shadow_dx, shadow_dy)) } else { None },
                        output_width, output_height,
                        &clip_check, &mut new_dirty,
                    );
                    char_offset += 1;
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Phase 3: GPU-first compositing (CPU fallback if GPU unavailable)
    // Only attempt if shared device was already initialized by another effect.
    // ------------------------------------------------------------------
    let gpu_composite_ok = if gpu_ready {
        #[cfg(feature = "gpu")]
        {
            match crate::gpu::ass_subtitle::try_ass_subtitle_gpu_composite(
                temp_buf, output,
                output_width as u32, output_height as u32,
                blend_mode as u32,
            ) {
                Ok(true) => { stats.gpu_composite_used = true; true }
                _ => false,
            }
        }
        #[cfg(not(feature = "gpu"))]
        false
    } else {
        false
    };
    if !gpu_composite_ok {
        if let Some(dr) = new_dirty.clamp(output_width as i32, output_height as i32) {
            cpu_composite_dirty_rect(temp_buf, output, output_width, &dr, blend_mode);
        }
    }
    cache.prev_dirty = new_dirty;

    stats
}

// ---------------------------------------------------------------------------
// Combined glyph compositing — single bitmap scan for fill + shadow + outline
// ---------------------------------------------------------------------------

/// Composite a glyph with fill, outline, and shadow in a single bitmap scan.
/// Returns the number of fill pixels written.
fn composite_glyph_all_layers(
    output: &mut [u8],
    bitmap: &[u8],
    bm_w: usize,
    bm_h: usize,
    gx: i32,
    gy: i32,
    fill_color: Option<[f32; 4]>,
    outline: Option<([f32; 4], i32)>,
    shadow: Option<([f32; 4], f32, f32)>,
    out_w: usize,
    out_h: usize,
    clip: &Option<ClipCheck>,
    dirty: &mut DirtyRect,
) -> usize {
    let mut pixels = 0usize;
    let r2 = outline.map_or(0.0, |(_, r)| (r * r) as f32);
    let radius = outline.map_or(0, |(_, r)| r);
    let (sh_color, sh_dx, sh_dy) = shadow.unwrap_or(([0.0; 4], 0.0, 0.0));
    let sh_x = sh_dx.round() as i32;
    let sh_y = sh_dy.round() as i32;
    let has_shadow = shadow.is_some();
    let has_outline = outline.is_some();
    let has_fill = fill_color.is_some();
    let fill = fill_color.unwrap_or([0.0; 4]);
    let out_color = outline.map_or([0.0; 4], |(c, _)| c);

    for by in 0..bm_h {
        for bx in 0..bm_w {
            let alpha = bitmap[by * bm_w + bx];
            if alpha == 0 { continue; }
            let coverage = alpha as f32 / 255.0;
            let base_x = gx + bx as i32;
            let base_y = gy + by as i32;

            // Fill — at glyph position
            if has_fill {
                if base_x >= 0 && base_y >= 0 && base_x < out_w as i32 && base_y < out_h as i32
                    && !clip_reject(base_x, base_y, clip)
                {
                    let idx = (base_y as usize * out_w + base_x as usize) * 4;
                    direct_composite(output, idx, fill, coverage);
                    dirty.expand(base_x, base_y, 2);
                    pixels += 1;
                }
            }

            // Shadow — at offset position
            if has_shadow {
                let sx = base_x + sh_x;
                let sy = base_y + sh_y;
                if sx >= 0 && sy >= 0 && sx < out_w as i32 && sy < out_h as i32
                    && !clip_reject(sx, sy, clip)
                {
                    let idx = (sy as usize * out_w + sx as usize) * 4;
                    direct_composite(output, idx, sh_color, coverage);
                    dirty.expand(sx, sy, 2);
                }
            }

            // Outline — draw colored pixels around glyph in a circular radius
            if has_outline {
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        if (dx * dx + dy * dy) as f32 > r2 { continue; }
                        let ox = base_x + dx;
                        let oy = base_y + dy;
                        if ox < 0 || oy < 0 || ox >= out_w as i32 || oy >= out_h as i32 { continue; }
                        if clip_reject(ox, oy, clip) { continue; }
                        let idx = (oy as usize * out_w + ox as usize) * 4;
                        direct_composite_max(output, idx, out_color, coverage);
                        dirty.expand(ox, oy, 2);
                    }
                }
            }
        }
    }
    pixels
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Check if any \t transforms affect text layout (font, size, boldness, scale, spacing).
fn has_layout_affecting_transforms(transforms: &[super::types::OverrideTransform]) -> bool {
    transforms.iter().any(|t| {
        t.tags.fontsize.is_some()
            || t.tags.fontname.is_some()
            || t.tags.bold.is_some()
            || t.tags.italic.is_some()
            || t.tags.scale_x.is_some()
            || t.tags.scale_y.is_some()
            || t.tags.spacing.is_some()
    })
}

/// Pack [r, g, b, a] normalized 0..1 into u32 RGBA8.
#[cfg(feature = "gpu")]
fn pack_rgba8(c: [f32; 4]) -> u32 {
    let r = (c[0].clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    let g = (c[1].clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    let b = (c[2].clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    let a = (c[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    r | (g << 8) | (b << 16) | (a << 24)
}

/// Fast non-crypto string hash for layout cache keying.
fn hash_str(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Return active events at a given time, sorted by layer (higher = later = on top).
fn active_events(ass_script: &AssScript, time_ms: i64) -> Vec<&OwnedEvent> {
    let mut events: Vec<&OwnedEvent> = ass_script
        .events
        .iter()
        .filter(|e| e.start_ms <= time_ms && time_ms < e.end_ms)
        .collect();
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
            }
        }

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

// ---------------------------------------------------------------------------
// Per-character font style helpers (Fix 5)
// ---------------------------------------------------------------------------

/// Check whether two ParsedTags agree on font-affecting properties.
fn font_style_equal(a: &ParsedTags, b: &ParsedTags) -> bool {
    a.bold == b.bold && a.italic == b.italic && a.fontname == b.fontname && a.fontsize == b.fontsize
}

/// Build karaoke syllable boundaries from tag segments.
fn build_karaoke_syllables(segments: &[TagSegment]) -> Vec<KaraokeSyllable> {
    let mut syllables = Vec::new();
    let mut char_offset = 0usize;
    for seg in segments {
        let seg_len = seg.text.chars().count();
        if let Some(ref kd) = seg.tags.karaoke {
            syllables.push(KaraokeSyllable {
                char_end: char_offset + seg_len,
                duration_cs: kd.duration_cs,
            });
        }
        char_offset += seg_len;
    }
    syllables
}

/// Compute karaoke color for a character at the given time.
/// Returns (fill_color, outline_color) based on elapsed time vs syllable durations.
fn compute_karaoke_color(
    char_idx: usize,
    time_ms: i64,
    ev_start: i64,
    syllables: &[KaraokeSyllable],
    before_fill: [f32; 4],
    after_fill: [f32; 4],
    before_outline: [f32; 4],
    after_outline: [f32; 4],
) -> ([f32; 4], [f32; 4]) {
    let elapsed_ms = time_ms - ev_start;
    let mut cumulative_ms: i64 = 0;
    for syl in syllables {
        if char_idx < syl.char_end {
            let syl_elapsed = elapsed_ms - cumulative_ms;
            let syl_dur_ms = syl.duration_cs * 10;
            if syl_elapsed >= syl_dur_ms {
                // Syllable fully sung
                return (after_fill, after_outline);
            } else if syl_elapsed > 0 {
                // Currently being sung — partial blend
                let frac = syl_elapsed as f32 / syl_dur_ms as f32;
                return (
                    [
                        before_fill[0] + (after_fill[0] - before_fill[0]) * frac,
                        before_fill[1] + (after_fill[1] - before_fill[1]) * frac,
                        before_fill[2] + (after_fill[2] - before_fill[2]) * frac,
                        before_fill[3] + (after_fill[3] - before_fill[3]) * frac,
                    ],
                    [
                        before_outline[0] + (after_outline[0] - before_outline[0]) * frac,
                        before_outline[1] + (after_outline[1] - before_outline[1]) * frac,
                        before_outline[2] + (after_outline[2] - before_outline[2]) * frac,
                        before_outline[3] + (after_outline[3] - before_outline[3]) * frac,
                    ],
                );
            } else {
                // Not yet reached this syllable
                return (before_fill, before_outline);
            }
        }
        cumulative_ms += syl.duration_cs * 10;
    }
    (before_fill, before_outline)
}

/// Group consecutive segments that share font-affecting properties.
/// Returns Vec of (start_idx, end_idx) ranges.
fn group_segments_by_font(segments: &[TagSegment]) -> Vec<(usize, usize)> {
    if segments.is_empty() {
        return Vec::new();
    }
    let mut groups = Vec::new();
    let mut start = 0usize;
    for i in 1..segments.len() {
        if !font_style_equal(&segments[start].tags, &segments[i].tags) {
            groups.push((start, i));
            start = i;
        }
    }
    groups.push((start, segments.len()));
    groups
}
