use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::OnceLock;

use ratex_types::{Color, DisplayItem, DisplayList, PathCommand};
use resvg::{tiny_skia, usvg};

use crate::settings::latex_display::{LaTeXDisplay, MathStyle as LaTeXMathStyle};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SUPERSCRIPT_SCALE: f64 = 0.7;
const SUBSCRIPT_SCALE: f64 = 0.7;
const SUPERSCRIPT_Y_OFFSET: f64 = 0.35;
const SUBSCRIPT_Y_OFFSET: f64 = -0.25;
const FRACTION_BAR_THICKNESS: f64 = 0.05;
const FRACTION_BAR_OVERHANG: f64 = 0.5;
const SQRT_BAR_THICKNESS: f64 = 0.04;
const H_SPACING_FACTOR: f64 = 0.6;
const SQRT_SYMBOL: char = '\u{221A}';
const LAYOUT_FONT_SIZE: f64 = 10.0;
const DISPLAY_SCALE_BOOST: f64 = 1.15;
const INLINE_SCALE_BOOST: f64 = 1.0;

// ---------------------------------------------------------------------------
// LaTeX → Unicode symbol map
// ---------------------------------------------------------------------------

fn get_symbol_map() -> &'static HashMap<&'static str, char> {
    static MAP: OnceLock<HashMap<&'static str, char>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert("alpha", '\u{03B1}');
        m.insert("beta", '\u{03B2}');
        m.insert("gamma", '\u{03B3}');
        m.insert("delta", '\u{03B4}');
        m.insert("epsilon", '\u{03B5}');
        m.insert("zeta", '\u{03B6}');
        m.insert("eta", '\u{03B7}');
        m.insert("theta", '\u{03B8}');
        m.insert("iota", '\u{03B9}');
        m.insert("kappa", '\u{03BA}');
        m.insert("lambda", '\u{03BB}');
        m.insert("mu", '\u{03BC}');
        m.insert("nu", '\u{03BD}');
        m.insert("xi", '\u{03BE}');
        m.insert("pi", '\u{03C0}');
        m.insert("rho", '\u{03C1}');
        m.insert("sigma", '\u{03C3}');
        m.insert("tau", '\u{03C4}');
        m.insert("upsilon", '\u{03C5}');
        m.insert("phi", '\u{03C6}');
        m.insert("chi", '\u{03C7}');
        m.insert("psi", '\u{03C8}');
        m.insert("omega", '\u{03C9}');
        // Uppercase Greek
        m.insert("Gamma", '\u{0393}');
        m.insert("Delta", '\u{0394}');
        m.insert("Theta", '\u{0398}');
        m.insert("Lambda", '\u{039B}');
        m.insert("Xi", '\u{039E}');
        m.insert("Pi", '\u{03A0}');
        m.insert("Sigma", '\u{03A3}');
        m.insert("Upsilon", '\u{03A5}');
        m.insert("Phi", '\u{03A6}');
        m.insert("Psi", '\u{03A8}');
        m.insert("Omega", '\u{03A9}');
        // Math symbols
        m.insert("infty", '\u{221E}');
        m.insert("pm", '\u{00B1}');
        m.insert("mp", '\u{2213}');
        m.insert("times", '\u{00D7}');
        m.insert("div", '\u{00F7}');
        m.insert("cdot", '\u{22C5}');
        m.insert("leq", '\u{2264}');
        m.insert("geq", '\u{2265}');
        m.insert("neq", '\u{2260}');
        m.insert("approx", '\u{2248}');
        m.insert("equiv", '\u{2261}');
        m.insert("propto", '\u{221D}');
        m.insert("sim", '\u{223C}');
        m.insert("subset", '\u{2282}');
        m.insert("supset", '\u{2283}');
        m.insert("subseteq", '\u{2286}');
        m.insert("supseteq", '\u{2287}');
        m.insert("cap", '\u{2229}');
        m.insert("cup", '\u{222A}');
        m.insert("forall", '\u{2200}');
        m.insert("exists", '\u{2203}');
        m.insert("neg", '\u{00AC}');
        m.insert("vee", '\u{2228}');
        m.insert("wedge", '\u{2227}');
        m.insert("rightarrow", '\u{2192}');
        m.insert("leftarrow", '\u{2190}');
        m.insert("leftrightarrow", '\u{2194}');
        m.insert("Rightarrow", '\u{21D2}');
        m.insert("Leftarrow", '\u{21D0}');
        m.insert("Leftrightarrow", '\u{21D4}');
        m.insert("to", '\u{2192}');
        m.insert("mapsto", '\u{21A6}');
        m.insert("partial", '\u{2202}');
        m.insert("nabla", '\u{2207}');
        m.insert("sum", '\u{2211}');
        m.insert("prod", '\u{220F}');
        m.insert("int", '\u{222B}');
        m.insert("oint", '\u{222E}');
        m.insert("iint", '\u{222C}');
        m.insert("iiint", '\u{222D}');
        m.insert("angle", '\u{2220}');
        m.insert("triangle", '\u{25B3}');
        m.insert("parallel", '\u{2225}');
        m.insert("perp", '\u{27C2}');
        m.insert("circ", '\u{2218}');
        m.insert("bullet", '\u{2022}');
        m.insert("oplus", '\u{2295}');
        m.insert("ominus", '\u{2296}');
        m.insert("otimes", '\u{2297}');
        m.insert("oslash", '\u{2298}');
        m.insert("odot", '\u{2299}');
        m.insert("star", '\u{22C6}');
        m.insert("ast", '\u{2217}');
        m.insert("dagger", '\u{2020}');
        m.insert("ddagger", '\u{2021}');
        m.insert("S", '\u{00A7}');
        m.insert("P", '\u{00B6}');
        m.insert("pounds", '\u{00A3}');
        m.insert("dots", '\u{2026}');
        m.insert("cdots", '\u{22EF}');
        m.insert("vdots", '\u{22EE}');
        m.insert("ddots", '\u{22F1}');
        m.insert("hbar", '\u{0127}');
        m.insert("ell", '\u{2113}');
        m.insert("wp", '\u{2118}');
        m.insert("Re", '\u{211C}');
        m.insert("Im", '\u{2111}');
        m.insert("aleph", '\u{2135}');
        m.insert("emptyset", '\u{2205}');
        m.insert("varnothing", '\u{2205}');
        m.insert("Box", '\u{25A1}');
        m.insert("square", '\u{25A1}');
        m.insert("Diamond", '\u{25C7}');
        m.insert("flat", '\u{266D}');
        m.insert("natural", '\u{266E}');
        m.insert("sharp", '\u{266F}');
        m.insert("clubsuit", '\u{2663}');
        m.insert("diamondsuit", '\u{2662}');
        m.insert("heartsuit", '\u{2661}');
        m.insert("spadesuit", '\u{2660}');
        m.insert("langle", '\u{27E8}');
        m.insert("rangle", '\u{27E9}');
        m
    })
}

// ---------------------------------------------------------------------------
// Lazy fontdb — shared with svg_display via crate::get_fontdb()
// ---------------------------------------------------------------------------

fn get_fontdb() -> &'static usvg::fontdb::Database {
    crate::get_fontdb()
}

// ---------------------------------------------------------------------------
// Cached LaTeX state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct CachedLaTeX {
    pub tree: usvg::Tree,
    pub native_w: f32,
    pub native_h: f32,
    pub dpi: f32,
    formula_hash: u64,
    math_style: u8,
    text_color: [f32; 4],
    font_name_hash: u64,
}

impl CachedLaTeX {
    fn hash_formula(formula: &str) -> u64 {
        let mut h = DefaultHasher::new();
        formula.len().hash(&mut h);
        if formula.len() <= 128 {
            formula.hash(&mut h);
        } else {
            formula[..64].hash(&mut h);
            formula[formula.len() - 64..].hash(&mut h);
        }
        h.finish()
    }

    fn hash_str(s: &str) -> u64 {
        let mut h = DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    }

    pub fn is_valid(
        &self,
        formula: &str,
        dpi: f32,
        math_style: LaTeXMathStyle,
        text_color: &[f32; 4],
        font_name: &str,
    ) -> bool {
        (self.dpi - dpi).abs() < f32::EPSILON
            && self.math_style == math_style as u8
            && self.formula_hash == Self::hash_formula(formula)
            && self.text_color == *text_color
            && self.font_name_hash == Self::hash_str(font_name)
    }
}

// ---------------------------------------------------------------------------
// Parser state
// ---------------------------------------------------------------------------

struct Parser<'a> {
    chars: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { chars: input.as_bytes(), pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) {
        if self.pos < self.chars.len() {
            self.pos += 1;
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(b' ') = self.peek() {
            self.advance();
        }
    }

    fn try_read_command(&mut self) -> Option<String> {
        if self.peek() == Some(b'\\') {
            self.advance(); // skip backslash
            let start = self.pos;
            // Read command name: alphabetic chars
            while let Some(ch) = self.peek() {
                if ch.is_ascii_alphabetic() {
                    self.advance();
                } else {
                    break;
                }
            }
            if self.pos == start {
                // Single-char command like \^, \_, etc.
                if self.peek().is_some() {
                    let cmd = String::from_utf8_lossy(&self.chars[self.pos..self.pos + 1]).into_owned();
                    self.advance();
                    return Some(cmd);
                }
                return None;
            }
            Some(String::from_utf8_lossy(&self.chars[start..self.pos]).into_owned())
        } else {
            None
        }
    }

    fn try_read_group(&mut self) -> Option<String> {
        if self.peek() == Some(b'{') {
            self.advance(); // skip {
            let mut depth = 1u32;
            let start = self.pos;
            while self.pos < self.chars.len() && depth > 0 {
                match self.chars[self.pos] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    self.pos += 1;
                }
            }
            if depth != 0 {
                // Unbalanced braces — group was never closed
                return None;
            }
            let content = String::from_utf8_lossy(&self.chars[start..self.pos]).into_owned();
            if self.pos < self.chars.len() {
                self.advance(); // skip closing }
            }
            Some(content)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Layout state (accumulates DisplayList items and tracks bounding box)
// ---------------------------------------------------------------------------

struct LayoutState {
    items: Vec<DisplayItem>,
    current_x: f64,
    max_y: f64,
    min_y: f64,
}

impl LayoutState {
    fn new() -> Self {
        Self {
            items: Vec::new(),
            current_x: 0.0,
            max_y: 0.0,
            min_y: 0.0,
        }
    }

    fn push_glyph(&mut self, ch: char, font_size: f64, y_offset: f64, color: Color) {
        let char_width = font_size * H_SPACING_FACTOR;
        self.items.push(DisplayItem::GlyphPath {
            x: self.current_x,
            y: y_offset,
            scale: font_size,
            font: "Main-Regular".into(),
            char_code: ch as u32,
            color,
        });
        let glyph_top = y_offset + font_size * 0.7;
        let glyph_bottom = y_offset - font_size * 0.2;
        self.max_y = self.max_y.max(glyph_top);
        self.min_y = self.min_y.min(glyph_bottom);
        self.current_x += char_width;
    }

    fn push_line(&mut self, x: f64, y: f64, width: f64, thickness: f64, color: Color) {
        self.items.push(DisplayItem::Line {
            x,
            y,
            width,
            thickness,
            color,
            dashed: false,
        });
    }

    fn to_display_list(self) -> DisplayList {
        let height = self.max_y.max(0.0);
        let depth = (-self.min_y).max(0.0);
        let width = self.current_x.max(0.0);
        DisplayList {
            items: self.items,
            width,
            height,
            depth,
        }
    }
}

// ---------------------------------------------------------------------------
// Main parsing entry: formula string → DisplayList
// ---------------------------------------------------------------------------

fn parse_formula(formula: &str, math_style: LaTeXMathStyle) -> Option<DisplayList> {
    let mut parser = Parser::new(formula);
    let mut state = LayoutState::new();
    let default_color = Color::WHITE;
    let scale_boost = match math_style {
        LaTeXMathStyle::Display => DISPLAY_SCALE_BOOST,
        LaTeXMathStyle::Inline => INLINE_SCALE_BOOST,
    };

    parse_expression(&mut parser, &mut state, LAYOUT_FONT_SIZE * scale_boost, 0.0, default_color, false);
    Some(state.to_display_list())
}

fn parse_expression(
    parser: &mut Parser,
    state: &mut LayoutState,
    font_size: f64,
    y_offset: f64,
    color: Color,
    is_math_mode: bool,
) {
    let symbol_map = get_symbol_map();

    while parser.pos < parser.chars.len() {
        parser.skip_whitespace();
        if parser.pos >= parser.chars.len() {
            break;
        }

        // Check for LaTeX commands
        if let Some(cmd) = parser.try_read_command() {
            let cmd_lower = cmd.to_lowercase();
            match cmd_lower.as_str() {
                "frac" => {
                    let num = parser.try_read_group();
                    let den = parser.try_read_group();
                    if let (Some(num_content), Some(den_content)) = (num, den) {
                        parse_fraction(state, &num_content, &den_content, font_size, y_offset, color);
                    }
                }
                "sqrt" => {
                    let radicand = parser.try_read_group();
                    if let Some(content) = radicand {
                        parse_sqrt(state, &content, font_size, y_offset, color);
                    }
                }
                "text" | "mathrm" | "textrm" => {
                    if let Some(content) = parser.try_read_group() {
                        parse_expression(&mut Parser::new(&content), state, font_size, y_offset, color, false);
                    }
                }
                "displaystyle" => {
                    // Switch to display style (larger) — handled inline
                }
                "limits" | "nolimits" => {
                    // Simple ignore for now
                }
                "left" => {
                    if let Some(ch) = parser.peek() {
                        let c = ch as char;
                        if matches!(c, '(' | ')' | '[' | ']' | '|' | '.') {
                            parser.advance();
                            state.push_glyph(c, font_size * 1.2, y_offset, color);
                        }
                        // '{' and '}' are excluded — require LaTeX-style \left\{ escaping
                        // which goes through the ordinary character path as escaped braces
                    }
                }
                "right" => {
                    if let Some(ch) = parser.peek() {
                        let c = ch as char;
                        if matches!(c, '(' | ')' | '[' | ']' | '|' | '.') {
                            parser.advance();
                            state.push_glyph(c, font_size * 1.2, y_offset, color);
                        }
                    }
                }
                _ => {
                    // Look up in symbol map
                    if let Some(&ch) = symbol_map.get(cmd.as_str()) {
                        let scale = if matches!(cmd_lower.as_str(), "sum" | "prod" | "int" | "oint" | "iint" | "iiint") {
                            font_size * 1.5
                        } else {
                            font_size
                        };
                        state.push_glyph(ch, scale, y_offset, color);
                    } else if let Some(&ch) = symbol_map.get(cmd_lower.as_str()) {
                        state.push_glyph(ch, font_size, y_offset, color);
                    }
                    // Unknown commands are silently ignored
                }
            }
            continue;
        }

        // Check for superscript:
        if parser.peek() == Some(b'^') {
            parser.advance();
            let super_fs = font_size * SUPERSCRIPT_SCALE;
            let super_y = y_offset + font_size * SUPERSCRIPT_Y_OFFSET;
            // Consume next token (group, command, or single char)
            if let Some(group) = parser.try_read_group() {
                parse_expression(&mut Parser::new(&group), state, super_fs, super_y, color, true);
            } else if let Some(cmd) = parser.try_read_command() {
                let symbol_map = get_symbol_map();
                if let Some(&ch) = symbol_map.get(cmd.as_str()) {
                    state.push_glyph(ch, super_fs, super_y, color);
                }
            } else if let Some(ch) = parser.peek() {
                parser.advance();
                state.push_glyph(ch as char, super_fs, super_y, color);
            }
            continue;
        }

        // Check for subscript: _
        if parser.peek() == Some(b'_') {
            parser.advance();
            let sub_fs = font_size * SUBSCRIPT_SCALE;
            let sub_y = y_offset + font_size * SUBSCRIPT_Y_OFFSET;
            if let Some(group) = parser.try_read_group() {
                parse_expression(&mut Parser::new(&group), state, sub_fs, sub_y, color, true);
            } else if let Some(cmd) = parser.try_read_command() {
                let symbol_map = get_symbol_map();
                if let Some(&ch) = symbol_map.get(cmd.as_str()) {
                    state.push_glyph(ch, sub_fs, sub_y, color);
                }
            } else if let Some(ch) = parser.peek() {
                parser.advance();
                state.push_glyph(ch as char, sub_fs, sub_y, color);
            }
            continue;
        }

        // Check for group: {...}
        if parser.peek() == Some(b'{') {
            if let Some(group) = parser.try_read_group() {
                parse_expression(&mut Parser::new(&group), state, font_size, y_offset, color, is_math_mode);
            }
            continue;
        }

        // Skip closing brace (should not happen at top level but be safe)
        if parser.peek() == Some(b'}') {
            parser.advance();
            continue;
        }

        // Ordinary character
        if let Some(ch) = parser.peek() {
            parser.advance();
            state.push_glyph(ch as char, font_size, y_offset, color);
        }
    }
}

// ---------------------------------------------------------------------------
// Fraction: \frac{numerator}{denominator}
// ---------------------------------------------------------------------------

fn parse_fraction(
    state: &mut LayoutState,
    num_content: &str,
    den_content: &str,
    font_size: f64,
    y_offset: f64,
    color: Color,
) {
    let reduced_fs = font_size * 0.8;

    // Layout numerator (above baseline)
    let mut num_state = LayoutState::new();
    parse_expression(&mut Parser::new(num_content), &mut num_state, reduced_fs, 0.0, color, true);
    let num_width = num_state.current_x;
    let num_height = num_state.max_y;

    // Layout denominator (below baseline)
    let mut den_state = LayoutState::new();
    parse_expression(&mut Parser::new(den_content), &mut den_state, reduced_fs, 0.0, color, true);
    let den_width = den_state.current_x;
    let den_depth = -den_state.min_y;

    let total_width = num_width.max(den_width) + FRACTION_BAR_OVERHANG * 2.0;
    let gap = font_size * 0.1; // padding between numerator/denominator and bar
    let bar_y = y_offset;
    let num_y = bar_y + FRACTION_BAR_THICKNESS / 2.0 + gap;
    let den_y = bar_y - FRACTION_BAR_THICKNESS / 2.0 - gap - den_depth;

    // Center numerator and denominator
    let num_x_offset = (total_width - num_width) / 2.0;
    let den_x_offset = (total_width - den_width) / 2.0;

    // Offset by current position
    let origin_x = state.current_x + FRACTION_BAR_OVERHANG;

    // Place numerator items
    for item in &num_state.items {
        let mut item = item.clone();
        match &mut item {
            DisplayItem::GlyphPath { x, y, .. } => {
                *x += origin_x + num_x_offset;
                *y = num_y;
            }
            DisplayItem::Line { x, y, .. } => {
                *x += origin_x + num_x_offset;
                *y = num_y;
            }
            DisplayItem::Rect { x, y, .. } => {
                *x += origin_x + num_x_offset;
                *y = num_y;
            }
            DisplayItem::Path { x, y, .. } => {
                *x += origin_x + num_x_offset;
                *y = num_y;
            }
        }
        state.items.push(item);
    }

    // Place denominator items
    for item in &den_state.items {
        let mut item = item.clone();
        match &mut item {
            DisplayItem::GlyphPath { x, y, .. } => {
                *x += origin_x + den_x_offset;
                *y = den_y;
            }
            DisplayItem::Line { x, y, .. } => {
                *x += origin_x + den_x_offset;
                *y = den_y;
            }
            DisplayItem::Rect { x, y, .. } => {
                *x += origin_x + den_x_offset;
                *y = den_y;
            }
            DisplayItem::Path { x, y, .. } => {
                *x += origin_x + den_x_offset;
                *y = den_y;
            }
        }
        state.items.push(item);
    }

    // Fraction bar
    state.push_line(
        origin_x - FRACTION_BAR_OVERHANG,
        bar_y,
        total_width,
        font_size * FRACTION_BAR_THICKNESS,
        color,
    );

    state.current_x += total_width + FRACTION_BAR_OVERHANG * 2.0;

    // Update bounding box
    let top = bar_y + FRACTION_BAR_THICKNESS / 2.0 + gap + num_height;
    let bottom = bar_y - FRACTION_BAR_THICKNESS / 2.0 - gap - den_depth;
    state.max_y = state.max_y.max(top);
    state.min_y = state.min_y.min(bottom);
}

// ---------------------------------------------------------------------------
// Square root: \sqrt{radicand}
// ---------------------------------------------------------------------------

fn parse_sqrt(
    state: &mut LayoutState,
    content: &str,
    font_size: f64,
    y_offset: f64,
    color: Color,
) {
    let mut inner_state = LayoutState::new();
    parse_expression(&mut Parser::new(content), &mut inner_state, font_size * 0.85, 0.0, color, true);
    let inner_width = inner_state.current_x;
    let inner_height = inner_state.max_y.max(-inner_state.min_y);

    let origin_x = state.current_x;
    let sqrt_height = inner_height + font_size * 0.3;
    let bar_y = sqrt_height;
    let bar_width = inner_width + font_size * 0.2;

    // Radical symbol (√) before the content
    state.push_glyph(SQRT_SYMBOL, sqrt_height * 1.8, y_offset + sqrt_height / 2.0, color);

    // Overline
    let overline_x = origin_x + sqrt_height * 0.9;
    state.push_line(
        overline_x,
        y_offset + bar_y + font_size * 0.1,
        bar_width,
        font_size * SQRT_BAR_THICKNESS,
        color,
    );

    // Place radicand items
    for item in &inner_state.items {
        let mut item = item.clone();
        match &mut item {
            DisplayItem::GlyphPath { x, y, .. } => {
                *x += overline_x + font_size * 0.1;
                *y += y_offset + font_size * 0.15;
            }
            DisplayItem::Line { x, y, .. } => {
                *x += overline_x + font_size * 0.1;
                *y += y_offset + font_size * 0.15;
            }
            DisplayItem::Rect { x, y, .. } => {
                *x += overline_x + font_size * 0.1;
                *y += y_offset + font_size * 0.15;
            }
            DisplayItem::Path { x, y, .. } => {
                *x += overline_x + font_size * 0.1;
                *y += y_offset + font_size * 0.15;
            }
        }
        state.items.push(item);
    }

    state.current_x += sqrt_height * 0.9 + bar_width + font_size * 0.3;
    state.max_y = state.max_y.max(y_offset + bar_y + font_size * 0.2);
    state.min_y = state.min_y.min(y_offset - font_size * 0.1);
}

// ---------------------------------------------------------------------------
// DisplayList → SVG string converter
// ---------------------------------------------------------------------------

fn display_list_to_svg(dl: &DisplayList, font_family: &str, text_color: [f32; 4]) -> String {
    let w = dl.width.max(1.0) as f32;
    let h = dl.total_height().max(1.0) as f32;
    let baseline_y = dl.height as f32;

    // Pre-allocate capacity: header + ~300 bytes per item (avoids O(n²) reallocation)
    let est_cap = 256 + dl.items.len() * 320;
    let mut svg = String::with_capacity(est_cap);

    write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.1}" height="{h:.1}" viewBox="0 0 {w:.1} {h:.1}">"#
    )
    .unwrap();

    for item in &dl.items {
        match item {
            DisplayItem::GlyphPath {
                x, y, scale, char_code, color, ..
            } => {
                let sx = *x as f32;
                let sy = baseline_y - *y as f32;
                let fs = *scale as f32;
                let ch = char::from_u32(*char_code).unwrap_or('?');
                let fill = apply_text_color(color, text_color);
                write_escaped_char(&mut svg, ch);
                write!(
                    svg,
                    r#"</text><text x="{sx}" y="{sy}" font-family="{font_family}" font-size="{fs}px" fill="{fill}">"#
                )
                .unwrap();
            }
            DisplayItem::Line {
                x, y, width, thickness, color, dashed,
            } => {
                let sx = *x as f32;
                let sy = baseline_y - *y as f32 - *thickness as f32 / 2.0;
                let sw = *width as f32;
                let st = (*thickness as f32).max(0.5);
                let fill = apply_text_color(color, text_color);
                if *dashed {
                    write!(
                        svg,
                        r#"<line x1="{sx}" y1="{sy}" x2="{x2}" y2="{sy}" stroke="{fill}" stroke-width="{st}" stroke-dasharray="{st} {st}"/>"#,
                        x2 = sx + sw,
                    )
                    .unwrap();
                } else {
                    write!(
                        svg,
                        r#"<rect x="{sx}" y="{sy}" width="{sw}" height="{st}" fill="{fill}"/>"#
                    )
                    .unwrap();
                }
            }
            DisplayItem::Rect {
                x, y, width, height, color,
            } => {
                let sx = *x as f32;
                let sy = baseline_y - *y as f32 - *height as f32;
                let sw = *width as f32;
                let sh = *height as f32;
                let fill = apply_text_color(color, text_color);
                write!(
                    svg,
                    r#"<rect x="{sx}" y="{sy}" width="{sw}" height="{sh}" fill="{fill}"/>"#
                )
                .unwrap();
            }
            DisplayItem::Path {
                x, y, commands, fill, color,
            } => {
                let tx = *x as f32;
                let ty = baseline_y - *y as f32;
                let path_d = commands_to_svg_d(commands);
                let stroke_color = apply_text_color(color, text_color);
                if *fill {
                    write!(
                        svg,
                        r#"<path d="{path_d}" transform="translate({tx},{ty}) scale(1,-1)" fill="{stroke_color}"/>"#
                    )
                    .unwrap();
                } else {
                    write!(
                        svg,
                        r#"<path d="{path_d}" transform="translate({tx},{ty}) scale(1,-1)" fill="none" stroke="{stroke_color}" stroke-width="0.5"/>"#
                    )
                    .unwrap();
                }
            }
        }
    }

    svg.push_str("</svg>");
    svg
}

/// Append a character to the SVG buffer, XML-escaping only when necessary.
fn write_escaped_char(svg: &mut String, ch: char) {
    match ch {
        '<' => svg.push_str("&lt;"),
        '>' => svg.push_str("&gt;"),
        '&' => svg.push_str("&amp;"),
        '"' => svg.push_str("&quot;"),
        '\'' => svg.push_str("&apos;"),
        _ => svg.push(ch),
    }
}

fn apply_text_color(item_color: &Color, user_color: [f32; 4]) -> String {
    let r = (item_color.r * user_color[0] * 255.0) as u8;
    let g = (item_color.g * user_color[1] * 255.0) as u8;
    let b = (item_color.b * user_color[2] * 255.0) as u8;
    let a = item_color.a * user_color[3];
    if (a - 1.0).abs() < f32::EPSILON {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("rgba({r},{g},{b},{a:.2})")
    }
}

fn commands_to_svg_d(commands: &[PathCommand]) -> String {
    let mut d = String::new();
    for cmd in commands {
        match cmd {
            PathCommand::MoveTo { x, y } => d.push_str(&format!("M{x} {y} ")),
            PathCommand::LineTo { x, y } => d.push_str(&format!("L{x} {y} ")),
            PathCommand::CubicTo { x1, y1, x2, y2, x, y } => {
                d.push_str(&format!("C{x1} {y1} {x2} {y2} {x} {y} "))
            }
            PathCommand::QuadTo { x1, y1, x, y } => {
                d.push_str(&format!("Q{x1} {y1} {x} {y} "))
            }
            PathCommand::Close => d.push_str("Z "),
        }
    }
    d.trim_end().to_string()
}

// ---------------------------------------------------------------------------
// SVG parsing
// ---------------------------------------------------------------------------

fn parse_svg(svg_bytes: &[u8], dpi: f32) -> Option<usvg::Tree> {
    let mut opt = usvg::Options::default();
    opt.fontdb = get_fontdb().clone().into();
    opt.dpi = dpi;
    usvg::Tree::from_data(svg_bytes, &opt).ok()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn build_cache(
    formula: &str,
    font_family: &str,
    settings: &LaTeXDisplay,
    text_color: &[f32; 4],
) -> Option<CachedLaTeX> {
    let dl = parse_formula(formula, settings.math_style)?;
    if dl.width <= 0.0 || dl.total_height() <= 0.0 {
        return None;
    }
    let svg = display_list_to_svg(&dl, font_family, *text_color);
    let svg_bytes = svg.as_bytes();
    let tree = parse_svg(svg_bytes, settings.dpi)?;
    let size = tree.size();
    let w = size.width();
    let h = size.height();
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    Some(CachedLaTeX {
        tree,
        native_w: w,
        native_h: h,
        dpi: settings.dpi,
        formula_hash: CachedLaTeX::hash_formula(formula),
        math_style: settings.math_style as u8,
        text_color: *text_color,
        font_name_hash: CachedLaTeX::hash_str(font_family),
    })
}

pub fn render_latex(
    formula: &str,
    font_family: &str,
    cache: Option<&CachedLaTeX>,
    settings: &LaTeXDisplay,
    pos_x: f32,
    pos_y: f32,
    dst_buf: &mut [u8],
    output_w: usize,
    output_h: usize,
    bg: [f32; 4],
    text_color: [f32; 4],
) -> Option<CachedLaTeX> {
    let cached = if let Some(c) = cache {
        if c.is_valid(formula, settings.dpi, settings.math_style, &text_color, font_family) {
            c.clone()
        } else {
            build_cache(formula, font_family, settings, &text_color)?
        }
    } else {
        build_cache(formula, font_family, settings, &text_color)?
    };

    let is_new_cache = cache.map_or(true, |c| !std::ptr::eq(c, &cached));

    let scale = compute_effective_scale(settings, cached.native_w, cached.native_h, output_w, output_h);

    let transform = build_transform(
        cached.native_w,
        cached.native_h,
        scale,
        settings.rotation,
        pos_x,
        pos_y,
        output_w,
        output_h,
    );

    let mut svg_pixmap = match tiny_skia::Pixmap::new(output_w as u32, output_h as u32) {
        Some(p) => p,
        None => return if is_new_cache { Some(cached) } else { None },
    };

    // Render the cached tree directly — no per-frame SVG re-generation
    resvg::render(&cached.tree, transform, &mut svg_pixmap.as_mut());

    composite_svg_over_bg(
        svg_pixmap.data(),
        dst_buf,
        settings.opacity,
        bg,
        output_w,
        output_h,
    );

    if is_new_cache { Some(cached) } else { None }
}

// ---------------------------------------------------------------------------
// Scale & transform helpers
// ---------------------------------------------------------------------------

fn compute_effective_scale(
    settings: &LaTeXDisplay,
    svg_w: f32,
    svg_h: f32,
    output_w: usize,
    output_h: usize,
) -> f32 {
    let sx = output_w as f32 / svg_w;
    let sy = output_h as f32 / svg_h;
    let fit_scale = sx.min(sy);
    // font_size multiplies output size relative to LAYOUT_FONT_SIZE (10.0)
    fit_scale * settings.scale * (settings.font_size / LAYOUT_FONT_SIZE as f32)
}

fn build_transform(
    svg_w: f32,
    svg_h: f32,
    scale: f32,
    angle_deg: f32,
    pos_x: f32,
    pos_y: f32,
    output_w: usize,
    output_h: usize,
) -> tiny_skia::Transform {
    let tgt_x = pos_x * output_w as f32;
    let tgt_y = pos_y * output_h as f32;
    let mut t = tiny_skia::Transform::from_translate(tgt_x, tgt_y);
    t = t.pre_concat(tiny_skia::Transform::from_rotate(angle_deg));
    t = t.pre_concat(tiny_skia::Transform::from_scale(scale, scale));
    t = t.pre_concat(tiny_skia::Transform::from_translate(-svg_w / 2.0, -svg_h / 2.0));
    t
}

fn composite_svg_over_bg(
    svg_pixels: &[u8],
    dst: &mut [u8],
    opacity: f32,
    bg: [f32; 4],
    output_w: usize,
    output_h: usize,
) {
    crate::blend::composite_over_bg(svg_pixels, dst, opacity, bg, output_w, output_h);
}
