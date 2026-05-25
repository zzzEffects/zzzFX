//! Data types for the ASS subtitle system.

use std::collections::HashMap;


// ---------------------------------------------------------------------------
// Top-level script container
// ---------------------------------------------------------------------------

/// Parsed ASS script with owned data.
pub struct AssScript {
    pub info: HashMap<String, String>,
    pub styles: Vec<OwnedStyle>,
    pub events: Vec<OwnedEvent>,
    pub play_res_x: Option<u32>,
    pub play_res_y: Option<u32>,
}

// ---------------------------------------------------------------------------
// Style & Event
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Parsed override tags
// ---------------------------------------------------------------------------

/// Accumulator for inline override tags (`\fs50`, `\b1`, etc.).
/// Every field is `Option<T>` so the parser can distinguish "not set" from
/// "explicitly set to default."
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

    // Clip — single field; `inverse` distinguishes \clip from \iclip
    pub clip: Option<ClipData>,

    // Fade
    pub fade: Option<FadeData>,

    // Karaoke
    pub karaoke: Option<KaraokeData>,

    // Drawing
    pub drawing_scale: Option<f32>,

    // Reset — only meaningful within parse_tag_segments, NOT in the final merged tags
    #[doc(hidden)]
    pub reset: bool,
    #[doc(hidden)]
    pub reset_style: Option<String>,

    // Transform
    pub transforms: Vec<OverrideTransform>,
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

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

impl ClipData {
    /// Build an AABB from clip points for fast rejection.
    pub fn bounding_box(&self) -> Option<(f32, f32, f32, f32)> {
        if self.points.len() < 4 {
            return None;
        }
        let mut x1 = f32::MAX;
        let mut y1 = f32::MAX;
        let mut x2 = f32::MIN;
        let mut y2 = f32::MIN;
        for &(px, py) in &self.points {
            x1 = x1.min(px);
            y1 = y1.min(py);
            x2 = x2.max(px);
            y2 = y2.max(py);
        }
        Some((x1, y1, x2, y2))
    }
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
// Render stats
// ---------------------------------------------------------------------------

/// Diagnostic counters returned by the renderer.
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
    /// Whether GPU compositing was used for the final blend step.
    pub gpu_composite_used: bool,
}

// ---------------------------------------------------------------------------
// Internal helper types (pub(crate))
// ---------------------------------------------------------------------------

/// Precomputed axis-aligned bounding box for fast clip rejection in pixel loops.
#[derive(Clone)]
pub(crate) struct ClipCheck {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub inverse: bool,
}

/// Tracks the bounding box of rendered pixels for efficient compositing.
#[derive(Clone, Copy, Default)]
pub(crate) struct DirtyRect {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl DirtyRect {
    pub fn expand(&mut self, x: i32, y: i32, pad: i32) {
        self.min_x = self.min_x.min(x - pad);
        self.min_y = self.min_y.min(y - pad);
        self.max_x = self.max_x.max(x + pad);
        self.max_y = self.max_y.max(y + pad);
    }

    pub fn clamp(self, w: i32, h: i32) -> Option<Self> {
        let min_x = self.min_x.max(0);
        let min_y = self.min_y.max(0);
        let max_x = self.max_x.min(w - 1);
        let max_y = self.max_y.min(h - 1);
        if min_x > max_x || min_y > max_y {
            None
        } else {
            Some(Self { min_x, min_y, max_x, max_y })
        }
    }
}

/// Karaoke syllable timing data for timed character coloring.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct KaraokeSyllable {
    pub char_start: usize,
    pub char_end: usize,
    pub duration_cs: i64,
    pub kind: KaraokeKind,
}

/// Precomputed per-event data, keyed by (generation, event pointer address).
pub(crate) struct CachedEventData {
    pub segments: Vec<TagSegment>,
    pub clean_text: String,
    /// Clean text with ASS \N/\n converted to real newlines for the layout engine.
    pub text_for_layout: String,
    /// Char-to-segment index lookup, built once per event.
    pub char_to_seg: Vec<usize>,
    /// Merged base tags accumulated by sequentially walking all segments.
    pub merged_base_tags: ParsedTags,
    /// Non-whitespace characters from clean_text, cached for font coverage checks.
    pub text_chars: Vec<char>,
    /// Fast hash of clean_text for layout cache keying.
    pub text_hash: u64,
    /// Karaoke syllable boundaries (empty if no karaoke tags).
    pub karaoke_syllables: Vec<KaraokeSyllable>,
}
