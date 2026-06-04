//! ASS file and override-tag parsing.

use std::collections::HashMap;

use crate::blend::RECIP_255;

use super::types::*;

// ---------------------------------------------------------------------------
// ASS file parser
// ---------------------------------------------------------------------------

/// Parse an ASS file from a string using oximedia-subtitle for structure.
/// Extracts raw event text with override tags preserved for our tag parser.
pub fn parse_ass_file(content: &str) -> Result<AssScript, String> {
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    let normalized = content.replace("\r\n", "\n");

    // Use oximedia-subtitle for script info + styles parsing
    let ass_file = oximedia_subtitle::parser::ssa::parse_ass(&normalized)
        .map_err(|e| format!("ASS parse error: {e}"))?;

    let info = ass_file.script_info;
    let play_res_x = info.get("PlayResX").and_then(|v| v.parse().ok());
    let play_res_y = info.get("PlayResY").and_then(|v| v.parse().ok());

    // oximedia's SubtitleStyle has no font_name field — extract from raw ASS ourselves
    let style_fontnames = parse_style_fontnames(&normalized);

    // Convert oximedia styles to our OwnedStyle
    let styles: Vec<OwnedStyle> = ass_file
        .styles
        .iter()
        .map(|(name, s)| convert_oximedia_style(name, s, &style_fontnames))
        .collect();

    // Extract events ourselves to preserve raw text (oximedia strips override tags)
    let events = parse_events_raw(&normalized, &styles);

    Ok(AssScript { info, styles, events, play_res_x, play_res_y })
}

// ---------------------------------------------------------------------------
// Raw event extraction (preserves override tags)
// ---------------------------------------------------------------------------

fn parse_events_raw(content: &str, _styles: &[OwnedStyle]) -> Vec<OwnedEvent> {
    let mut events = Vec::new();
    let mut in_events = false;
    let mut event_format = Vec::new();

    for line in content.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let section = &line[1..line.len() - 1];
            in_events = section == "Events";
            continue;
        }

        if !in_events {
            continue;
        }

        if line.starts_with("Format:") {
            event_format = parse_format_line(line);
        } else if line.starts_with("Dialogue:") || line.starts_with("Comment:") {
            // Skip comments
            if line.starts_with("Comment:") {
                continue;
            }

            let content_part = line.strip_prefix("Dialogue:").unwrap_or("");
            let parts: Vec<&str> = content_part.splitn(event_format.len(), ',').collect();

            let mut field_map: HashMap<String, String> = HashMap::new();
            for (i, field) in event_format.iter().enumerate() {
                if let Some(&value) = parts.get(i) {
                    field_map.insert(field.clone(), value.trim().to_string());
                }
            }

            let layer: i32 = field_map.get("Layer").and_then(|v| v.parse().ok()).unwrap_or(0);
            let start_ms = field_map
                .get("Start")
                .and_then(|v| parse_ass_timestamp(v))
                .unwrap_or(0);
            let end_ms = field_map
                .get("End")
                .and_then(|v| parse_ass_timestamp(v))
                .unwrap_or(0);
            let style_name = field_map
                .get("Style")
                .cloned()
                .unwrap_or_else(|| "Default".to_string());
            let name = field_map.get("Name").cloned().unwrap_or_default();
            let margin_l: i32 = field_map
                .get("MarginL")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let margin_r: i32 = field_map
                .get("MarginR")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let margin_v: i32 = field_map
                .get("MarginV")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let effect = field_map.get("Effect").cloned().unwrap_or_default();

            // Raw text — keep override tags intact
            let text = field_map.get("Text").cloned().unwrap_or_default();

            events.push(OwnedEvent {
                layer,
                start_ms,
                end_ms,
                style_name,
                name,
                margin_l,
                margin_r,
                margin_v,
                effect,
                text,
            });
        }
    }

    events
}

fn parse_format_line(line: &str) -> Vec<String> {
    line.split(':')
        .nth(1)
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .collect()
}

/// Parse ASS timestamp (e.g., "0:00:01.00") → milliseconds.
fn parse_ass_timestamp(ts: &str) -> Option<i64> {
    let parts: Vec<&str> = ts.split(':').collect();
    if parts.len() != 3 {
        return None;
    }

    let hours: i64 = parts[0].parse().ok()?;
    let minutes: i64 = parts[1].parse().ok()?;

    let sec_parts: Vec<&str> = parts[2].split('.').collect();
    if sec_parts.len() != 2 {
        return None;
    }

    let seconds: i64 = sec_parts[0].parse().ok()?;
    let centiseconds: i64 = sec_parts[1].parse().ok()?;

    Some(hours * 3600000 + minutes * 60000 + seconds * 1000 + centiseconds * 10)
}

// ---------------------------------------------------------------------------
// Style font name extraction (oximedia's SubtitleStyle has no font_name field)
// ---------------------------------------------------------------------------

/// Parse [V4+ Styles] section to extract font names per style.
/// oximedia's SubtitleStyle stores alignment/colors/outline but NOT the font name.
fn parse_style_fontnames(content: &str) -> HashMap<String, String> {
    let mut fontnames = HashMap::new();
    let mut in_styles = false;
    let mut name_col: Option<usize> = None;
    let mut font_col: Option<usize> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            let section = &line[1..line.len() - 1];
            in_styles = section == "V4+ Styles" || section == "V4 Styles";
            name_col = None;
            font_col = None;
            continue;
        }
        if !in_styles { continue; }

        if line.starts_with("Format:") {
            let cols: Vec<&str> = line[7..].split(',').map(|s| s.trim()).collect();
            name_col = cols.iter().position(|&c| c == "Name");
            font_col = cols.iter().position(|&c| c == "Fontname");
        } else if line.starts_with("Style:") {
            let parts: Vec<&str> = line[6..].split(',').collect();
            let name = name_col.and_then(|c| parts.get(c)).map(|s| s.trim().to_string());
            let fontname = font_col.and_then(|c| parts.get(c)).map(|s| s.trim().to_string());
            if let (Some(n), Some(f)) = (name, fontname) {
                fontnames.insert(n, f);
            }
        }
    }
    fontnames
}

// ---------------------------------------------------------------------------
// oximedia-subtitle type conversions
// ---------------------------------------------------------------------------

use oximedia_subtitle::style::{Color, SubtitleStyle};

fn convert_oximedia_style(name: &str, s: &SubtitleStyle, fontnames: &HashMap<String, String>) -> OwnedStyle {
    use oximedia_subtitle::style::FontWeight;
    let bold = matches!(s.font_weight, FontWeight::Bold | FontWeight::ExtraBold | FontWeight::Black);
    let italic = matches!(s.font_style, oximedia_subtitle::style::FontStyle::Italic);

    OwnedStyle {
        name: name.to_string(),
        fontname: fontnames.get(name).cloned().unwrap_or_else(|| "Arial".to_string()),
        fontsize: s.font_size,
        primary_color: oxi_color_to_rgba(s.primary_color),
        secondary_color: oxi_color_to_rgba(s.secondary_color),
        outline_color: s
            .outline
            .as_ref()
            .map(|o| oxi_color_to_rgba(o.color))
            .unwrap_or([0.0, 0.0, 0.0, 1.0]),
        back_color: s
            .shadow
            .as_ref()
            .map(|sh| oxi_color_to_rgba(sh.color))
            .unwrap_or([0.0, 0.0, 0.0, 1.0]),
        bold,
        italic,
        underline: false,
        strikeout: false,
        scale_x: 100.0,
        scale_y: 100.0,
        spacing: 0.0,
        angle: 0.0,
        border_style: 1,
        outline: s.outline.as_ref().map(|o| o.width).unwrap_or(2.0),
        shadow: s.shadow.as_ref().map(|sh| sh.offset_x.max(sh.offset_y)).unwrap_or(2.0),
        alignment: convert_oxi_alignment(s.alignment, s.vertical_alignment),
        margin_l: s.margin_left as i32,
        margin_r: s.margin_right as i32,
        margin_v: s.margin_bottom as i32,
    }
}

fn convert_oxi_alignment(
    h: oximedia_subtitle::style::Alignment,
    v: oximedia_subtitle::style::VerticalAlignment,
) -> i32 {
    use oximedia_subtitle::style::{Alignment, VerticalAlignment};
    match (h, v) {
        (Alignment::Left, VerticalAlignment::Bottom) => 1,
        (Alignment::Center, VerticalAlignment::Bottom) => 2,
        (Alignment::Right, VerticalAlignment::Bottom) => 3,
        (Alignment::Left, VerticalAlignment::Middle) => 4,
        (Alignment::Center, VerticalAlignment::Middle) => 5,
        (Alignment::Right, VerticalAlignment::Middle) => 6,
        (Alignment::Left, VerticalAlignment::Top) => 7,
        (Alignment::Center, VerticalAlignment::Top) => 8,
        (Alignment::Right, VerticalAlignment::Top) => 9,
    }
}

fn oxi_color_to_rgba(c: Color) -> [f32; 4] {
    [
        c.r as f32 * RECIP_255,
        c.g as f32 * RECIP_255,
        c.b as f32 * RECIP_255,
        c.a as f32 * RECIP_255,
    ]
}

// ---------------------------------------------------------------------------
// Color parsing
// ---------------------------------------------------------------------------

/// Parse ASS color `&HAABBGGRR` → `[r, g, b, a]` normalized 0..1.
/// ASS alpha is inverted: `&H00` = opaque, `&HFF` = transparent.
pub fn ass_color_to_rgba(hex: &str) -> [f32; 4] {
    let hex = hex
        .trim()
        .trim_start_matches("&H")
        .trim_start_matches("&h")
        .trim_end_matches('&');

    if hex.len() < 6 {
        return [1.0, 1.0, 1.0, 1.0];
    }

    // ASS colors: &HAABBGGRR — reversed byte order
    let parse_byte = |start: usize| -> Option<u8> {
        u8::from_str_radix(hex.get(start..start + 2)?, 16).ok()
    };

    // Alpha: first 2 chars if 8-char hex (AA in &HAABBGGRR); for 6-char, default 0x00 = opaque
    let alpha = if hex.len() >= 8 {
        parse_byte(0).unwrap_or(0x00)
    } else {
        0x00
    };

    // Take last 6 chars for BGR
    let color_part = if hex.len() >= 6 {
        &hex[hex.len() - 6..]
    } else {
        hex
    };

    let bb = u8::from_str_radix(&color_part[0..2], 16).unwrap_or(0xFF);
    let gg = u8::from_str_radix(&color_part[2..4], 16).unwrap_or(0xFF);
    let rr = u8::from_str_radix(&color_part[4..6], 16).unwrap_or(0xFF);

    let a = 1.0 - alpha as f32 * RECIP_255;
    [
        rr as f32 * RECIP_255,
        gg as f32 * RECIP_255,
        bb as f32 * RECIP_255,
        a,
    ]
}

// ---------------------------------------------------------------------------
// Override tag parsing — public API
// ---------------------------------------------------------------------------

/// Parse ASS override tags from dialogue text.
/// Returns (clean_text, parsed_tags).
/// Handles `\r` reset: bare `\r` resets all tags to defaults.
pub fn parse_override_tags(raw_text: &str) -> (String, ParsedTags) {
    let mut tags = ParsedTags::default();
    let mut clean = String::with_capacity(raw_text.len());
    let mut chars = raw_text.char_indices().peekable();

    while let Some((byte_pos, ch)) = chars.next() {
        if ch == '{' {
            let remaining = &raw_text[byte_pos..];
            if let Some(tag_end) = remaining.find('}') {
                let tag_str = &remaining[1..tag_end];
                parse_tag_block(tag_str, &mut tags);
                // Handle \r reset: bare \r clears all tag overrides
                if tags.reset && tags.reset_style.is_none() {
                    reset_all_tags(&mut tags);
                }
                let skip_bytes = byte_pos + tag_end + 1;
                while let Some((pos, _)) = chars.peek() {
                    if *pos < skip_bytes {
                        chars.next();
                    } else {
                        break;
                    }
                }
            } else {
                clean.push('{');
            }
        } else {
            clean.push(ch);
        }
    }

    (clean, tags)
}

/// Parse text into segments split by override tag blocks.
/// Each segment carries the tag state active for its text span.
/// `\r` reset is properly handled: bare `\r` resets to defaults,
/// named `\rStyleName` is recorded for later resolution by the renderer.
pub fn parse_tag_segments(raw_text: &str) -> Vec<TagSegment> {
    let mut segments = Vec::new();
    let mut current_tags = ParsedTags::default();
    let mut current_text = String::new();

    let mut chars = raw_text.char_indices().peekable();
    while let Some((byte_pos, ch)) = chars.next() {
        if ch == '{' {
            let remaining = &raw_text[byte_pos..];
            if let Some(tag_end) = remaining.find('}') {
                // Flush accumulated text before processing new tag block
                if !current_text.is_empty() {
                    segments.push(TagSegment {
                        text: std::mem::take(&mut current_text),
                        tags: current_tags.clone(),
                    });
                }
                let tag_str = &remaining[1..tag_end];
                parse_tag_block(tag_str, &mut current_tags);
                // Handle \r reset
                if current_tags.reset {
                    if current_tags.reset_style.is_none() {
                        // Bare \r: reset all fields to defaults
                        reset_all_tags(&mut current_tags);
                    }
                    // Named \r: keep the reset_style marker; renderer resolves it
                }
                let skip_end = byte_pos + tag_end + 1;
                while let Some((pos, _)) = chars.peek() {
                    if *pos < skip_end {
                        chars.next();
                    } else {
                        break;
                    }
                }
            } else {
                current_text.push('{');
            }
        } else {
            current_text.push(ch);
        }
    }

    if !current_text.is_empty() || segments.is_empty() {
        segments.push(TagSegment { text: current_text, tags: current_tags });
    }
    segments
}

// ---------------------------------------------------------------------------
// Override tag block parser
// ---------------------------------------------------------------------------

fn parse_tag_block(block: &str, tags: &mut ParsedTags) {
    let mut rest = block;
    while !rest.is_empty() {
        let Some(stripped) = rest.strip_prefix('\\') else { break; };
        rest = stripped;

        // Tag name: alphabetic chars
        let name_end = rest
            .find(|c: char| !c.is_alphabetic())
            .unwrap_or(rest.len());
        if name_end == 0 {
            break;
        }
        let name = &rest[..name_end];
        rest = &rest[name_end..];

        // Parse value: parens, or numeric/hex until next backslash
        let value: Option<&str> = if rest.starts_with('(') {
            let mut depth = 1u32;
            let mut close = 0usize;
            for (j, ch) in rest.char_indices().skip(1) {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            close = j;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if close > 0 {
                let v = &rest[1..close];
                rest = &rest[(close + 1).min(rest.len())..];
                Some(v)
            } else {
                let v = &rest[1..];
                rest = &rest[rest.len()..];
                Some(v)
            }
        } else if rest.starts_with(|c: char| c == '-' || c == '.' || c.is_numeric() || c == '&') {
            let v_end = rest.find('\\').unwrap_or(rest.len());
            let v = &rest[..v_end];
            rest = &rest[v_end..];
            Some(v)
        } else {
            None
        };

        match name {
            // Font
            "fn" => {
                if let Some(v) = value {
                    tags.fontname = Some(v.to_string());
                }
            }
            "fs" => {
                if let Some(v) = value {
                    tags.fontsize = v.parse().ok();
                }
            }
            "b" => parse_bool_tag(value, &mut tags.bold),
            "i" => parse_bool_tag(value, &mut tags.italic),
            "u" => parse_bool_tag(value, &mut tags.underline),
            "s" => parse_bool_tag(value, &mut tags.strikeout),
            "fsp" => {
                if let Some(v) = value {
                    tags.spacing = v.parse().ok();
                }
            }

            // Colors
            "c" | "1c" => {
                if let Some(v) = value {
                    tags.primary_color = Some(ass_color_to_rgba(v));
                }
            }
            "2c" => {
                if let Some(v) = value {
                    tags.secondary_color = Some(ass_color_to_rgba(v));
                }
            }
            "3c" => {
                if let Some(v) = value {
                    tags.outline_color = Some(ass_color_to_rgba(v));
                }
            }
            "4c" => {
                if let Some(v) = value {
                    tags.back_color = Some(ass_color_to_rgba(v));
                }
            }

            // Alpha
            "alpha" => {
                if let Some(v) = value {
                    tags.alpha = parse_alpha_hex(v);
                }
            }
            "1a" => {
                if let Some(v) = value {
                    apply_alpha_channel(tags, 1, v);
                }
            }
            "2a" => {
                if let Some(v) = value {
                    apply_alpha_channel(tags, 2, v);
                }
            }
            "3a" => {
                if let Some(v) = value {
                    apply_alpha_channel(tags, 3, v);
                }
            }
            "4a" => {
                if let Some(v) = value {
                    apply_alpha_channel(tags, 4, v);
                }
            }

            // Scale
            "fscx" => {
                if let Some(v) = value {
                    tags.scale_x = v.parse().ok();
                }
            }
            "fscy" => {
                if let Some(v) = value {
                    tags.scale_y = v.parse().ok();
                }
            }

            // Alignment
            "an" => {
                if let Some(v) = value {
                    tags.alignment = v.parse().ok();
                }
            }
            "a" => {
                if let Some(v) = value {
                    let legacy: i32 = v.parse().unwrap_or(2);
                    tags.alignment = Some(legacy_to_an(legacy));
                }
            }

            // Position
            "pos" => {
                if let Some(v) = value {
                    let coords: Vec<&str> = v.split(',').collect();
                    if coords.len() >= 2 {
                        tags.pos = Some((
                            coords[0].trim().parse().unwrap_or(0.0),
                            coords[1].trim().parse().unwrap_or(0.0),
                        ));
                    }
                }
            }
            "org" => {
                if let Some(v) = value {
                    let parts: Vec<&str> = v.split(',').collect();
                    if parts.len() >= 2 {
                        tags.org = Some((
                            parts[0].trim().parse().unwrap_or(0.0),
                            parts[1].trim().parse().unwrap_or(0.0),
                        ));
                    }
                }
            }
            "move" => {
                if let Some(v) = value {
                    let parts: Vec<&str> = v.split(',').collect();
                    if parts.len() >= 4 {
                        let x1: f32 = parts[0].trim().parse().unwrap_or(0.0);
                        let y1: f32 = parts[1].trim().parse().unwrap_or(0.0);
                        let x2: f32 = parts[2].trim().parse().unwrap_or(0.0);
                        let y2: f32 = parts[3].trim().parse().unwrap_or(0.0);
                        let t1 = parts
                            .get(4)
                            .and_then(|s| s.trim().parse().ok())
                            .map(|ms: i64| ms);
                        let t2 = parts
                            .get(5)
                            .and_then(|s| s.trim().parse().ok())
                            .map(|ms: i64| ms);
                        tags.move_ = Some(MoveAnim { x1, y1, x2, y2, t1, t2 });
                    }
                }
            }

            // Rotation
            "frz" => {
                if let Some(v) = value {
                    tags.frz = v.parse().ok();
                }
            }
            "frx" => {
                if let Some(v) = value {
                    tags.frx = v.parse().ok();
                }
            }
            "fry" => {
                if let Some(v) = value {
                    tags.fry = v.parse().ok();
                }
            }

            // Shearing
            "fax" => {
                if let Some(v) = value {
                    tags.fax = v.parse().ok();
                }
            }
            "fay" => {
                if let Some(v) = value {
                    tags.fay = v.parse().ok();
                }
            }

            // Border / shadow
            "bord" => {
                if let Some(v) = value {
                    let val: f32 = v.parse().unwrap_or(0.0);
                    tags.bord = Some(val);
                    tags.xbord = Some(val);
                    tags.ybord = Some(val);
                }
            }
            "xbord" => {
                if let Some(v) = value {
                    tags.xbord = v.parse().ok();
                }
            }
            "ybord" => {
                if let Some(v) = value {
                    tags.ybord = v.parse().ok();
                }
            }
            "shad" => {
                if let Some(v) = value {
                    let val: f32 = v.parse().unwrap_or(0.0);
                    tags.shad = Some(val);
                    tags.xshad = Some(val);
                    tags.yshad = Some(val);
                }
            }
            "xshad" => {
                if let Some(v) = value {
                    tags.xshad = v.parse().ok();
                }
            }
            "yshad" => {
                if let Some(v) = value {
                    tags.yshad = v.parse().ok();
                }
            }

            // Blur
            "be" | "be1" => {
                if let Some(v) = value {
                    tags.be = v.parse().ok();
                }
            }
            "blur" => {
                if let Some(v) = value {
                    tags.blur = v.parse().ok();
                }
            }

            // Clip — \clip and \iclip merged into single field.
            // \clip sets inverse=false; \iclip sets inverse=true. Last tag wins.
            "clip" => {
                if let Some(v) = value {
                    tags.clip = Some(parse_clip(v, false));
                }
            }
            "iclip" => {
                if let Some(v) = value {
                    tags.clip = Some(parse_clip(v, true));
                }
            }

            // Fade
            "fad" => {
                if let Some(v) = value {
                    let parts: Vec<&str> = v.split(',').collect();
                    if parts.len() >= 2 {
                        tags.fade = Some(FadeData {
                            a1: 0.0,
                            a2: 1.0,
                            a3: 1.0,
                            t1: parts[0].trim().parse().unwrap_or(0),
                            t2: parts[1].trim().parse().unwrap_or(0),
                            t3: 0,
                            t4: 0,
                            is_complex: false,
                        });
                    }
                }
            }
            "fade" => {
                if let Some(v) = value {
                    let parts: Vec<&str> = v.split(',').collect();
                    if parts.len() >= 7 {
                        tags.fade = Some(FadeData {
                            a1: parse_alpha_str(parts[0].trim()),
                            a2: parse_alpha_str(parts[1].trim()),
                            a3: parse_alpha_str(parts[2].trim()),
                            t1: parts[3].trim().parse().unwrap_or(0),
                            t2: parts[4].trim().parse().unwrap_or(0),
                            t3: parts[5].trim().parse().unwrap_or(0),
                            t4: parts[6].trim().parse().unwrap_or(0),
                            is_complex: true,
                        });
                    }
                }
            }

            // Karaoke
            "k" | "K" => {
                if let Some(v) = value {
                    tags.karaoke = Some(KaraokeData {
                        duration_cs: v.parse().unwrap_or(0),
                        kind: KaraokeKind::Normal,
                    });
                }
            }
            "kf" | "KF" => {
                if let Some(v) = value {
                    tags.karaoke = Some(KaraokeData {
                        duration_cs: v.parse().unwrap_or(0),
                        kind: KaraokeKind::Fill,
                    });
                }
            }
            "ko" | "KO" => {
                if let Some(v) = value {
                    tags.karaoke = Some(KaraokeData {
                        duration_cs: v.parse().unwrap_or(0),
                        kind: KaraokeKind::Outline,
                    });
                }
            }

            // Drawing
            "p" => {
                let scale: f32 = value
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0.0);
                tags.drawing_scale = if scale > 0.0 { Some(scale) } else { None };
            }

            // Reset
            "r" => {
                tags.reset = true;
                tags.reset_style = value.map(|v| v.trim().to_string());
            }

            // Transform \t(...)
            "t" => {
                if let Some(v) = value {
                    if let Some(t) = parse_transform(v) {
                        tags.transforms.push(t);
                    }
                }
            }

            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tag parsing helpers
// ---------------------------------------------------------------------------

fn parse_bool_tag(value: Option<&str>, target: &mut Option<bool>) {
    match value {
        Some(v) => {
            let val: i32 = v.parse().unwrap_or(-1);
            *target = Some(val != 0);
        }
        None => *target = Some(true),
    }
}

fn parse_alpha_hex(v: &str) -> Option<f32> {
    let hex = v
        .trim()
        .trim_start_matches("&H")
        .trim_start_matches("&h");
    if let Ok(val) = u32::from_str_radix(hex, 16) {
        let a_byte = (val & 0xFF) as u8;
        Some(1.0 - a_byte as f32 * RECIP_255)
    } else {
        None
    }
}

fn parse_alpha_str(s: &str) -> f32 {
    s.parse::<f32>()
        .ok()
        .map(|v| v * RECIP_255)
        .unwrap_or_else(|| parse_alpha_hex(s).unwrap_or(1.0))
}

fn apply_alpha_channel(tags: &mut ParsedTags, channel: u8, v: &str) {
    let a = parse_alpha_hex(v);
    let apply = |color: &mut Option<[f32; 4]>| {
        if let Some(c) = color {
            if let Some(alpha) = a {
                c[3] = alpha;
            }
        }
    };
    match channel {
        1 => apply(&mut tags.primary_color),
        2 => apply(&mut tags.secondary_color),
        3 => apply(&mut tags.outline_color),
        4 => apply(&mut tags.back_color),
        _ => {}
    }
}

fn parse_clip(v: &str, inverse: bool) -> ClipData {
    let parts: Vec<&str> = v.split(',').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return ClipData {
            inverse,
            scale: None,
            points: vec![],
        };
    }

    // Rectangular clip: x1,y1,x2,y2 (4 parts, no scale)
    if parts.len() == 4 && parts[0].parse::<f32>().map_or(false, |_| true) {
        let x1: f32 = parts[0].parse().unwrap_or(0.0);
        let y1: f32 = parts[1].parse().unwrap_or(0.0);
        let x2: f32 = parts[2].parse().unwrap_or(0.0);
        let y2: f32 = parts[3].parse().unwrap_or(0.0);
        return ClipData {
            inverse,
            scale: None,
            points: vec![(x1, y1), (x2, y1), (x2, y2), (x1, y2)],
        };
    }

    // Vector clip: scale,x1,y1,x2,y2,...
    let scale: f32 = parts[0].parse().unwrap_or(1.0);
    let mut points = Vec::new();
    let mut i = 1;
    while i + 1 < parts.len() {
        let x: f32 = parts[i].parse().unwrap_or(0.0);
        let y: f32 = parts[i + 1].parse().unwrap_or(0.0);
        points.push((x, y));
        i += 2;
    }
    ClipData {
        inverse,
        scale: Some(scale),
        points,
    }
}

fn parse_transform(v: &str) -> Option<OverrideTransform> {
    let parts: Vec<&str> = v.split(',').collect();
    if parts.is_empty() {
        return None;
    }

    // Find where the tag part starts (first backslash)
    let tag_start = v.find('\\').unwrap_or(v.len());
    let before_tags = &v[..tag_start];
    let before_parts: Vec<&str> =
        before_tags.split(',').filter(|s| !s.trim().is_empty()).collect();

    match before_parts.len() {
        // \t(\tags...) — animate over entire event duration, accel=1.0
        0 => {
            let mut tags = ParsedTags::default();
            if tag_start < v.len() {
                parse_tag_block(&v[tag_start..], &mut tags);
            }
            Some(OverrideTransform {
                start_t: 0,
                end_t: 0,
                acceleration: 1.0,
                tags: Box::new(tags),
            })
        }
        // \t(accel,\tags...)
        1 => {
            let accel: f32 = before_parts[0].trim().parse().unwrap_or(1.0);
            let mut tags = ParsedTags::default();
            if tag_start < v.len() {
                parse_tag_block(&v[tag_start..], &mut tags);
            }
            Some(OverrideTransform {
                start_t: 0,
                end_t: 0,
                acceleration: accel,
                tags: Box::new(tags),
            })
        }
        // \t(t1,t2,\tags...) or \t(t1,t2,accel,\tags...)
        2 | 3 => {
            let t1: i64 = before_parts[0].trim().parse().unwrap_or(0);
            let t2: i64 = before_parts[1].trim().parse().unwrap_or(0);
            let accel: f32 = before_parts
                .get(2)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(1.0);
            let mut tags = ParsedTags::default();
            if tag_start < v.len() {
                parse_tag_block(&v[tag_start..], &mut tags);
            }
            Some(OverrideTransform {
                start_t: t1,
                end_t: t2,
                acceleration: accel,
                tags: Box::new(tags),
            })
        }
        _ => None,
    }
}

/// Convert legacy ASS alignment (1-9) to ASS+ alignment (1-9 bottom-to-top).
fn legacy_to_an(legacy: i32) -> i32 {
    match legacy {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 7,
        5 => 8,
        6 => 9,
        7 => 4,
        8 => 5,
        9 => 6,
        _ => 2,
    }
}

/// Reset all tag fields on a `ParsedTags` to `None` (bare `\r` behavior).
fn reset_all_tags(tags: &mut ParsedTags) {
    tags.fontname = None;
    tags.fontsize = None;
    tags.bold = None;
    tags.italic = None;
    tags.underline = None;
    tags.strikeout = None;
    tags.primary_color = None;
    tags.secondary_color = None;
    tags.outline_color = None;
    tags.back_color = None;
    tags.alpha = None;
    tags.scale_x = None;
    tags.scale_y = None;
    tags.spacing = None;
    tags.alignment = None;
    tags.pos = None;
    tags.org = None;
    tags.move_ = None;
    tags.frz = None;
    tags.frx = None;
    tags.fry = None;
    tags.fax = None;
    tags.fay = None;
    tags.bord = None;
    tags.shad = None;
    tags.xbord = None;
    tags.ybord = None;
    tags.xshad = None;
    tags.yshad = None;
    tags.be = None;
    tags.blur = None;
    tags.clip = None;
    tags.fade = None;
    tags.karaoke = None;
    tags.drawing_scale = None;
    tags.reset = false;
}

// ---------------------------------------------------------------------------
// Alignment helpers (used by renderer)
// ---------------------------------------------------------------------------

/// Convert alignment (an 1-9) to (x_anchor, y_anchor): 0=left/bottom, 1=center, 2=right/top.
pub(crate) fn alignment_to_anchor(an: i32) -> (i32, i32) {
    match an {
        1 => (0, 0),
        2 => (1, 0),
        3 => (2, 0),
        4 => (0, 1),
        5 => (1, 1),
        6 => (2, 1),
        7 => (0, 2),
        8 => (1, 2),
        9 => (2, 2),
        _ => (1, 0),
    }
}

/// Convert an anchor X position to the text block's left edge (base_x).
pub(crate) fn anchor_to_base_x(align_x: i32, anchor_x: f32, text_w: f32) -> f32 {
    match align_x {
        0 => anchor_x,
        1 => anchor_x - text_w * 0.5,
        2 => anchor_x - text_w,
        _ => anchor_x - text_w * 0.5,
    }
}

/// Convert an anchor Y position to the text block's top edge (base_y).
pub(crate) fn anchor_to_base_y(align_y: i32, anchor_y: f32, text_h: f32) -> f32 {
    match align_y {
        0 => anchor_y - text_h,
        1 => anchor_y - text_h * 0.5,
        2 => anchor_y,
        _ => anchor_y - text_h,
    }
}
