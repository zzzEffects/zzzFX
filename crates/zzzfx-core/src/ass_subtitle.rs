//! ASS/SSA subtitle parser and renderer.
//!
//! Parses `.ass` files via `ass-core`, resolves styles and override tags, then
//! rasterizes subtitle text using `ab_glyph` onto an RGBA8 output buffer.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use ab_glyph::Font;
use ass_core::{Script, Section};
use rayon::prelude::*;

use crate::blend::RECIP_255;
use crate::settings::ass_subtitle::AssBlendMode;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// Parsed ASS script with owned data.
pub struct AssScript {
    pub info: HashMap<String, String>,
    pub styles: Vec<OwnedStyle>,
    pub events: Vec<OwnedEvent>,
    pub play_res_x: Option<u32>,
    pub play_res_y: Option<u32>,
}

/// Fully-resolved style with parsed native fields.
#[derive(Debug, Clone)]
pub struct OwnedStyle {
    pub name: String,
    pub fontname: String,
    pub fontsize: f32,
    pub primary_color: [f32; 4],
    pub secondary_color: [f32; 4],
    pub outline_color: [f32; 4],
    pub back_color: [f32; 4],
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub scale_x: f32,
    pub scale_y: f32,
    pub spacing: f32,
    pub angle: f32,
    pub border_style: i32,
    pub outline: f32,
    pub shadow: f32,
    pub alignment: i32,
    pub margin_l: i32,
    pub margin_r: i32,
    pub margin_v: i32,
}

/// Fully-resolved event with parsed timing in milliseconds.
#[derive(Debug, Clone)]
pub struct OwnedEvent {
    pub layer: i32,
    pub start_ms: i64,
    pub end_ms: i64,
    pub style_name: String,
    pub name: String,
    pub margin_l: i32,
    pub margin_r: i32,
    pub margin_v: i32,
    pub effect: String,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedTags {
    // Font
    pub fontname: Option<String>,
    pub fontsize: Option<f32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strikeout: Option<bool>,

    // Color & alpha
    pub primary_color: Option<[f32; 4]>,
    pub secondary_color: Option<[f32; 4]>,
    pub outline_color: Option<[f32; 4]>,
    pub back_color: Option<[f32; 4]>,
    pub alpha: Option<f32>,

    // Scale & spacing
    pub scale_x: Option<f32>,
    pub scale_y: Option<f32>,
    pub spacing: Option<f32>,

    // Position & alignment
    pub alignment: Option<i32>,
    pub pos: Option<(f32, f32)>,
    pub org: Option<(f32, f32)>,
    pub move_: Option<MoveAnim>,

    // Rotation & shearing
    pub frz: Option<f32>,
    pub frx: Option<f32>,
    pub fry: Option<f32>,
    pub fax: Option<f32>,
    pub fay: Option<f32>,

    // Border, shadow, blur
    pub bord: Option<f32>,
    pub shad: Option<f32>,
    pub xbord: Option<f32>,
    pub ybord: Option<f32>,
    pub xshad: Option<f32>,
    pub yshad: Option<f32>,
    pub be: Option<f32>,
    pub blur: Option<f32>,

    // Clip
    pub clip: Option<ClipData>,
    pub iclip: Option<ClipData>,

    // Fade
    pub fade: Option<FadeData>,

    // Karaoke
    pub karaoke: Option<KaraokeData>,

    // Drawing
    pub drawing_scale: Option<f32>,

    // Reset
    pub reset: bool,
    pub reset_style: Option<String>,

    // Transform
    pub transforms: Vec<OverrideTransform>,
}

// ---- Supporting types ----

#[derive(Debug, Clone)]
pub struct MoveAnim {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub t1: Option<i64>,
    pub t2: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ClipData {
    pub inverse: bool,
    pub scale: Option<f32>,
    pub points: Vec<(f32, f32)>,
}

#[derive(Debug, Clone)]
pub struct FadeData {
    pub a1: f32,
    pub a2: f32,
    pub a3: f32,
    pub t1: i64,
    pub t2: i64,
    pub t3: i64,
    pub t4: i64,
    pub is_complex: bool,
}

#[derive(Debug, Clone)]
pub struct KaraokeData {
    pub duration_cs: i64,
    pub kind: KaraokeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KaraokeKind {
    Normal,
    Fill,
    Outline,
}

#[derive(Debug, Clone)]
pub struct OverrideTransform {
    pub start_t: i64,
    pub end_t: i64,
    pub acceleration: f32,
    pub tags: Box<ParsedTags>,
}

#[derive(Debug, Clone)]
pub struct TagSegment {
    pub text: String,
    pub tags: ParsedTags,
}

// ---------------------------------------------------------------------------
// Glyph cache — avoids re-rasterizing the same glyph every frame
// ---------------------------------------------------------------------------

/// Precomputed axis-aligned bounding box for clip region checks.
struct ClipCheck {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    inverse: bool,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct GlyphCacheKey {
    font_ptr: usize,
    glyph_id: u16,
    scale_x: u32,
    scale_y: u32,
    bold_x: u32,
}

/// Pre-rasterized glyph coverage data, reusable across frames and events.
#[derive(Clone)]
struct CachedGlyph {
    px_bounds_min_x: f32,
    px_bounds_min_y: f32,
    coverage: Vec<(u32, u32, f32)>,
}

/// Tracks the bounding box of rendered pixels for efficient compositing.
#[derive(Clone, Copy, Default)]
struct DirtyRect {
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
}

impl DirtyRect {
    fn expand(&mut self, x: i32, y: i32, pad: i32) {
        self.min_x = self.min_x.min(x - pad);
        self.min_y = self.min_y.min(y - pad);
        self.max_x = self.max_x.max(x + pad);
        self.max_y = self.max_y.max(y + pad);
    }

    fn clamp(mut self, w: i32, h: i32) -> Option<Self> {
        self.min_x = self.min_x.max(0);
        self.min_y = self.min_y.max(0);
        self.max_x = self.max_x.min(w - 1);
        self.max_y = self.max_y.min(h - 1);
        if self.min_x > self.max_x || self.min_y > self.max_y {
            None
        } else {
            Some(self)
        }
    }

}

/// Reusable per-frame rendering state to amortize allocations.
pub struct RenderCache {
    pub temp_buf: Vec<u8>,
    glyph_cache: HashMap<GlyphCacheKey, CachedGlyph>,
    font_data_cache: HashMap<usize, Option<Arc<Vec<u8>>>>,
    event_cache: HashMap<usize, CachedEventData>,
    prev_dirty: DirtyRect,
}

/// Precomputed per-event data that never changes frame-to-frame.
struct CachedEventData {
    segments: Vec<TagSegment>,
    clean_text: String,
    text_normalized: String,
}

impl RenderCache {
    pub fn new() -> Self {
        Self {
            temp_buf: Vec::new(),
            glyph_cache: HashMap::new(),
            font_data_cache: HashMap::new(),
            event_cache: HashMap::new(),
            prev_dirty: DirtyRect::default(),
        }
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Outline offset cache — reuses offset arrays for common (width, blur) pairs
// ---------------------------------------------------------------------------

static OUTLINE_CACHE: OnceLock<Mutex<HashMap<(i64, i64), Arc<[(f32, f32)]>>>> =
    OnceLock::new();

fn get_outline_offsets_cached(outline_w: f32, blur: f32) -> Arc<[(f32, f32)]> {
    let cache = OUTLINE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (
        (outline_w * 100.0).round() as i64,
        (blur * 100.0).round() as i64,
    );
    let mut guard = cache.lock().unwrap();
    if let Some(offsets) = guard.get(&key) {
        return Arc::clone(offsets);
    }
    let v = gen_outline_offsets(outline_w, blur);
    let a: Arc<[(f32, f32)]> = v.into();
    guard.insert(key, Arc::clone(&a));
    a
}

// ---------------------------------------------------------------------------
// ASS parser (ass-core based)
// ---------------------------------------------------------------------------

impl Default for OwnedStyle {
    fn default() -> Self {
        Self {
            name: String::from("Default"),
            fontname: String::from("Arial"),
            fontsize: 48.0,
            primary_color: [1.0, 1.0, 1.0, 1.0],
            secondary_color: [1.0, 0.0, 0.0, 1.0],
            outline_color: [0.0, 0.0, 0.0, 1.0],
            back_color: [0.0, 0.0, 0.0, 1.0],
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
            scale_x: 100.0,
            scale_y: 100.0,
            spacing: 0.0,
            angle: 0.0,
            border_style: 1,
            outline: 2.0,
            shadow: 2.0,
            alignment: 2,
            margin_l: 10,
            margin_r: 10,
            margin_v: 10,
        }
    }
}

impl Default for OwnedEvent {
    fn default() -> Self {
        Self {
            layer: 0,
            start_ms: 0,
            end_ms: 0,
            style_name: String::from("Default"),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: String::new(),
            text: String::new(),
        }
    }
}

/// Parse an ASS file from a string using `ass-core`.
pub fn parse_ass_file(content: &str) -> Result<AssScript, String> {
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    let script = Script::parse(content).map_err(|e| format!("ASS parse error: {e}"))?;

    let mut info: HashMap<String, String> = HashMap::new();
    let mut styles: Vec<OwnedStyle> = Vec::new();
    let mut events: Vec<OwnedEvent> = Vec::new();
    let mut play_res_x: Option<u32> = None;
    let mut play_res_y: Option<u32> = None;

    for section in script.sections() {
        match section {
            Section::ScriptInfo(si) => {
                for (k, v) in &si.fields {
                    let key = *k;
                    let val = *v;
                    info.insert(key.to_string(), val.to_string());
                    if key.eq_ignore_ascii_case("PlayResX") {
                        play_res_x = val.parse().ok();
                    } else if key.eq_ignore_ascii_case("PlayResY") {
                        play_res_y = val.parse().ok();
                    }
                }
            }
            Section::Styles(ass_styles) => {
                for s in ass_styles {
                    styles.push(convert_style(s));
                }
            }
            Section::Events(ass_events) => {
                for e in ass_events {
                    if e.is_dialogue() {
                        events.push(convert_event(e));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(AssScript { info, styles, events, play_res_x, play_res_y })
}

fn convert_style(s: &ass_core::parser::Style<'_>) -> OwnedStyle {
    OwnedStyle {
        name: s.name.to_string(),
        fontname: s.fontname.to_string(),
        fontsize: s.fontsize.parse().unwrap_or(48.0),
        primary_color: ass_color_to_rgba(s.primary_colour),
        secondary_color: ass_color_to_rgba(s.secondary_colour),
        outline_color: ass_color_to_rgba(s.outline_colour),
        back_color: ass_color_to_rgba(s.back_colour),
        bold: s.bold == "-1" || s.bold == "1",
        italic: s.italic == "-1" || s.italic == "1",
        underline: s.underline == "-1" || s.underline == "1",
        strikeout: s.strikeout == "-1" || s.strikeout == "1",
        scale_x: s.scale_x.parse().unwrap_or(100.0),
        scale_y: s.scale_y.parse().unwrap_or(100.0),
        spacing: s.spacing.parse().unwrap_or(0.0),
        angle: s.angle.parse().unwrap_or(0.0),
        border_style: s.border_style.parse().unwrap_or(1),
        outline: s.outline.parse().unwrap_or(2.0),
        shadow: s.shadow.parse().unwrap_or(2.0),
        alignment: s.alignment.parse().unwrap_or(2),
        margin_l: s.margin_l.parse().unwrap_or(10),
        margin_r: s.margin_r.parse().unwrap_or(10),
        margin_v: s.margin_v.parse().unwrap_or(10),
    }
}

fn convert_event(e: &ass_core::parser::Event<'_>) -> OwnedEvent {
    let start_ms = e.start_time_cs().unwrap_or(0) as i64 * 10;
    let end_ms = e.end_time_cs().unwrap_or(0) as i64 * 10;
    OwnedEvent {
        layer: e.layer.parse().unwrap_or(0),
        start_ms,
        end_ms,
        style_name: e.style.to_string(),
        name: e.name.to_string(),
        margin_l: e.margin_l.parse().unwrap_or(0),
        margin_r: e.margin_r.parse().unwrap_or(0),
        margin_v: e.margin_v.parse().unwrap_or(0),
        effect: e.effect.to_string(),
        text: e.text.to_string(),
    }
}

/// Parse ASS color `&HAABBGGRR` → `[r, g, b, a]` normalized 0..1.
/// ASS alpha is inverted: `&H00` = opaque, `&HFF` = transparent.
/// Uses `ass_core::utils::parse_bgr_color` and inverts the alpha byte.
pub fn ass_color_to_rgba(hex: &str) -> [f32; 4] {
    match ass_core::utils::parse_bgr_color(hex) {
        Ok([r, g, b, a_byte]) => {
            let a = 1.0 - a_byte as f32 * RECIP_255;
            [r as f32 * RECIP_255, g as f32 * RECIP_255, b as f32 * RECIP_255, a]
        }
        Err(_) => [1.0, 1.0, 1.0, 1.0],
    }
}

// Keep the old name as an alias for backward compatibility in tests.
// (Will be removed once tests are updated.)
#[allow(dead_code)]
fn parse_ass_color(hex: &str) -> [f32; 4] {
    ass_color_to_rgba(hex)
}

/// Parse ASS time `H:MM:SS.cc` → milliseconds.
/// Uses `ass_core::utils::parse_ass_time` (returns centiseconds) and converts to ms.
#[allow(dead_code)]
fn parse_ass_time(s: &str) -> i64 {
    ass_core::utils::parse_ass_time(s).unwrap_or(0) as i64 * 10
}

// ---------------------------------------------------------------------------
// Override tag parser
// ---------------------------------------------------------------------------

/// Parse ASS override tags from dialogue text.
/// Returns (clean_text, parsed_tags).
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
pub fn parse_tag_segments(raw_text: &str) -> Vec<TagSegment> {
    let mut segments = Vec::new();
    let mut current_tags = ParsedTags::default();
    let mut current_text = String::new();

    let mut chars = raw_text.char_indices().peekable();
    while let Some((byte_pos, ch)) = chars.next() {
        if ch == '{' {
            let remaining = &raw_text[byte_pos..];
            if let Some(tag_end) = remaining.find('}') {
                // Flush accumulated text with current tags before processing new tag block
                if !current_text.is_empty() {
                    segments.push(TagSegment { text: current_text.clone(), tags: current_tags.clone() });
                    current_text.clear();
                }
                let tag_str = &remaining[1..tag_end];
                parse_tag_block(tag_str, &mut current_tags);
                // Skip all chars inside the tag block
                let skip_end = byte_pos + tag_end + 1;
                while let Some((pos, _)) = chars.peek() {
                    if *pos < skip_end { chars.next(); } else { break; }
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

fn parse_tag_block(block: &str, tags: &mut ParsedTags) {
    let mut rest = block;
    while !rest.is_empty() {
        let Some(stripped) = rest.strip_prefix('\\') else { break; };
        rest = stripped;

        // Tag name: alphabetic chars
        let name_end = rest.find(|c: char| !c.is_alphabetic()).unwrap_or(rest.len());
        if name_end == 0 { break; }
        let name = &rest[..name_end];
        rest = &rest[name_end..];

        // Parse value: parens, or numeric/hex until next backslash
        let value: Option<&str> = if rest.starts_with('(') {
            // Find matching close paren (handle nested parens for \t)
            let mut depth = 1u32;
            let mut close = 0usize;
            for (j, ch) in rest.char_indices().skip(1) {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 { close = j; break; }
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
            "fn" => if let Some(v) = value { tags.fontname = Some(v.to_string()); },
            "fs" => if let Some(v) = value { tags.fontsize = v.parse().ok(); },
            "b" => parse_bool_tag(value, &mut tags.bold),
            "i" => parse_bool_tag(value, &mut tags.italic),
            "u" => parse_bool_tag(value, &mut tags.underline),
            "s" => parse_bool_tag(value, &mut tags.strikeout),
            "fsp" => if let Some(v) = value { tags.spacing = v.parse().ok(); },

            // Colors
            "c" | "1c" => if let Some(v) = value { tags.primary_color = Some(ass_color_to_rgba(v)); },
            "2c" => if let Some(v) = value { tags.secondary_color = Some(ass_color_to_rgba(v)); },
            "3c" => if let Some(v) = value { tags.outline_color = Some(ass_color_to_rgba(v)); },
            "4c" => if let Some(v) = value { tags.back_color = Some(ass_color_to_rgba(v)); },

            // Alpha
            "alpha" => if let Some(v) = value { tags.alpha = parse_alpha_hex(v); },
            "1a" => if let Some(v) = value { apply_alpha_channel(tags, 1, v); },
            "2a" => if let Some(v) = value { apply_alpha_channel(tags, 2, v); },
            "3a" => if let Some(v) = value { apply_alpha_channel(tags, 3, v); },
            "4a" => if let Some(v) = value { apply_alpha_channel(tags, 4, v); },

            // Scale
            "fscx" => if let Some(v) = value { tags.scale_x = v.parse().ok(); },
            "fscy" => if let Some(v) = value { tags.scale_y = v.parse().ok(); },

            // Alignment
            "an" => if let Some(v) = value { tags.alignment = v.parse().ok(); },
            "a" => if let Some(v) = value {
                let legacy: i32 = v.parse().unwrap_or(2);
                tags.alignment = Some(legacy_to_an(legacy));
            },

            // Position
            "pos" => if let Some(v) = value {
                let coords: Vec<&str> = v.split(',').collect();
                if coords.len() >= 2 {
                    tags.pos = Some((coords[0].trim().parse().unwrap_or(0.0), coords[1].trim().parse().unwrap_or(0.0)));
                }
            },
            "org" => if let Some(v) = value {
                let parts: Vec<&str> = v.split(',').collect();
                if parts.len() >= 2 {
                    tags.org = Some((parts[0].trim().parse().unwrap_or(0.0), parts[1].trim().parse().unwrap_or(0.0)));
                }
            },
            "move" => if let Some(v) = value {
                let parts: Vec<&str> = v.split(',').collect();
                if parts.len() >= 4 {
                    let x1: f32 = parts[0].trim().parse().unwrap_or(0.0);
                    let y1: f32 = parts[1].trim().parse().unwrap_or(0.0);
                    let x2: f32 = parts[2].trim().parse().unwrap_or(0.0);
                    let y2: f32 = parts[3].trim().parse().unwrap_or(0.0);
                    let t1 = parts.get(4).and_then(|s| s.trim().parse().ok()).map(|ms: i64| ms);
                    let t2 = parts.get(5).and_then(|s| s.trim().parse().ok()).map(|ms: i64| ms);
                    tags.move_ = Some(MoveAnim { x1, y1, x2, y2, t1, t2 });
                }
            },

            // Rotation
            "frz" => if let Some(v) = value { tags.frz = v.parse().ok(); },
            "frx" => if let Some(v) = value { tags.frx = v.parse().ok(); },
            "fry" => if let Some(v) = value { tags.fry = v.parse().ok(); },

            // Shearing
            "fax" => if let Some(v) = value { tags.fax = v.parse().ok(); },
            "fay" => if let Some(v) = value { tags.fay = v.parse().ok(); },

            // Border / shadow
            "bord" => if let Some(v) = value { let val: f32 = v.parse().unwrap_or(0.0); tags.bord = Some(val); tags.xbord = Some(val); tags.ybord = Some(val); },
            "xbord" => if let Some(v) = value { tags.xbord = v.parse().ok(); },
            "ybord" => if let Some(v) = value { tags.ybord = v.parse().ok(); },
            "shad" => if let Some(v) = value { let val: f32 = v.parse().unwrap_or(0.0); tags.shad = Some(val); tags.xshad = Some(val); tags.yshad = Some(val); },
            "xshad" => if let Some(v) = value { tags.xshad = v.parse().ok(); },
            "yshad" => if let Some(v) = value { tags.yshad = v.parse().ok(); },

            // Blur
            "be" | "be1" => if let Some(v) = value { tags.be = v.parse().ok(); },
            "blur" => if let Some(v) = value { tags.blur = v.parse().ok(); },

            // Clip
            "clip" => if let Some(v) = value { tags.clip = Some(parse_clip(v, true)); },
            "iclip" => if let Some(v) = value { tags.iclip = Some(parse_clip(v, true)); },

            // Fade
            "fad" => if let Some(v) = value {
                let parts: Vec<&str> = v.split(',').collect();
                if parts.len() >= 2 {
                    tags.fade = Some(FadeData {
                        a1: 0.0, a2: 1.0, a3: 1.0,
                        t1: parts[0].trim().parse().unwrap_or(0),
                        t2: parts[1].trim().parse().unwrap_or(0),
                        t3: 0, t4: 0,
                        is_complex: false,
                    });
                }
            },
            "fade" => if let Some(v) = value {
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
            },

            // Karaoke
            "k" | "K" => if let Some(v) = value {
                tags.karaoke = Some(KaraokeData { duration_cs: v.parse().unwrap_or(0), kind: KaraokeKind::Normal });
            },
            "kf" | "KF" => if let Some(v) = value {
                tags.karaoke = Some(KaraokeData { duration_cs: v.parse().unwrap_or(0), kind: KaraokeKind::Fill });
            },
            "ko" | "KO" => if let Some(v) = value {
                tags.karaoke = Some(KaraokeData { duration_cs: v.parse().unwrap_or(0), kind: KaraokeKind::Outline });
            },

            // Drawing
            "p" => {
                let scale: f32 = value.and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
                tags.drawing_scale = if scale > 0.0 { Some(scale) } else { None };
            },

            // Reset
            "r" => {
                tags.reset = true;
                tags.reset_style = value.map(|v| v.trim().to_string());
            },

            // Transform \t(accel,tags) or \t(t1,t2,tags) or \t(t1,t2,accel,tags)
            "t" => if let Some(v) = value {
                if let Some(t) = parse_transform(v) {
                    tags.transforms.push(t);
                }
            },

            _ => {}
        }
    }
}

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
    let hex = v.trim().trim_start_matches("&H").trim_start_matches("&h");
    if let Ok(val) = u32::from_str_radix(hex, 16) {
        let a_byte = (val & 0xFF) as u8;
        Some(1.0 - a_byte as f32 * RECIP_255)
    } else {
        None
    }
}

fn parse_alpha_str(s: &str) -> f32 {
    s.parse::<f32>().ok().map(|v| v * RECIP_255).unwrap_or_else(|| parse_alpha_hex(s).unwrap_or(1.0))
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

fn parse_clip(v: &str, _rect_check: bool) -> ClipData {
    let parts: Vec<&str> = v.split(',').map(|s| s.trim()).collect();
    if parts.is_empty() { return ClipData { inverse: false, scale: None, points: vec![] }; }

    // Rectangular clip: x1,y1,x2,y2  (4 parts, no scale)
    if parts.len() == 4 && parts[0].parse::<f32>().map_or(false, |_| true) {
        let x1: f32 = parts[0].parse().unwrap_or(0.0);
        let y1: f32 = parts[1].parse().unwrap_or(0.0);
        let x2: f32 = parts[2].parse().unwrap_or(0.0);
        let y2: f32 = parts[3].parse().unwrap_or(0.0);
        return ClipData {
            inverse: false,
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
    ClipData { inverse: false, scale: Some(scale), points }
}

fn parse_transform(v: &str) -> Option<OverrideTransform> {
    let parts: Vec<&str> = v.split(',').collect();
    if parts.is_empty() { return None; }

    // Find where the tag part starts (first backslash)
    let tag_start = v.find('\\').unwrap_or(v.len());
    let before_tags = &v[..tag_start];
    let before_parts: Vec<&str> = before_tags.split(',').filter(|s| !s.trim().is_empty()).collect();

    match before_parts.len() {
        // \t(accel,\tags...)
        1 => {
            let accel: f32 = before_parts[0].trim().parse().unwrap_or(1.0);
            let mut tags = ParsedTags::default();
            if tag_start < v.len() {
                parse_tag_block(&v[tag_start..], &mut tags);
            }
            Some(OverrideTransform { start_t: 0, end_t: 0, acceleration: accel, tags: Box::new(tags) })
        }
        // \t(t1,t2,\tags...) or \t(t1,t2,accel,\tags...)
        2 | 3 => {
            let t1: i64 = before_parts[0].trim().parse().unwrap_or(0);
            let t2: i64 = before_parts[1].trim().parse().unwrap_or(0);
            let accel: f32 = before_parts.get(2).and_then(|s| s.trim().parse().ok()).unwrap_or(1.0);
            let mut tags = ParsedTags::default();
            if tag_start < v.len() {
                parse_tag_block(&v[tag_start..], &mut tags);
            }
            Some(OverrideTransform { start_t: t1, end_t: t2, acceleration: accel, tags: Box::new(tags) })
        }
        _ => None,
    }
}

/// Convert legacy ASS alignment (1-9) to ASS+ alignment (1-9 bottom-to-top).
fn legacy_to_an(legacy: i32) -> i32 {
    match legacy {
        1 => 1, 2 => 2, 3 => 3,
        4 => 7, 5 => 8, 6 => 9,
        7 => 4, 8 => 5, 9 => 6,
        _ => 2,
    }
}

// ---------------------------------------------------------------------------
// Font cache (font-kit based)
// ---------------------------------------------------------------------------

use font_kit::source::SystemSource;
use font_kit::handle::Handle as FkHandle;
use std::cell::RefCell;

struct FontEntry {
    family_name: String,
    full_name: String,
    postscript_name: String,
    handle: FkHandle,
}

pub struct FontCache {
    entries: Vec<FontEntry>,
    loaded: RefCell<HashMap<String, Arc<Vec<u8>>>>,
}

impl FontCache {
    pub fn new() -> Self {
        let source = SystemSource::new();
        let mut entries = Vec::new();
        let handles = source.all_fonts().unwrap_or_default();
        for handle in handles {
            if let Ok(font) = handle.load() {
                entries.push(FontEntry {
                    family_name: font.family_name().to_lowercase(),
                    full_name: font.full_name().to_lowercase(),
                    postscript_name: font.postscript_name().unwrap_or_default().to_lowercase(),
                    handle,
                });
            }
        }
        Self { entries, loaded: RefCell::new(HashMap::new()) }
    }

    /// Try to match a font name against known variants (exact match).
    fn matches_name_exact(entry: &FontEntry, q: &str) -> bool {
        entry.family_name == q || entry.full_name == q || entry.postscript_name == q
    }

    /// Lazy-load raw font data for an entry via interior mutability.
    fn load_font_data(&self, entry: &FontEntry) -> Option<Arc<Vec<u8>>> {
        let key = &entry.full_name;
        {
            let loaded = self.loaded.borrow();
            if let Some(data) = loaded.get(key) { return Some(Arc::clone(data)); }
        }
        if let Ok(font) = entry.handle.load() {
            if let Some(data) = font.copy_font_data() {
                self.loaded.borrow_mut().insert(key.clone(), Arc::clone(&data));
                return Some(data);
            }
        }
        None
    }

    /// Look up entries matching a font name query, returning indices.
    fn find_matching_indices(&self, font_name: &str) -> Vec<usize> {
        let q = font_name.to_lowercase();
        let mut indices = Vec::new();
        // Tier 1: exact match on any name variant
        for (i, entry) in self.entries.iter().enumerate() {
            if Self::matches_name_exact(entry, &q) { indices.push(i); }
        }
        if !indices.is_empty() { return indices; }
        // Tier 2: substring match
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.family_name.contains(&q) || entry.full_name.contains(&q) { indices.push(i); }
        }
        if !indices.is_empty() { return indices; }
        // Tier 3: prefix match
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.family_name.starts_with(&q) || entry.full_name.starts_with(&q) { indices.push(i); }
        }
        indices
    }

    pub fn find_font(&self, font_name: &str) -> Option<Arc<Vec<u8>>> {
        for idx in self.find_matching_indices(font_name) {
            if let Some(data) = self.load_font_data(&self.entries[idx]) {
                return Some(data);
            }
        }
        None
    }

    pub fn find_font_for_chars(&self, chars: &[char], preferred_name: &str) -> Option<Arc<Vec<u8>>> {
        if chars.is_empty() { return None; }
        let q = preferred_name.to_lowercase();
        // Try preferred name first
        for entry in &self.entries {
            if Self::matches_name_exact(entry, &q) {
                if let Some(data) = self.load_font_data(entry) {
                    if let Ok(font) = entry.handle.load() {
                        if chars.iter().all(|&c| font.glyph_for_char(c).is_some()) {
                            return Some(data);
                        }
                    }
                }
                break;
            }
        }
        // Scan all fonts for best coverage
        let mut best: Option<(Arc<Vec<u8>>, usize)> = None;
        for entry in &self.entries {
            if let Ok(font) = entry.handle.load() {
                let covered = chars.iter().filter(|&&c| font.glyph_for_char(c).is_some()).count();
                if covered == chars.len() {
                    return self.load_font_data(entry);
                }
                match &best {
                    None => { if let Some(d) = self.load_font_data(entry) { best = Some((d, covered)); } }
                    Some((_, prev)) if covered > *prev => {
                        if let Some(d) = self.load_font_data(entry) { best = Some((d, covered)); }
                    }
                    _ => {}
                }
            }
        }
        best.map(|(d, _)| d)
    }

    pub fn find_font_for_char(&self, ch: char, preferred_name: &str) -> Option<Arc<Vec<u8>>> {
        let q = preferred_name.to_lowercase();
        for entry in &self.entries {
            if Self::matches_name_exact(entry, &q) {
                if let Ok(font) = entry.handle.load() {
                    if font.glyph_for_char(ch).is_some() {
                        return self.load_font_data(entry);
                    }
                }
            }
        }
        for entry in &self.entries {
            if let Ok(font) = entry.handle.load() {
                if font.glyph_for_char(ch).is_some() {
                    return self.load_font_data(entry);
                }
            }
        }
        None
    }

    pub fn find_fonts_for_chars_grouped(
        &self,
        chars: &[char],
        preferred_name: &str,
    ) -> HashMap<String, Option<Arc<Vec<u8>>>> {
        let mut groups: HashMap<String, Vec<char>> = HashMap::new();
        for &c in chars {
            groups.entry(script_group(c).to_string()).or_default().push(c);
        }
        let mut result = HashMap::new();
        for (script, group_chars) in &groups {
            let font = self.find_font_for_chars(group_chars, preferred_name);
            result.insert(script.clone(), font);
        }
        result
    }

    pub fn list_font_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.entries.iter().map(|e| e.full_name.clone()).collect();
        names.sort();
        names.dedup();
        names
    }
}

/// Classify a character into a Unicode script group for font fallback purposes.
fn script_group(c: char) -> &'static str {
    if c <= '\u{007F}' { return "Latin"; }
    if ('\u{4E00}'..='\u{9FFF}').contains(&c) { return "CJK"; }
    if ('\u{3400}'..='\u{4DBF}').contains(&c) { return "CJK"; }
    if ('\u{F900}'..='\u{FAFF}').contains(&c) { return "CJK"; }
    if ('\u{3040}'..='\u{309F}').contains(&c) { return "Hiragana"; }
    if ('\u{30A0}'..='\u{30FF}').contains(&c) { return "Katakana"; }
    if ('\u{AC00}'..='\u{D7AF}').contains(&c) { return "Hangul"; }
    if ('\u{0600}'..='\u{06FF}').contains(&c) { return "Arabic"; }
    if ('\u{0E00}'..='\u{0E7F}').contains(&c) { return "Thai"; }
    if ('\u{0400}'..='\u{04FF}').contains(&c) { return "Cyrillic"; }
    if ('\u{0370}'..='\u{03FF}').contains(&c) { return "Greek"; }
    if ('\u{0590}'..='\u{05FF}').contains(&c) { return "Hebrew"; }
    if ('\u{0900}'..='\u{097F}').contains(&c) { return "Devanagari"; }
    "Other"
}

// ---------------------------------------------------------------------------
// Blend modes for a single pixel (premultiplied RGBA values in 0..1)
// ---------------------------------------------------------------------------

fn blend_pixel(mode: AssBlendMode, src: [f32; 4], dst: [f32; 4]) -> [f32; 4] {
    match mode {
        AssBlendMode::Normal => {
            let out_a = src[3] + dst[3] * (1.0 - src[3]);
            if out_a < 0.001 {
                return [0.0, 0.0, 0.0, 0.0];
            }
            let out_r = (src[0] + dst[0] * (1.0 - src[3])) / out_a;
            let out_g = (src[1] + dst[1] * (1.0 - src[3])) / out_a;
            let out_b = (src[2] + dst[2] * (1.0 - src[3])) / out_a;
            [out_r, out_g, out_b, out_a]
        }
        AssBlendMode::Add => {
            let r = (src[0] + dst[0]).min(1.0);
            let g = (src[1] + dst[1]).min(1.0);
            let b = (src[2] + dst[2]).min(1.0);
            let a = (src[3] + dst[3]).min(1.0);
            [r, g, b, a]
        }
        AssBlendMode::Screen => {
            let r = 1.0 - (1.0 - src[0]) * (1.0 - dst[0]);
            let g = 1.0 - (1.0 - src[1]) * (1.0 - dst[1]);
            let b = 1.0 - (1.0 - src[2]) * (1.0 - dst[2]);
            let a = 1.0 - (1.0 - src[3]) * (1.0 - dst[3]);
            [r, g, b, a]
        }
        AssBlendMode::Multiply => {
            [src[0] * dst[0], src[1] * dst[1], src[2] * dst[2], src[3] * dst[3]]
        }
        AssBlendMode::Overlay => {
            let overlay_ch = |s: f32, d: f32| -> f32 {
                if d < 0.5 { 2.0 * d * s } else { 1.0 - 2.0 * (1.0 - d) * (1.0 - s) }
            };
            [
                overlay_ch(src[0], dst[0]),
                overlay_ch(src[1], dst[1]),
                overlay_ch(src[2], dst[2]),
                overlay_ch(src[3], dst[3]),
            ]
        }
    }
}

// ---------------------------------------------------------------------------
// Public render API
// ---------------------------------------------------------------------------

/// Diagnostic stats returned by the renderer.
#[derive(Debug, Default, Clone)]
pub struct RenderStats {
    pub events_total: usize,
    pub events_active: usize,
    pub font_found: bool,
    pub font_parsed: bool,
    pub text_char_count: usize,
    pub glyph_rasterize_attempts: usize,
    pub glyph_rasterize_ok: usize,
    pub pixels_written: usize,
    pub max_blur_radius: f32,
}

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
    if active.is_empty() { return stats; }

    // Compute PlayRes-to-output scaling factors.
    let res_scale_x = if use_native_size {
        ass_script.play_res_x.map_or(1.0, |prx| if prx > 0 { output_width as f32 / prx as f32 } else { 1.0 })
    } else { 1.0 };
    let res_scale_y = if use_native_size {
        ass_script.play_res_y.map_or(1.0, |pry| if pry > 0 { output_height as f32 / pry as f32 } else { 1.0 })
    } else { 1.0 };

    // Reuse temp_buf — reallocate only when the output size changes
    let buf_size = output_width * output_height * 4;
    if cache.temp_buf.len() != buf_size {
        cache.temp_buf.resize(buf_size, 0);
    }
    // Clear only the region dirtied in the previous frame
    let prev = cache.prev_dirty;
    {
        let buf_len = cache.temp_buf.len();
        if prev.min_x < prev.max_x && prev.min_y < prev.max_y {
            let w = output_width;
            for py in prev.min_y.max(0)..=prev.max_y.min(output_height as i32 - 1) {
                let row_start = py as usize * w * 4 + prev.min_x.max(0) as usize * 4;
                let row_end = py as usize * w * 4 + (prev.max_x.min(output_width as i32 - 1) as usize + 1) * 4;
                cache.temp_buf[row_start..row_end.min(buf_len)].fill(0);
            }
        } else if cache.prev_dirty.min_x == 0 && cache.prev_dirty.max_x == 0
            && cache.prev_dirty.min_y == 0 && cache.prev_dirty.max_y == 0
        {
            // First frame with this cache: full clear
            cache.temp_buf.fill(0);
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
    let glyph_cache = &mut cache.glyph_cache;
    let event_cache = &mut cache.event_cache;
    let font_data_cache = &mut cache.font_data_cache;

    for ev in &active {
        let style = resolve_style(ass_script, ev);

        // Cached event data: segments, clean_text, text_normalized
        let ev_key = *ev as *const OwnedEvent as usize;
        let cached_ev = event_cache.entry(ev_key).or_insert_with(|| {
            let segments = parse_tag_segments(&ev.text);
            let clean_text: String = segments.iter().map(|s| s.text.as_str()).collect();
            let text_normalized = clean_text.replace("\\n", "\\N");
            CachedEventData { segments, clean_text, text_normalized }
        });
        let segments = &cached_ev.segments;
        let clean_text: &str = &cached_ev.clean_text;
        let text_normalized: &str = &cached_ev.text_normalized;

        // Use the last segment's tags for "global" properties, then apply \t transforms
        let base_tags = segments.last().map(|s| &s.tags).cloned().unwrap_or_default();
        let inline_tags = apply_transforms(time_ms, ev.start_ms, ev.end_ms,
            &base_tags.transforms, &base_tags);

        // Build char-to-segment lookup for per-character properties
        let mut char_to_seg: Vec<usize> = Vec::with_capacity(clean_text.len());
        for (si, seg) in segments.iter().enumerate() {
            for _ in seg.text.chars() {
                char_to_seg.push(si);
            }
        }

        // Merge inline tags with style, applying font override if set
        let base_fontname = inline_tags.fontname.as_deref().unwrap_or(&style.fontname);
        let fontname = font_override
            .filter(|s| !s.is_empty())
            .and_then(|ov| if font_cache.find_font(ov).is_some() { Some(ov) } else { None })
            .unwrap_or(base_fontname);
        let fontsize = inline_tags.fontsize.unwrap_or(style.fontsize);
        let fill_color = inline_tags.primary_color.unwrap_or(style.primary_color);
        let alignment = inline_tags.alignment.unwrap_or(style.alignment);

        // Outline/shadow from tags, fall back to style (scaled by res_scale_y for PlayRes mapping)
        let outline_w = (inline_tags.bord.or(inline_tags.xbord).unwrap_or(style.outline)) * res_scale_y;
        let shadow_dx = (inline_tags.xshad.unwrap_or(style.shadow)) * res_scale_x;
        let shadow_dy = (inline_tags.yshad.unwrap_or(style.shadow)) * res_scale_y;

        // Accumulate blur radius for post-render blur pass
        let blur_radius = inline_tags.blur.or(inline_tags.be).unwrap_or(0.0);
        stats.max_blur_radius = stats.max_blur_radius.max(blur_radius);

        // Effective clip with precomputed bounding box
        let effective_clip: Option<ClipData> = inline_tags.clip.clone().or_else(|| inline_tags.iclip.clone().map(|mut c| { c.inverse = true; c }));
        let clip_check: Option<ClipCheck> = effective_clip.as_ref().and_then(|clip| {
            let pts = &clip.points;
            if pts.len() < 4 { return None; }
            let (mut x1, mut y1) = (f32::MAX, f32::MAX);
            let (mut x2, mut y2) = (f32::MIN, f32::MIN);
            for p in pts {
                x1 = x1.min(p.0 * res_scale_x);
                y1 = y1.min(p.1 * res_scale_y);
                x2 = x2.max(p.0 * res_scale_x);
                y2 = y2.max(p.1 * res_scale_y);
            }
            Some(ClipCheck { x1, y1, x2, y2, inverse: clip.inverse })
        });
        let outline_color = inline_tags.outline_color.unwrap_or(style.outline_color);
        let shadow_color = inline_tags.back_color.unwrap_or(style.back_color);
        let border_style = style.border_style;

        // Scale from tags on top of style
        let tag_sx = inline_tags.scale_x.unwrap_or(style.scale_x) / 100.0;
        let tag_sy = inline_tags.scale_y.unwrap_or(style.scale_y) / 100.0;

        // Fade
        let fade_alpha = compute_fade_alpha(time_ms, ev.start_ms, ev.end_ms, &inline_tags.fade);

        // Movement
        let move_pos = compute_move_pos(time_ms, ev.start_ms, ev.end_ms, &inline_tags.move_);

        stats.text_char_count = clean_text.chars().count();

        // Cached font lookup: avoid scanning all system fonts every frame
        let font_data = if let Some(cached) = font_data_cache.get(&ev_key) {
            stats.font_found = cached.is_some();
            cached.clone()
        } else {
            let text_chars: Vec<char> = clean_text.chars().filter(|c| !c.is_whitespace()).collect();
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


        let px_scale_x = fontsize * res_scale_y * font_scale_x.max(0.01) * tag_sx;
        let px_scale_y = fontsize * res_scale_y * font_scale_y.max(0.01) * tag_sy;
        let px_scale = ab_glyph::PxScale { x: px_scale_x, y: px_scale_y };
        let units_per_em = font.units_per_em().unwrap_or(1000.0);

        // Use actual font metrics for line height and baseline positioning
        let ascent = font.ascent_unscaled() * px_scale_y / units_per_em;
        let descent = font.descent_unscaled() * px_scale_y / units_per_em;
        let line_gap = font.line_gap_unscaled() * px_scale_y / units_per_em;
        let line_height = ascent - descent + line_gap;

        // Spacing from inline tags (raw value without overall scale)
        let spacing_raw = inline_tags.spacing.unwrap_or(style.spacing);
        let spacing_adv = spacing_raw * res_scale_x * px_scale.x / units_per_em * scale;

        let lines: Vec<&str> = text_normalized.split("\\N").collect();

        // Phase 1: Layout — compute line widths, store per-char glyph info
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
        for line in &lines {
            let mut line_w: f32 = 0.0;
            let mut char_count = 0usize;
            let mut chars = Vec::new();
            for c in line.chars() {
                let glyph_id = font.glyph_id(c);
                let glyph_w = font.h_advance_unscaled(glyph_id) * px_scale.x / units_per_em;
                chars.push(CharLayout { glyph_id, h_adv_px: glyph_w });
                line_w += glyph_w;
                char_count += 1;
            }
            if char_count > 0 {
                line_w += spacing_raw * res_scale_x * px_scale.x / units_per_em * (char_count - 1) as f32;
            }
            max_line_width = max_line_width.max(line_w);
            total_height += line_height;
            line_layouts.push(LineLayout { chars, width_before_scale: line_w });
        }

        let text_w = max_line_width * scale;
        let text_h = total_height * scale;

        let (align_x, align_y) = alignment_to_anchor(alignment);

        let margin_l = (if ev.margin_l != 0 { ev.margin_l } else { style.margin_l }) as f32 * scale * res_scale_x;
        let margin_r = (if ev.margin_r != 0 { ev.margin_r } else { style.margin_r }) as f32 * scale * res_scale_x;
        let margin_v = (if ev.margin_v != 0 { ev.margin_v } else { style.margin_v }) as f32 * scale * res_scale_y;

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

        // Apply \pos or \move override, then position setting
        let (base_x, base_y) = if let Some((px, py)) = inline_tags.pos {
            (px * scale * res_scale_x + (position_x - 0.5) * w, py * scale * res_scale_y + (position_y - 0.5) * h)
        } else if let Some((mx, my)) = move_pos {
            (mx * scale * res_scale_x + (position_x - 0.5) * w, my * scale * res_scale_y + (position_y - 0.5) * h)
        } else {
            (base_x, base_y)
        };

        // Precompute outline offset directions (cached)
        let outline_offsets: Arc<[(f32, f32)]> = if outline_w > 0.0 && border_style == 1 {
            get_outline_offsets_cached(outline_w, blur_radius)
        } else {
            Arc::new([])
        };

        // Phase 2: Glyph rendering with coverage cache
        let font_ptr = Arc::as_ptr(&font_data) as usize;
        let has_explicit_pos = inline_tags.pos.is_some() || inline_tags.move_.is_some();
        let first_baseline_y = if has_explicit_pos {
            base_y
        } else {
            base_y + ascent * scale
        };
        let mut cursor_y = first_baseline_y;
        for (line, layout) in lines.iter().zip(line_layouts.iter()) {
            if cursor_y + line_height * scale <= 0.0 || cursor_y >= h {
                cursor_y += line_height * scale;
                continue;
            }

            let line_x = match align_x {
                0 => base_x,
                1 => base_x + (text_w - layout.width_before_scale * scale) / 2.0,
                2 => base_x + text_w - layout.width_before_scale * scale,
                _ => base_x,
            };

            let mut cursor_x = line_x;
            let mut char_idx = 0usize;
            for (_c, ch_layout) in line.chars().zip(layout.chars.iter()) {

                let seg_tags = if char_idx < char_to_seg.len() {
                    &segments[char_to_seg[char_idx]].tags
                } else {
                    &inline_tags
                };
                let ch_fill_color = seg_tags.primary_color.unwrap_or(fill_color);
                let ch_bold = seg_tags.bold.unwrap_or(style.bold);
                let ch_italic = seg_tags.italic.unwrap_or(style.italic);

                let use_glyph_id = ch_layout.glyph_id;
                let use_font = &font;

                let h_adv_px = ch_layout.h_adv_px * scale;
                let sb_px = use_font.h_side_bearing_unscaled(use_glyph_id) * px_scale.x / units_per_em * scale;
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
                        let glyph = ab_glyph::Glyph {
                            id: use_glyph_id,
                            scale: use_scale,
                            position: ab_glyph::point(0.0, 0.0),
                        };
                        if let Some(outline) = use_font.outline_glyph(glyph) {
                            let px_bounds = outline.px_bounds();
                            let mut coverage = Vec::new();
                            outline.draw(|gx, gy, cov| {
                                coverage.push((gx, gy, cov));
                            });
                            glyph_cache.insert(cache_key.clone(), CachedGlyph {
                                px_bounds_min_x: px_bounds.min.x,
                                px_bounds_min_y: px_bounds.min.y,
                                coverage,
                            });
                        }
                    }

                    if let Some(cached) = glyph_cache.get(&cache_key) {
                        stats.glyph_rasterize_ok += 1;
                        let mut local_pixels = 0usize;
                        let slant_offset = |gy: f32| -> f32 { gy * italic_shear };

                        let oo = &outline_offsets;
                        let out_color = apply_fade_to_color(outline_color, fade_alpha);

                        // Shadow pass
                        if shadow_dx != 0.0 || shadow_dy != 0.0 {
                            let sh_color = apply_fade_to_color(shadow_color, fade_alpha);
                            for &(gx, gy, cov) in &cached.coverage {
                                let px = (glyph_x + (cached.px_bounds_min_x + gx as f32 + slant_offset(gy as f32)) * scale + shadow_dx * scale).round() as i32;
                                let py = (cursor_y + (cached.px_bounds_min_y + gy as f32) * scale + shadow_dy * scale).round() as i32;
                                if px < 0 || py < 0 || px >= output_width as i32 || py >= output_height as i32 { continue; }
                                if let Some(ref cc) = clip_check {
                                    let (px_f, py_f) = (px as f32, py as f32);
                                    if (px_f >= cc.x1 && px_f <= cc.x2 && py_f >= cc.y1 && py_f <= cc.y2) == cc.inverse { continue; }
                                }
                                let idx = (py as usize * output_width + px as usize) * 4;
                                direct_composite(temp_buf, idx, sh_color, cov);
                                new_dirty.expand(px, py, 1);
                            }
                        }

                        // Outline passes
                        if !oo.is_empty() {
                            for &(ox, oy) in oo.iter() {
                                for &(gx, gy, cov) in &cached.coverage {
                                    let px = (glyph_x + (cached.px_bounds_min_x + gx as f32 + slant_offset(gy as f32)) * scale + ox * scale).round() as i32;
                                    let py = (cursor_y + (cached.px_bounds_min_y + gy as f32) * scale + oy * scale).round() as i32;
                                    if px < 0 || py < 0 || px >= output_width as i32 || py >= output_height as i32 { continue; }
                                    if let Some(ref cc) = clip_check {
                                        let (px_f, py_f) = (px as f32, py as f32);
                                        if (px_f >= cc.x1 && px_f <= cc.x2 && py_f >= cc.y1 && py_f <= cc.y2) == cc.inverse { continue; }
                                    }
                                    let idx = (py as usize * output_width + px as usize) * 4;
                                    direct_composite(temp_buf, idx, out_color, cov);
                                    new_dirty.expand(px, py, 1);
                                }
                            }
                        }

                        // Fill pass
                        let fill = apply_fade_to_color(ch_fill_color, fade_alpha);
                        for &(gx, gy, cov) in &cached.coverage {
                            local_pixels += 1;
                            let px = (glyph_x + (cached.px_bounds_min_x + gx as f32 + slant_offset(gy as f32)) * scale).round() as i32;
                            let py = (cursor_y + (cached.px_bounds_min_y + gy as f32) * scale).round() as i32;
                            if px < 0 || py < 0 || px >= output_width as i32 || py >= output_height as i32 { continue; }
                            if let Some(ref cc) = clip_check {
                                let (px_f, py_f) = (px as f32, py as f32);
                                if (px_f >= cc.x1 && px_f <= cc.x2 && py_f >= cc.y1 && py_f <= cc.y2) == cc.inverse { continue; }
                            }
                            let idx = (py as usize * output_width + px as usize) * 4;
                            direct_composite(temp_buf, idx, fill, cov);
                            new_dirty.expand(px, py, 1);
                        }

                        stats.pixels_written += local_pixels;
                    }
                }
                cursor_x += (h_adv_px + spacing_adv) * bold_scale;
                char_idx += 1;
            }

            cursor_y += line_height * scale;
        }
    }

    // Phase 3: CPU composite dirty rect onto output
    if let Some(dr) = new_dirty.clamp(output_width as i32, output_height as i32) {
        cpu_composite_dirty_rect(temp_buf, output, output_width, &dr, blend_mode);
    }
    cache.prev_dirty = new_dirty;

    stats
}


/// Direct source-over composite with fast transparent-dst path.
#[inline]
fn direct_composite(output: &mut [u8], idx: usize, color: [f32; 4], coverage: f32) {
    let alpha = coverage; // ab_glyph coverage is already in 0..=1
    let src_a = alpha * color[3];
    if src_a < 0.0001 { return; }

    // Fast path: destination pixel is transparent — just write src
    let dst_a = output[idx + 3];
    if dst_a == 0 {
        output[idx] = (color[0] * src_a * 255.0 + 0.5) as u8;
        output[idx + 1] = (color[1] * src_a * 255.0 + 0.5) as u8;
        output[idx + 2] = (color[2] * src_a * 255.0 + 0.5) as u8;
        output[idx + 3] = (src_a * 255.0 + 0.5) as u8;
        return;
    }

    let dst_r = output[idx] as f32 * RECIP_255;
    let dst_g = output[idx + 1] as f32 * RECIP_255;
    let dst_b = output[idx + 2] as f32 * RECIP_255;
    let dst_a_f = dst_a as f32 * RECIP_255;

    let out_a = src_a + dst_a_f * (1.0 - src_a);
    let inv_out_a = 1.0 / out_a;
    let dst_factor = dst_a_f * (1.0 - src_a);
    let out_r = (color[0] * src_a + dst_r * dst_factor) * inv_out_a;
    let out_g = (color[1] * src_a + dst_g * dst_factor) * inv_out_a;
    let out_b = (color[2] * src_a + dst_b * dst_factor) * inv_out_a;

    output[idx] = (out_r * 255.0 + 0.5) as u8;
    output[idx + 1] = (out_g * 255.0 + 0.5) as u8;
    output[idx + 2] = (out_b * 255.0 + 0.5) as u8;
    output[idx + 3] = (out_a * 255.0 + 0.5) as u8;
}

/// CPU composite: blend source buffer onto destination within a dirty rectangle.
fn cpu_composite_dirty_rect(
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    dirty: &DirtyRect,
    blend_mode: AssBlendMode,
) {
    // D4: Parallelized by row — split dst into non-overlapping row chunks
    assert_eq!(dst.len() % (width * 4), 0);
    let row_stride = width * 4;
    let min_y = dirty.min_y.max(0) as usize;
    let max_y = (dirty.max_y as usize).min(dst.len() / row_stride).saturating_sub(1);
    if min_y > max_y { return; }

    dst[min_y * row_stride..=(max_y * row_stride + row_stride - 1)]
        .par_chunks_mut(row_stride)
        .enumerate()
        .for_each(|(rel_y, dst_row)| {
            let y = (min_y + rel_y) as i32;
            let row_start = y as usize * width * 4;
            for x in dirty.min_x..=dirty.max_x {
                let col_offset = x as usize * 4;
                let idx = row_start + col_offset;
                let sa = src[idx + 3];
                if sa == 0 { continue; }

                // Fast path: opaque src pixel
                if sa == 255 {
                    dst_row[col_offset] = src[idx];
                    dst_row[col_offset + 1] = src[idx + 1];
                    dst_row[col_offset + 2] = src[idx + 2];
                    dst_row[col_offset + 3] = 255;
                    continue;
                }

                // Fast path: transparent dst
                if dst_row[col_offset + 3] == 0 {
                    dst_row[col_offset] = src[idx];
                    dst_row[col_offset + 1] = src[idx + 1];
                    dst_row[col_offset + 2] = src[idx + 2];
                    dst_row[col_offset + 3] = sa;
                    continue;
                }

                let sa_f = sa as f32 * RECIP_255;
                let sr = src[idx] as f32 * RECIP_255;
                let sg = src[idx + 1] as f32 * RECIP_255;
                let sb = src[idx + 2] as f32 * RECIP_255;

                let da = dst_row[col_offset + 3] as f32 * RECIP_255;
                let dr = dst_row[col_offset] as f32 * RECIP_255;
                let dg = dst_row[col_offset + 1] as f32 * RECIP_255;
                let db = dst_row[col_offset + 2] as f32 * RECIP_255;

                let src_px = [sr * sa_f, sg * sa_f, sb * sa_f, sa_f];
                let dst_px = [dr * da, dg * da, db * da, da];

                let blended = blend_pixel(blend_mode, src_px, dst_px);

                dst_row[col_offset] = (blended[0] * 255.0 + 0.5) as u8;
                dst_row[col_offset + 1] = (blended[1] * 255.0 + 0.5) as u8;
                dst_row[col_offset + 2] = (blended[2] * 255.0 + 0.5) as u8;
                dst_row[col_offset + 3] = (blended[3] * 255.0 + 0.5) as u8;
            }
        });
}

fn apply_fade_to_color(color: [f32; 4], fade_alpha: f32) -> [f32; 4] {
    [color[0], color[1], color[2], color[3] * fade_alpha]
}

/// Generate outline sampling offsets given outline width and optional blur radius.
/// When blur > 0, generates additional samples at larger radii to simulate a soft glow
/// behind the fill text (blur is achieved by the outline, so layering is always correct).
fn gen_outline_offsets(width: f32, blur: f32) -> Vec<(f32, f32)> {
    let mut offsets = Vec::new();
    // Base outline: 16 directions at the given width
    let d = width;
    let d2 = width * 0.7071;
    offsets.extend_from_slice(&[
        (d, 0.0), (-d, 0.0), (0.0, d), (0.0, -d),
        (d2, d2), (-d2, d2), (d2, -d2), (-d2, -d2),
        (d, d2), (d, -d2), (-d, d2), (-d, -d2),
        (d2, d), (-d2, d), (d2, -d), (-d2, -d),
    ]);
    // If blur is active, add extra rings of samples at larger radii
    // to approximate a Gaussian blur of the outline behind the fill text
    if blur > 0.0 {
        let steps = (blur * 2.0).round() as i32;
        for ring in 1..=steps {
            let r = width + blur * (ring as f32) / steps as f32;
            let r2 = r * 0.7071;
            offsets.push((r, 0.0));
            offsets.push((-r, 0.0));
            offsets.push((0.0, r));
            offsets.push((0.0, -r));
            offsets.push((r2, r2));
            offsets.push((-r2, r2));
            offsets.push((r2, -r2));
            offsets.push((-r2, -r2));
        }
    }
    offsets
}

/// Compute fade alpha for the current time.
fn compute_fade_alpha(time_ms: i64, ev_start: i64, ev_end: i64, fade: &Option<FadeData>) -> f32 {
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
        // \fad(t1,t2) — fade in over t1 ms, fade out over last t2 ms
        let dur = ev_end - ev_start;
        if elapsed <= fade.t1 {
            elapsed as f32 / fade.t1.max(1) as f32
        } else if elapsed >= dur - fade.t2 {
            (dur - elapsed) as f32 / fade.t2.max(1) as f32
        } else {
            1.0
        }
    }
}

/// Apply \t animated transforms active at the current time.
fn apply_transforms(
    time_ms: i64,
    ev_start: i64,
    ev_end: i64,
    transforms: &[OverrideTransform],
    base: &ParsedTags,
) -> ParsedTags {
    let mut result = base.clone();
    let elapsed = time_ms - ev_start;
    for tform in transforms {
        let t1 = if tform.start_t == 0 { 0 } else { tform.start_t };
        let t2 = if tform.end_t == 0 { ev_end - ev_start } else { tform.end_t };
        if elapsed < t1 || elapsed > t2 { continue; }
        let dur = (t2 - t1).max(1);
        let raw_t = (elapsed - t1) as f32 / dur as f32;
        // Acceleration curve: t = t^accel (accel > 1 = slow start, < 1 = fast start)
        let t = raw_t.powf(tform.acceleration);
        let target = &tform.tags;
        // Interpolate each field
        interpolate_f32(&mut result.fontsize, target.fontsize, base.fontsize, t);
        interpolate_color(&mut result.primary_color, target.primary_color, base.primary_color, t);
        interpolate_color(&mut result.secondary_color, target.secondary_color, base.secondary_color, t);
        interpolate_color(&mut result.outline_color, target.outline_color, base.outline_color, t);
        interpolate_color(&mut result.back_color, target.back_color, base.back_color, t);
        interpolate_f32(&mut result.scale_x, target.scale_x, base.scale_x, t);
        interpolate_f32(&mut result.scale_y, target.scale_y, base.scale_y, t);
        interpolate_f32(&mut result.spacing, target.spacing, base.spacing, t);
        interpolate_f32(&mut result.frz, target.frz, base.frz, t);
        interpolate_f32(&mut result.frx, target.frx, base.frx, t);
        interpolate_f32(&mut result.fry, target.fry, base.fry, t);
        interpolate_f32(&mut result.fax, target.fax, base.fax, t);
        interpolate_f32(&mut result.fay, target.fay, base.fay, t);
        interpolate_f32(&mut result.bord, target.bord, base.bord, t);
        interpolate_f32(&mut result.xbord, target.xbord, base.xbord, t);
        interpolate_f32(&mut result.ybord, target.ybord, base.ybord, t);
        interpolate_f32(&mut result.shad, target.shad, base.shad, t);
        interpolate_f32(&mut result.xshad, target.xshad, base.xshad, t);
        interpolate_f32(&mut result.yshad, target.yshad, base.yshad, t);
        interpolate_f32(&mut result.be, target.be, base.be, t);
        interpolate_f32(&mut result.blur, target.blur, base.blur, t);
        interpolate_f32(&mut result.alpha, target.alpha, base.alpha, t);
        // Clip: interpolate rectangular clip coordinates
        if let Some(ref tgt_clip) = target.clip {
            if let Some(ref base_clip) = base.clip {
                let n = base_clip.points.len().min(tgt_clip.points.len());
                let mut pts = base_clip.points.clone();
                for i in 0..n {
                    pts[i].0 = base_clip.points[i].0 + (tgt_clip.points[i].0 - base_clip.points[i].0) * t;
                    pts[i].1 = base_clip.points[i].1 + (tgt_clip.points[i].1 - base_clip.points[i].1) * t;
                }
                result.clip = Some(ClipData { points: pts, ..base_clip.clone() });
            } else {
                result.clip = Some(tgt_clip.clone());
            }
        }
        // Pos: interpolate
        if let Some(tgt_pos) = target.pos {
            result.pos = Some(if let Some(base_pos) = base.pos {
                (base_pos.0 + (tgt_pos.0 - base_pos.0) * t, base_pos.1 + (tgt_pos.1 - base_pos.1) * t)
            } else { tgt_pos });
        }
        // Bool/binary fields: use target once t > 0.5
        if t > 0.5 {
            if target.bold.is_some() { result.bold = target.bold; }
            if target.italic.is_some() { result.italic = target.italic; }
            if target.underline.is_some() { result.underline = target.underline; }
            if target.strikeout.is_some() { result.strikeout = target.strikeout; }
            if target.fontname.is_some() { result.fontname = target.fontname.clone(); }
            if target.alignment.is_some() { result.alignment = target.alignment; }
        }
    }
    result
}

fn interpolate_f32(target: &mut Option<f32>, tgt_val: Option<f32>, base_val: Option<f32>, t: f32) {
    if let Some(tv) = tgt_val {
        *target = Some(if let Some(bv) = base_val { bv + (tv - bv) * t } else { tv });
    }
}

fn interpolate_color(target: &mut Option<[f32; 4]>, tgt_val: Option<[f32; 4]>, base_val: Option<[f32; 4]>, t: f32) {
    if let Some(tv) = tgt_val {
        *target = Some(if let Some(bv) = base_val {
            [bv[0] + (tv[0] - bv[0]) * t,
             bv[1] + (tv[1] - bv[1]) * t,
             bv[2] + (tv[2] - bv[2]) * t,
             bv[3] + (tv[3] - bv[3]) * t]
        } else { tv });
    }
}

/// Compute interpolated position for \move animation.
fn compute_move_pos(time_ms: i64, ev_start: i64, ev_end: i64, mv: &Option<MoveAnim>) -> Option<(f32, f32)> {
    let mv = mv.as_ref()?;
    let t1 = mv.t1.unwrap_or(0);
    let t2 = mv.t2.unwrap_or(ev_end - ev_start);
    let elapsed = time_ms - ev_start;
    if elapsed < t1 { return Some((mv.x1, mv.y1)); }
    if elapsed > t2 { return Some((mv.x2, mv.y2)); }
    let dur = (t2 - t1).max(1);
    let frac = (elapsed - t1) as f32 / dur as f32;
    Some((mv.x1 + (mv.x2 - mv.x1) * frac, mv.y1 + (mv.y2 - mv.y1) * frac))
}

/// Return active events at a given time.
fn active_events(ass_script: &AssScript, time_ms: i64) -> Vec<&OwnedEvent> {
    ass_script.events.iter().filter(|e| e.start_ms <= time_ms && time_ms < e.end_ms).collect()
}

/// Resolve the style for a dialogue event (look up by style_name).
fn resolve_style<'a>(ass_script: &'a AssScript, ev: &OwnedEvent) -> &'a OwnedStyle {
    static DEFAULT_STYLE: std::sync::OnceLock<OwnedStyle> = std::sync::OnceLock::new();
    ass_script
        .styles
        .iter()
        .find(|s| s.name == ev.style_name)
        .or_else(|| ass_script.styles.first())
        .unwrap_or_else(|| DEFAULT_STYLE.get_or_init(OwnedStyle::default))
}

/// Convert alignment (an 1-9) to (x_anchor, y_anchor): 0=left/bottom, 1=center, 2=right/top.
fn alignment_to_anchor(an: i32) -> (i32, i32) {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ass_color_to_rgba_alpha_inversion() {
        // ASS &H00 = opaque, &HFF = transparent
        let c = ass_color_to_rgba("&H00FFFFFF");
        assert!((c[0] - 1.0).abs() < 0.01, "R should be 1.0");
        assert!((c[1] - 1.0).abs() < 0.01, "G should be 1.0");
        assert!((c[2] - 1.0).abs() < 0.01, "B should be 1.0");
        assert!((c[3] - 1.0).abs() < 0.01, "A should be 1.0 (opaque)");

        let c = ass_color_to_rgba("&H80000000");
        assert!((c[3] - 0.5).abs() < 0.01, "Alpha should be ~0.5");

        let c = ass_color_to_rgba("&HFF000000");
        assert!(c[3] < 0.01, "Alpha should be 0.0 (transparent)");

        let c = ass_color_to_rgba("&HFFFFFF");
        assert!((c[3] - 1.0).abs() < 0.01, "No-alpha colors should default to opaque");
    }

    #[test]
    fn test_ass_color_to_rgba_rgb_order() {
        let c = ass_color_to_rgba("&H000000FF");
        assert!((c[0] - 1.0).abs() < 0.01, "Should be pure red");
        assert!(c[1] < 0.01, "Should be no green");
        assert!(c[2] < 0.01, "Should be no blue");

        let c = ass_color_to_rgba("&H0000FF00");
        assert!(c[0] < 0.01, "Should be no red");
        assert!((c[1] - 1.0).abs() < 0.01, "Should be pure green");
        assert!(c[2] < 0.01, "Should be no blue");

        let c = ass_color_to_rgba("&H00FF0000");
        assert!(c[0] < 0.01, "Should be no red");
        assert!(c[1] < 0.01, "Should be no green");
        assert!((c[2] - 1.0).abs() < 0.01, "Should be pure blue");
    }

    #[test]
    fn test_parse_ass_time() {
        assert_eq!(parse_ass_time("0:00:00.00"), 0);
        assert_eq!(parse_ass_time("0:00:01.00"), 1000);
        assert_eq!(parse_ass_time("0:01:00.00"), 60000);
        assert_eq!(parse_ass_time("1:00:00.00"), 3600000);
        assert_eq!(parse_ass_time("0:00:00.50"), 500);
        assert_eq!(parse_ass_time("0:00:05.25"), 5250);
    }

    #[test]
    fn test_parse_ass_basic() {
        let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000088FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World!
Dialogue: 0,0:00:03.00,0:00:08.00,Default,,0,0,0,,Line two
"#;
        let result = parse_ass_file(ass).expect("Should parse valid ASS file");
        assert_eq!(result.styles.len(), 1);
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.styles[0].name, "Default");
        assert_eq!(result.styles[0].fontname, "Arial");
        assert!((result.styles[0].fontsize - 48.0).abs() < 0.1);
        let pc = result.styles[0].primary_color;
        assert!((pc[3] - 1.0).abs() < 0.01, "Primary color should be opaque");

        assert_eq!(result.events[0].start_ms, 1000);
        assert_eq!(result.events[0].end_ms, 5000);
        assert_eq!(result.events[0].text, "Hello World!");
    }

    #[test]
    fn test_parse_override_tags() {
        let (clean, tags) = parse_override_tags(r"{\fs50}{\b1}{\i1}Hello {\c&H0000FF&}World!{\r}");
        assert_eq!(clean, "Hello World!");
        assert_eq!(tags.fontsize, Some(50.0));
        assert_eq!(tags.bold, Some(true));
        assert_eq!(tags.italic, Some(true));
        let color = tags.primary_color.unwrap();
        assert!((color[0] - 1.0).abs() < 0.01);
        assert!(color[1] < 0.01);
        assert!(color[2] < 0.01);
    }

    #[test]
    fn test_active_events() {
        let ass = AssScript {
            info: HashMap::new(),
            styles: vec![],
            events: vec![
                OwnedEvent { start_ms: 1000, end_ms: 5000, ..Default::default() },
                OwnedEvent { start_ms: 3000, end_ms: 8000, ..Default::default() },
            ],
            play_res_x: None,
            play_res_y: None,
        };
        assert_eq!(active_events(&ass, 0).len(), 0);
        assert_eq!(active_events(&ass, 2000).len(), 1);
        assert_eq!(active_events(&ass, 4000).len(), 2);
        assert_eq!(active_events(&ass, 6000).len(), 1);
        assert_eq!(active_events(&ass, 9000).len(), 0);
    }

    #[test]
    fn test_font_cache_has_fonts() {
        let cache = FontCache::new();
        assert!(!cache.entries.is_empty(), "Font cache should not be empty on Windows");
    }

    #[test]
    fn test_render_produces_pixels() {
        let content = r#"[Script Info]
Title: Test
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World
"#;
        let ass = parse_ass_file(content).expect("parse should succeed");
        let mut font_cache = FontCache::new();
        if font_cache.entries.is_empty() {
            eprintln!("SKIP test_render_produces_pixels: no fonts available");
            return;
        }

        let width = 640usize;
        let height = 480usize;
        let mut output = vec![0u8; width * height * 4];

        let mut cache = RenderCache::new();
        render_ass_subtitle_frame(
            &ass,
            2000,
            &mut font_cache,
            1.0,
            0.5,
            0.5,
            1.0,
            1.0,
            AssBlendMode::Normal,
            None,
            true,
            &mut output,
            width,
            height,
            &mut cache,
        );

        let non_zero = output.chunks(4).filter(|p| p[3] > 0).count();
        assert!(non_zero > 0, "Expected non-zero pixels in output, but all are transparent");
    }
}
