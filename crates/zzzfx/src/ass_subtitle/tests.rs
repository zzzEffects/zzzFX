use super::*;
use crate::settings::ass_subtitle::AssBlendMode;
use std::collections::HashMap;

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
    assert!(
        (c[3] - 1.0).abs() < 0.01,
        "No-alpha colors should default to opaque"
    );
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
    assert_eq!(parser::parse_ass_timestamp("0:00:00.00"), Some(0));
    assert_eq!(parser::parse_ass_timestamp("0:00:01.00"), Some(1000));
    assert_eq!(parser::parse_ass_timestamp("0:01:00.00"), Some(60000));
    assert_eq!(parser::parse_ass_timestamp("1:00:00.00"), Some(3600000));
    assert_eq!(parser::parse_ass_timestamp("0:00:00.50"), Some(500));
    assert_eq!(parser::parse_ass_timestamp("0:00:05.25"), Some(5250));
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
    assert!((result.styles[0].fontsize - 48.0).abs() < 0.1);
    let pc = result.styles[0].primary_color;
    assert!((pc[3] - 1.0).abs() < 0.01, "Primary color should be opaque");

    assert_eq!(result.events[0].start_ms, 1000);
    assert_eq!(result.events[0].end_ms, 5000);
    assert_eq!(result.events[0].text, "Hello World!");

    assert_eq!(result.events[1].start_ms, 3000);
    assert_eq!(result.events[1].end_ms, 8000);
    assert_eq!(result.events[1].text, "Line two");
}

#[test]
fn test_parse_override_tags() {
    // Without trailing \r: tags accumulate
    let (clean, tags) =
        parse_override_tags(r"{\fs50}{\b1}{\i1}Hello {\c&H0000FF&}World!");
    assert_eq!(clean, "Hello World!");
    assert_eq!(tags.fontsize, Some(50.0));
    assert_eq!(tags.bold, Some(true));
    assert_eq!(tags.italic, Some(true));
    let color = tags.primary_color.unwrap();
    assert!((color[0] - 1.0).abs() < 0.01);
    assert!(color[1] < 0.01);
    assert!(color[2] < 0.01);

    // With trailing \r: all tags are reset to defaults
    let (clean2, tags2) =
        parse_override_tags(r"{\fs50}{\b1}Hello{\r}");
    assert_eq!(clean2, "Hello");
    assert_eq!(tags2.fontsize, None);
    assert_eq!(tags2.bold, None);
}

#[test]
fn test_active_events() {
    let ass = AssScript {
        info: HashMap::new(),
        styles: vec![],
        events: vec![
            OwnedEvent {
                start_ms: 1000,
                end_ms: 5000,
                ..Default::default()
            },
            OwnedEvent {
                start_ms: 3000,
                end_ms: 8000,
                ..Default::default()
            },
        ],
        play_res_x: None,
        play_res_y: None,
    };
    let active: Vec<_> = ass
        .events
        .iter()
        .filter(|e| e.start_ms <= 0 && 0 < e.end_ms)
        .collect();
    assert_eq!(active.len(), 0);
    let active: Vec<_> = ass
        .events
        .iter()
        .filter(|e| e.start_ms <= 2000 && 2000 < e.end_ms)
        .collect();
    assert_eq!(active.len(), 1);
    let active: Vec<_> = ass
        .events
        .iter()
        .filter(|e| e.start_ms <= 4000 && 4000 < e.end_ms)
        .collect();
    assert_eq!(active.len(), 2);
    let active: Vec<_> = ass
        .events
        .iter()
        .filter(|e| e.start_ms <= 6000 && 6000 < e.end_ms)
        .collect();
    assert_eq!(active.len(), 1);
    let active: Vec<_> = ass
        .events
        .iter()
        .filter(|e| e.start_ms <= 9000 && 9000 < e.end_ms)
        .collect();
    assert_eq!(active.len(), 0);
}

#[test]
fn test_font_cache_has_fonts() {
    assert!(
        !font::global_font_entries().is_empty(),
        "Font cache should not be empty on Windows"
    );
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
    if font::global_font_entries().is_empty() {
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
    assert!(
        non_zero > 0,
        "Expected non-zero pixels in output, but all are transparent"
    );
}
