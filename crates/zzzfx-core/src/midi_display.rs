use midly::{
    MidiMessage, MetaMessage, Smf, Timing, TrackEventKind,
};

use crate::settings::midi_display::{
    MidiBpmSource, MidiNoteColorMode, MidiOrientation, MidiTrackFilterMode, ZzzMidiDisplay,
};

// ---------------------------------------------------------------------------
// MIDI data structures (owned, no lifetime ties to source bytes)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct NoteBlock {
    pub start_tick: u64,
    pub end_tick: u64,
    pub key: u8,
    pub velocity: u8,
    pub channel: u8,
    pub track: u32,
}

#[derive(Clone, Debug)]
pub struct TempoEvent {
    pub tick: u64,
    pub us_per_beat: u32,
}

#[derive(Clone, Debug)]
pub struct MidiData {
    pub note_blocks: Vec<NoteBlock>,
    pub tempo_map: Vec<TempoEvent>,
    pub ticks_per_beat: u16,
    pub total_duration_seconds: f64,
    pub note_seconds: Vec<(f64, f64)>, // (start_s, end_s) pre-computed at parse time
}

// ---------------------------------------------------------------------------
// MIDI parsing
// ---------------------------------------------------------------------------

pub fn parse_midi_file(bytes: &[u8]) -> Result<MidiData, String> {
    let smf = Smf::parse(bytes).map_err(|e| format!("MIDI parse error: {e}"))?;

    let ticks_per_beat = match smf.header.timing {
        Timing::Metrical(tpb) => tpb.as_int(),
        _ => return Err("Only metrical (PPQ) timing is supported".into()),
    };

    let mut note_blocks: Vec<NoteBlock> = Vec::new();
    let mut tempo_events: Vec<TempoEvent> = Vec::new();
    let mut max_end_tick: u64 = 0;

    for (track_idx, track) in smf.tracks.iter().enumerate() {
        let mut abs_tick: u64 = 0;
        // Active notes: (key, velocity, start_tick, channel)
        let mut active_notes: Vec<(u8, u8, u64, u8)> = Vec::new();

        for event in track {
            abs_tick = abs_tick.saturating_add(event.delta.as_int() as u64);

            match &event.kind {
                TrackEventKind::Midi { channel, message } => {
                    let ch = channel.as_int();
                    match message {
                        MidiMessage::NoteOn { key, vel } => {
                            let k = key.as_int();
                            let v = vel.as_int();
                            if v > 0 {
                                active_notes.push((k, v, abs_tick, ch));
                            } else {
                                // velocity 0 = NoteOff — use the original velocity from the NoteOn
                                active_notes.retain(|(ak, note_vel, start, ach)| {
                                    if *ak == k && *ach == ch {
                                        note_blocks.push(NoteBlock {
                                            start_tick: *start,
                                            end_tick: abs_tick,
                                            key: k,
                                            velocity: *note_vel,
                                            channel: ch,
                                            track: track_idx as u32,
                                        });
                                        max_end_tick = max_end_tick.max(abs_tick);
                                        false
                                    } else {
                                        true
                                    }
                                });
                            }
                        }
                        MidiMessage::NoteOff { key, vel: _release_vel } => {
                            let k = key.as_int();
                            active_notes.retain(|(ak, note_vel, start, ach)| {
                                if *ak == k && *ach == ch {
                                    note_blocks.push(NoteBlock {
                                        start_tick: *start,
                                        end_tick: abs_tick,
                                        key: k,
                                        velocity: *note_vel,
                                        channel: ch,
                                        track: track_idx as u32,
                                    });
                                    max_end_tick = max_end_tick.max(abs_tick);
                                    false
                                } else {
                                    true
                                }
                            });
                        }
                        _ => {}
                    }
                }
                TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
                    tempo_events.push(TempoEvent {
                        tick: abs_tick,
                        us_per_beat: tempo.as_int(),
                    });
                }
                _ => {}
            }
        }

        // Close any still-active notes at end of track
        for (k, v, start, ch) in active_notes {
            note_blocks.push(NoteBlock {
                start_tick: start,
                end_tick: abs_tick,
                key: k,
                velocity: v,
                channel: ch,
                track: track_idx as u32,
            });
            max_end_tick = max_end_tick.max(abs_tick);
        }
    }

    // Sort tempo events by tick
    tempo_events.sort_by_key(|t| t.tick);

    // Ensure at least one tempo event (default 120 BPM = 500_000 us/beat)
    if tempo_events.is_empty() || tempo_events[0].tick != 0 {
        tempo_events.insert(0, TempoEvent { tick: 0, us_per_beat: 500_000 });
    }

    // Sort note_blocks by start_tick (enables binary search for visible notes)
    note_blocks.sort_by_key(|n| n.start_tick);

    let total_duration_seconds =
        ticks_to_seconds(max_end_tick, &tempo_events, ticks_per_beat);

    // Pre-compute note start/end times in seconds
    let note_seconds: Vec<(f64, f64)> = note_blocks
        .iter()
        .map(|n| {
            let start_s = ticks_to_seconds(n.start_tick, &tempo_events, ticks_per_beat);
            let end_s = ticks_to_seconds(n.end_tick, &tempo_events, ticks_per_beat);
            (start_s, end_s)
        })
        .collect();

    Ok(MidiData { note_blocks, tempo_map: tempo_events, ticks_per_beat, total_duration_seconds, note_seconds })
}

// ---------------------------------------------------------------------------
// Tick-to-seconds conversion
// ---------------------------------------------------------------------------

pub fn ticks_to_seconds(ticks: u64, tempo_map: &[TempoEvent], ticks_per_beat: u16) -> f64 {
    let tpb = ticks_per_beat as f64;
    let mut seconds = 0.0;
    let mut prev_tick: u64 = 0;
    let mut current_tempo: u32 = 500_000; // 120 BPM default

    for te in tempo_map {
        if te.tick > ticks {
            break;
        }
        let delta = (te.tick - prev_tick) as f64;
        seconds += delta * current_tempo as f64 / (tpb * 1_000_000.0);
        prev_tick = te.tick;
        current_tempo = te.us_per_beat;
    }
    // Final segment
    let delta = (ticks.saturating_sub(prev_tick)) as f64;
    seconds += delta * current_tempo as f64 / (tpb * 1_000_000.0);

    seconds
}

// ---------------------------------------------------------------------------
// Color utilities
// ---------------------------------------------------------------------------

/// 16 distinct colors for MIDI channels
const CHANNEL_COLORS: [(f32, f32, f32); 16] = [
    (1.0, 0.0, 0.0),     // 0:  Red
    (0.0, 0.8, 0.0),     // 1:  Green
    (0.0, 0.4, 1.0),     // 2:  Blue
    (1.0, 1.0, 0.0),     // 3:  Yellow
    (1.0, 0.4, 0.0),     // 4:  Orange
    (0.6, 0.0, 1.0),     // 5:  Purple
    (0.0, 0.8, 0.8),     // 6:  Cyan
    (1.0, 0.0, 0.6),     // 7:  Pink
    (0.5, 0.5, 0.5),     // 8:  Gray
    (0.6, 0.3, 0.0),     // 9:  Brown
    (0.0, 0.6, 0.4),     // 10: Teal
    (0.8, 0.2, 0.4),     // 11: Rose
    (0.4, 0.6, 1.0),     // 12: Sky Blue
    (1.0, 0.6, 0.2),     // 13: Gold
    (0.3, 0.7, 0.2),     // 14: Lime
    (0.7, 0.3, 0.8),     // 15: Lavender
];

/// Generate a color from a track index using a simple hash.
fn track_color(track: u32) -> (f32, f32, f32) {
    let h = track.wrapping_mul(2654435761);
    let r = ((h >> 16) & 0xFF) as f32 / 255.0;
    let g = ((h >> 8) & 0xFF) as f32 / 255.0;
    let b = (h & 0xFF) as f32 / 255.0;
    // Ensure minimum brightness for visibility
    let max = r.max(g).max(b);
    if max < 0.3 {
        let scale = 0.3 / max.max(0.01);
        (r * scale, g * scale, b * scale)
    } else {
        (r, g, b)
    }
}

/// Velocity gradient: blue (low) → green (mid) → red (high)
fn velocity_color(velocity: u8) -> (f32, f32, f32) {
    let v = velocity as f32 / 127.0;
    if v < 0.5 {
        let t = v * 2.0;
        (0.0, t, 1.0 - t)
    } else {
        let t = (v - 0.5) * 2.0;
        (t, 1.0 - t, 0.0)
    }
}

/// Rainbow gradient based on pitch (key 0-127)
fn pitch_color(key: u8) -> (f32, f32, f32) {
    let hue = key as f32 / 127.0 * 300.0; // 0-300 degrees (red to magenta)
    let h = hue / 60.0;
    let c = 1.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h.floor() as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r, g, b)
}

fn get_note_color(mode: MidiNoteColorMode, note: &NoteBlock) -> (f32, f32, f32) {
    match mode {
        MidiNoteColorMode::Velocity => velocity_color(note.velocity),
        MidiNoteColorMode::Channel => {
            let idx = (note.channel as usize) % 16;
            CHANNEL_COLORS[idx]
        }
        MidiNoteColorMode::Track => track_color(note.track),
        MidiNoteColorMode::Pitch => pitch_color(note.key),
        MidiNoteColorMode::Solid => (1.0, 1.0, 1.0), // placeholder; caller overrides
    }
}

// ---------------------------------------------------------------------------
// Thread-local buffers (avoid per-frame allocations)
// ---------------------------------------------------------------------------

thread_local! {
    static VISIBLE_STASH: std::cell::RefCell<Vec<usize>> = std::cell::RefCell::new(Vec::with_capacity(4096));
}

// ---------------------------------------------------------------------------
// Drawing primitives (into u8 RGBA buffer)
// ---------------------------------------------------------------------------

fn blend_pixel_into(dst: &mut [u8], stride: usize, px: usize, py: usize, w: usize, h: usize, r: f32, g: f32, b: f32, a: f32) {
    if px >= w || py >= h {
        return;
    }
    let idx = py * stride + px * 4;
    let src_a = a.clamp(0.0, 1.0);
    let dst_a = dst[idx + 3] as f32 / 255.0;

    // Fast path: opaque source onto transparent destination
    if dst_a < 0.001 && src_a >= 1.0 {
        dst[idx] = (r * 255.0).round().clamp(0.0, 255.0) as u8;
        dst[idx + 1] = (g * 255.0).round().clamp(0.0, 255.0) as u8;
        dst[idx + 2] = (b * 255.0).round().clamp(0.0, 255.0) as u8;
        dst[idx + 3] = 255;
        return;
    }

    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a < 0.001 {
        dst[idx] = 0;
        dst[idx + 1] = 0;
        dst[idx + 2] = 0;
        dst[idx + 3] = 0;
        return;
    }
    let dst_r = dst[idx] as f32 / 255.0;
    let dst_g = dst[idx + 1] as f32 / 255.0;
    let dst_b = dst[idx + 2] as f32 / 255.0;
    dst[idx] = ((r * src_a + dst_r * dst_a * (1.0 - src_a)) / out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[idx + 1] = ((g * src_a + dst_g * dst_a * (1.0 - src_a)) / out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[idx + 2] = ((b * src_a + dst_b * dst_a * (1.0 - src_a)) / out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn fill_rect(
    dst: &mut [u8], stride: usize, img_w: usize, img_h: usize,
    x: i32, y: i32, w: i32, h: i32, r: f32, g: f32, b: f32, a: f32,
) {
    if a <= 0.0 || w <= 0 || h <= 0 {
        return;
    }
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(img_w as i32);
    let y1 = (y + h).min(img_h as i32);
    for py in y0..y1 {
        for px in x0..x1 {
            blend_pixel_into(dst, stride, px as usize, py as usize, img_w, img_h, r, g, b, a);
        }
    }
}

fn fill_rounded_rect(
    dst: &mut [u8], stride: usize, img_w: usize, img_h: usize,
    x: i32, y: i32, w: i32, h: i32, radius: f32,
    r: f32, g: f32, b: f32, a: f32,
) {
    if a <= 0.0 || w <= 0 || h <= 0 {
        return;
    }
    let r_i = (radius as i32).min(w / 2).min(h / 2);
    let r_f = r_i as f32;

    // Center rectangle (full width, between corner rows)
    fill_rect(dst, stride, img_w, img_h, x, y + r_i, w, h - 2 * r_i, r, g, b, a);
    // Left and right edge rectangles (between corners)
    fill_rect(dst, stride, img_w, img_h, x + r_i, y, w - 2 * r_i, h, r, g, b, a);

    if r_i <= 0 {
        return;
    }

    // Four corners using squared distance (no sqrt)
    let threshold = r_f + 0.5;
    let threshold_sq = threshold * threshold;
    let corners = [
        (x + r_i - 1, y + r_i - 1),                    // top-left
        (x + w - r_i, y + r_i - 1),                     // top-right
        (x + r_i - 1, y + h - r_i),                     // bottom-left
        (x + w - r_i, y + h - r_i),                      // bottom-right
    ];
    for &(cx, cy) in &corners {
        for dy in (-r_i)..r_i {
            for dx in (-r_i)..r_i {
                let dist_sq = (dx * dx + dy * dy) as f32;
                if dist_sq <= threshold_sq {
                    blend_pixel_into(
                        dst, stride,
                        (cx + dx) as usize, (cy + dy) as usize,
                        img_w, img_h, r, g, b, a,
                    );
                }
            }
        }
    }
}

fn fill_rounded_rect_with_border(
    dst: &mut [u8], stride: usize, img_w: usize, img_h: usize,
    x: i32, y: i32, w: i32, h: i32, radius: f32, border_thickness: f32,
    fill_r: f32, fill_g: f32, fill_b: f32, fill_a: f32,
    border_r: f32, border_g: f32, border_b: f32, border_a: f32,
) {
    let bt = border_thickness as i32;
    // Draw fill first (inset by border)
    fill_rounded_rect(
        dst, stride, img_w, img_h,
        x + bt, y + bt,
        (w - 2 * bt).max(0), (h - 2 * bt).max(0),
        (radius - border_thickness).max(0.0),
        fill_r, fill_g, fill_b, fill_a,
    );
    // Draw border ring only (avoids re-blending the entire interior)
    if border_thickness > 0.0 && border_a > 0.0 {
        // Top edge
        fill_rect(dst, stride, img_w, img_h, x, y, w, bt, border_r, border_g, border_b, border_a);
        // Bottom edge
        fill_rect(dst, stride, img_w, img_h, x, y + h - bt, w, bt, border_r, border_g, border_b, border_a);
        // Left edge (between top and bottom)
        fill_rect(dst, stride, img_w, img_h, x, y + bt, bt, h - 2 * bt, border_r, border_g, border_b, border_a);
        // Right edge
        fill_rect(dst, stride, img_w, img_h, x + w - bt, y + bt, bt, h - 2 * bt, border_r, border_g, border_b, border_a);
        // Corner arcs for border: draw outer rounded corners over the fill
        let r_i = (radius as i32).min(w / 2).min(h / 2);
        if r_i > 0 {
            let threshold = radius + 0.5;
            let threshold_sq = threshold * threshold;
            let corners = [
                (x + r_i - 1, y + r_i - 1),
                (x + w - r_i, y + r_i - 1),
                (x + r_i - 1, y + h - r_i),
                (x + w - r_i, y + h - r_i),
            ];
            for &(cx, cy) in &corners {
                for dy in (-r_i)..r_i {
                    for dx in (-r_i)..r_i {
                        let dist_sq = (dx * dx + dy * dy) as f32;
                        if dist_sq <= threshold_sq {
                            let inner_dist_sq = dist_sq; // reuse
                            let inner_threshold = (radius - border_thickness).max(0.0) + 0.5;
                            // Only draw border if outside the inner fill radius
                            if inner_threshold <= 0.0 || inner_dist_sq > inner_threshold * inner_threshold {
                                blend_pixel_into(
                                    dst, stride,
                                    (cx + dx) as usize, (cy + dy) as usize,
                                    img_w, img_h,
                                    border_r, border_g, border_b, border_a,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Piano keyboard drawing
// ---------------------------------------------------------------------------

/// White key positions within an octave: C(0), D(2), E(4), F(5), G(7), A(9), B(11)
const WHITE_KEY_INDICES: [usize; 7] = [0, 2, 4, 5, 7, 9, 11];

fn is_white_key(key: u8) -> bool {
    WHITE_KEY_INDICES.contains(&((key % 12) as usize))
}

fn draw_keyboard(
    dst: &mut [u8], stride: usize, img_w: usize, img_h: usize,
    orientation: MidiOrientation,
    keyboard_start: i32, keyboard_size: i32,
    key_range_min: i32, key_range_max: i32,
    pixels_per_key: f32, key_count: i32,
) {
    let white = (1.0, 1.0, 1.0);
    let black = (0.15, 0.15, 0.15);
    let white_border = (0.5, 0.5, 0.5);

    match orientation {
        MidiOrientation::Horizontal => {
            // Keyboard on the LEFT side, keys stacked vertically
            let kw = keyboard_size;
            for key in key_range_min..=key_range_max {
                if key < 0 || key > 127 { continue; }
                let key_idx = key - key_range_min;
                // Bottom-to-top: key 0 at bottom
                let ky = ((key_count - 1 - key_idx) as f32 * pixels_per_key) as i32;
                let kh = pixels_per_key.ceil() as i32;

                if is_white_key(key as u8) {
                    fill_rect(dst, stride, img_w, img_h,
                        keyboard_start, ky, kw, kh.max(2),
                        white.0, white.1, white.2, 0.9);
                    // subtle border between white keys
                    fill_rect(dst, stride, img_w, img_h,
                        keyboard_start, ky, kw, 1,
                        white_border.0, white_border.1, white_border.2, 0.3);
                } else {
                    let bkh = (kh as f32 * 0.6) as i32;
                    let bky = ky + (kh - bkh) / 2;
                    let bkw = (kw as f32 * 0.6) as i32;
                    fill_rect(dst, stride, img_w, img_h,
                        keyboard_start, bky, bkw, bkh.max(1),
                        black.0, black.1, black.2, 0.9);
                }
            }
        }
        MidiOrientation::Vertical => {
            // Keyboard on the TOP side, keys stacked horizontally
            let kh = keyboard_size;
            for key in key_range_min..=key_range_max {
                if key < 0 || key > 127 { continue; }
                let key_idx = key - key_range_min;
                let kx = (key_idx as f32 * pixels_per_key) as i32;
                let kw = pixels_per_key.ceil() as i32;

                if is_white_key(key as u8) {
                    fill_rect(dst, stride, img_w, img_h,
                        kx, keyboard_start, kw.max(2), kh,
                        white.0, white.1, white.2, 0.9);
                    fill_rect(dst, stride, img_w, img_h,
                        kx, keyboard_start, 1, kh,
                        white_border.0, white_border.1, white_border.2, 0.3);
                } else {
                    let bkw = (kw as f32 * 0.6) as i32;
                    let bkx = kx + (kw - bkw) / 2;
                    let bkh = (kh as f32 * 0.6) as i32;
                    fill_rect(dst, stride, img_w, img_h,
                        bkx, keyboard_start, bkw.max(1), bkh,
                        black.0, black.1, black.2, 0.9);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main render function
// ---------------------------------------------------------------------------

impl ZzzMidiDisplay {
    pub fn render(
        &self,
        midi_data: &MidiData,
        dst_buf: &mut [u8],
        output_width: usize,
        output_height: usize,
        current_time_seconds: f64,
    ) {
        let img_w = output_width;
        let img_h = output_height;
        let stride = img_w * 4;

        if img_w == 0 || img_h == 0 {
            return;
        }

        // --- Effective time ---
        let effective_time = (current_time_seconds * self.speed as f64) + self.time_offset as f64;
        let effective_time = if self.loop_playback && midi_data.total_duration_seconds > 0.0 {
            effective_time.rem_euclid(midi_data.total_duration_seconds)
        } else {
            effective_time
        };

        // --- Background ---
        let bg_a = (self.background_color_a * self.background_opacity).clamp(0.0, 1.0);
        fill_rect(
            dst_buf, stride, img_w, img_h,
            0, 0, img_w as i32, img_h as i32,
            self.background_color_r, self.background_color_g, self.background_color_b, bg_a,
        );

        if midi_data.note_blocks.is_empty() {
            return;
        }

        // --- Effective BPM ---
        let effective_bpm: f32 = match self.bpm_source {
            MidiBpmSource::UserSpecified => self.user_bpm,
            MidiBpmSource::FromMidi => {
                // Use first tempo event for visual scaling
                let default_tempo = midi_data.tempo_map.first().map(|t| t.us_per_beat).unwrap_or(500_000);
                60_000_000.0 / default_tempo as f32
            }
        };

        // --- Layout calculations ---
        let (time_axis_len, pitch_axis_len, keyboard_size): (usize, usize, i32) = match self.orientation {
            MidiOrientation::Horizontal => {
                let kb = if self.show_keyboard {
                    (img_w as f32 * self.keyboard_width) as i32
                } else {
                    0
                };
                let time_len = ((img_w as i32) - kb).max(1) as usize;
                (time_len, img_h, kb)
            }
            MidiOrientation::Vertical => {
                let kb = if self.show_keyboard {
                    (img_h as f32 * self.keyboard_width) as i32
                } else {
                    0
                };
                let time_len = ((img_h as i32) - kb).max(1) as usize;
                (time_len, img_w, kb)
            }
        };

        let key_min = self.key_range_min.min(self.key_range_max);
        let key_max = self.key_range_max.max(self.key_range_min);
        let key_count = (key_max - key_min + 1).max(1);
        let pixels_per_key = self.note_height_min.max(pitch_axis_len as f32 / key_count as f32);

        // Compute pixels per second from BPM
        let pixels_per_beat: f32 = 40.0;
        let beats_per_second = effective_bpm / 60.0;
        let pixels_per_second = pixels_per_beat * beats_per_second;

        let visible_seconds = time_axis_len as f32 / pixels_per_second.max(0.001);
        // Current time at 25% from start of time axis
        let view_start = effective_time - visible_seconds as f64 * 0.25;
        let view_end = view_start + visible_seconds as f64;

        // --- Try GPU render ---
        #[cfg(feature = "gpu")]
        {
            if !midi_data.note_blocks.is_empty() {
                let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Build visible note data for GPU
                    let min_vel = self.minimum_velocity as u8;
                    let n_seconds = &midi_data.note_seconds;
                    let mut gpu_notes: Vec<crate::gpu::midi_display::NoteGpu> = Vec::with_capacity(1024);
                    for i in 0..midi_data.note_blocks.len() {
                        let note = &midi_data.note_blocks[i];
                        if note.velocity < min_vel { continue; }
                        if self.track_filter_mode == MidiTrackFilterMode::SpecificTrack
                            && note.track != self.track_number as u32
                        { continue; }
                        if note.key < key_min as u8 || note.key > key_max as u8 { continue; }
                        let (start_s, end_s) = n_seconds[i];
                        if self.quantize_display {
                            let beat_duration = 60.0 / effective_bpm as f64;
                            let ss = (start_s / beat_duration).round() * beat_duration;
                            let se = (end_s / beat_duration).round() * beat_duration;
                            if !(ss < view_end && se > view_start) { continue; }
                        } else if !(start_s < view_end && end_s > view_start) { continue; }

                        // Compute note data same as CPU path
                        let (ss, se) = n_seconds[i];
                        let d = (se - ss).max(0.01);
                        let nw = (d * pixels_per_second as f64).max(1.0) as i32;
                        let (base_r, base_g, base_b) = match self.note_color_mode {
                            MidiNoteColorMode::Solid => (self.note_color_r, self.note_color_g, self.note_color_b),
                            other => get_note_color(other, note),
                        };
                        let vel_factor = if self.velocity_affects_brightness {
                            (note.velocity as f32 / 127.0).max(0.1)
                        } else { 1.0 };
                        let nr = (base_r * vel_factor).clamp(0.0, 1.0);
                        let ng = (base_g * vel_factor).clamp(0.0, 1.0);
                        let nb = (base_b * vel_factor).clamp(0.0, 1.0);
                        let mut na = (self.note_color_a * self.note_opacity).clamp(0.0, 1.0);
                        if self.velocity_affects_opacity { na *= note.velocity as f32 / 127.0; }
                        let nh = if self.show_velocity_as_height {
                            (pixels_per_key * note.velocity as f32 / 127.0).max(1.0)
                        } else { pixels_per_key };
                        let key_idx = (note.key as i32 - key_min) as i32;
                        let (nx, ny, nw2, nh2) = match self.orientation {
                            MidiOrientation::Horizontal => {
                                let nx = keyboard_size + ((ss - view_start) * pixels_per_second as f64) as i32;
                                let ny = ((key_count - 1 - key_idx) as f32 * pixels_per_key
                                    + (pixels_per_key - nh) * 0.5) as i32;
                                (nx, ny, nw, nh.ceil() as i32)
                            }
                            MidiOrientation::Vertical => {
                                let ny = keyboard_size + ((ss - view_start) * pixels_per_second as f64) as i32;
                                let nx = (key_idx as f32 * pixels_per_key
                                    + (pixels_per_key - nh) * 0.5) as i32;
                                (nx, ny, nh.ceil() as i32, nw)
                            }
                        };
                        let b_a = (self.note_border_color_a * self.note_border_opacity).clamp(0.0, 1.0);
                        let cr = self.note_corner_radius.min((nw2 as f32 / 2.0).floor()).min((nh2 as f32 / 2.0).floor());
                        gpu_notes.push(crate::gpu::midi_display::NoteGpu {
                            x: nx, y: ny, w: nw2, h: nh2,
                            corner_radius: cr,
                            border_thickness: self.note_border_thickness,
                            fill_r: nr, fill_g: ng, fill_b: nb, fill_a: na,
                            border_r: self.note_border_color_r,
                            border_g: self.note_border_color_g,
                            border_b: self.note_border_color_b,
                            border_a: b_a,
                        });
                    }

                    let rel_time = effective_time - view_start;
                    let ind_pos = keyboard_size + (rel_time * pixels_per_second as f64) as i32;
                    let bg_a = (self.background_color_a * self.background_opacity).clamp(0.0, 1.0);

                    crate::gpu::midi_display::try_midi_display_gpu_render(
                        img_w as u32, img_h as u32,
                        self.background_color_r, self.background_color_g, self.background_color_b, bg_a,
                        0, keyboard_size,
                        self.orientation as u32,
                        key_min, key_max,
                        pixels_per_key,
                        ind_pos,
                        keyboard_size,
                        &gpu_notes,
                        dst_buf,
                    )
                }));
                match gpu_result {
                    Ok(Ok(true)) => return,
                    _ => {} // GPU unavailable, fall through to CPU
                }
            }
        }

        // --- Draw keyboard ---
        if self.show_keyboard && keyboard_size > 0 {
            let ks_start = match self.orientation {
                MidiOrientation::Horizontal => 0i32,
                MidiOrientation::Vertical => 0i32,
            };
            draw_keyboard(
                dst_buf, stride, img_w, img_h,
                self.orientation,
                ks_start, keyboard_size,
                key_min, key_max,
                pixels_per_key, key_count,
            );
        }

        // --- Draw current position indicator ---
        let rel_time = effective_time - view_start;
        let indicator_pos = keyboard_size + (rel_time * pixels_per_second as f64) as i32;
        match self.orientation {
            MidiOrientation::Horizontal => {
                fill_rect(dst_buf, stride, img_w, img_h,
                    indicator_pos, 0, 2, img_h as i32,
                    1.0, 1.0, 1.0, 0.6);
            }
            MidiOrientation::Vertical => {
                fill_rect(dst_buf, stride, img_w, img_h,
                    0, indicator_pos, img_w as i32, 2,
                    1.0, 1.0, 1.0, 0.6);
            }
        }

        // --- Filter and collect visible notes (uses pre-computed note_seconds) ---
        let min_vel = self.minimum_velocity as u8;
        let time_axis_offset = keyboard_size;
        let n_seconds = &midi_data.note_seconds;

        VISIBLE_STASH.with(|stash| {
            let visible = &mut *stash.borrow_mut();
            visible.clear();
            for i in 0..midi_data.note_blocks.len() {
                let note = &midi_data.note_blocks[i];
                if note.velocity < min_vel { continue; }
                if self.track_filter_mode == MidiTrackFilterMode::SpecificTrack
                    && note.track != self.track_number as u32
                { continue; }
                if note.key < key_min as u8 || note.key > key_max as u8 { continue; }
                let (start_s, end_s) = n_seconds[i];
                if self.quantize_display {
                    let beat_duration = 60.0 / effective_bpm as f64;
                    let snapped_start = (start_s / beat_duration).round() * beat_duration;
                    let snapped_end = (end_s / beat_duration).round() * beat_duration;
                    if !(snapped_start < view_end && snapped_end > view_start) { continue; }
                } else {
                    if !(start_s < view_end && end_s > view_start) { continue; }
                }
                visible.push(i);
            }

            // Notes are already sorted by start_tick from parse_midi_file, so iterating
            // in order naturally gives back-to-front rendering (earliest on bottom).
            // No explicit sort needed.

            // --- Draw each visible note ---
            for &idx in visible.iter() {
                let note = &midi_data.note_blocks[idx];
                let (start_s, end_s) = if self.quantize_display {
                    let beat_duration = 60.0 / effective_bpm as f64;
                    let ss = (n_seconds[idx].0 / beat_duration).round() * beat_duration;
                    let se = (n_seconds[idx].1 / beat_duration).round() * beat_duration;
                    (ss, se)
                } else {
                    n_seconds[idx]
                };

            // Ensure minimum note duration for visibility
            let note_dur = (end_s - start_s).max(0.01);
            let note_width = (note_dur * pixels_per_second as f64).max(1.0) as i32;

            // Compute note color
            let (base_r, base_g, base_b) = match self.note_color_mode {
                MidiNoteColorMode::Solid => {
                    (self.note_color_r, self.note_color_g, self.note_color_b)
                }
                other => get_note_color(other, note),
            };

            // Apply velocity brightness
            let vel_factor = if self.velocity_affects_brightness {
                (note.velocity as f32 / 127.0).max(0.1)
            } else {
                1.0
            };
            let nr = (base_r * vel_factor).clamp(0.0, 1.0);
            let ng = (base_g * vel_factor).clamp(0.0, 1.0);
            let nb = (base_b * vel_factor).clamp(0.0, 1.0);

            // Compute note alpha
            let mut note_a = (self.note_color_a * self.note_opacity).clamp(0.0, 1.0);
            if self.velocity_affects_opacity {
                note_a *= note.velocity as f32 / 127.0;
            }

            // Note height based on velocity
            let note_height = if self.show_velocity_as_height {
                (pixels_per_key * note.velocity as f32 / 127.0).max(1.0)
            } else {
                pixels_per_key
            };

            // Position
            let key_idx = (note.key as i32 - key_min) as i32;
            let (nx, ny, nw, nh) = match self.orientation {
                MidiOrientation::Horizontal => {
                    let nx = time_axis_offset + ((start_s - view_start) * pixels_per_second as f64) as i32;
                    // pitch bottom-to-top
                    let ny = ((key_count - 1 - key_idx) as f32 * pixels_per_key
                        + (pixels_per_key - note_height) * 0.5) as i32;
                    (nx, ny, note_width, note_height.ceil() as i32)
                }
                MidiOrientation::Vertical => {
                    let ny = time_axis_offset + ((start_s - view_start) * pixels_per_second as f64) as i32;
                    let nx = (key_idx as f32 * pixels_per_key
                        + (pixels_per_key - note_height) * 0.5) as i32;
                    (nx, ny, note_height.ceil() as i32, note_width)
                }
            };

            // Border
            let border_a = (self.note_border_color_a * self.note_border_opacity).clamp(0.0, 1.0);
            let corner_r = self.note_corner_radius.min((nw as f32 / 2.0).floor()).min((nh as f32 / 2.0).floor());

            fill_rounded_rect_with_border(
                dst_buf, stride, img_w, img_h,
                nx, ny, nw, nh, corner_r, self.note_border_thickness,
                nr, ng, nb, note_a,
                self.note_border_color_r, self.note_border_color_g, self.note_border_color_b, border_a,
            );
        }
    });
    }
}
