use crate::settings::zzz_sprite_sheet::{
    PlaybackMode, ReadingDirection, ScaleAlgorithm, ZzzSpriteSheet,
};

/// Positive modulo: always returns a non-negative result.
fn positive_mod(a: i64, b: i64) -> i64 {
    if b <= 0 {
        return 0;
    }
    if a >= 0 {
        a % b
    } else {
        (b - (a.abs() % b)) % b
    }
}

impl ZzzSpriteSheet {
    /// Compute the crop rectangle (x, y, w, h) within the sprite sheet for the
    /// sprite that should be displayed at `time` (in frames, typically floor(time)).
    ///
    /// Returns `None` if the total cycle length is zero (no sprites to display).
    pub fn get_crop_rect(
        &self,
        time: f64,
        project_frame_rate: f64,
        sheet_w: u32,
        sheet_h: u32,
    ) -> Option<(u32, u32, u32, u32)> {
        use ReadingDirection::*;

        let dir = self.reading_direction;
        let vertical_read = matches!(dir, VForward | VBackward | VForwardS | VBackwardS);
        let backward_read =
            matches!(dir, HBackward | VBackward | HBackwardS | VBackwardS);
        let s_shaped =
            matches!(dir, HForwardS | HBackwardS | VForwardS | VBackwardS);

        // Number of sprites in the range
        let n = (self.sprite_range_end - self.sprite_range_start).unsigned_abs() as i32 + 1;
        // Number of sprites in the repeat sub-range
        let m =
            (self.repeat_range_end - self.repeat_range_start).unsigned_abs() as i32 + 1;

        // Adjust n for NormalReverseMerge (last frame = first frame merged)
        let n_adj = if self.playback_mode == PlaybackMode::NormalReverseMerge {
            n - 1
        } else {
            n
        };
        let total = n_adj + m * self.repeat_count;
        if total <= 0 {
            return None;
        }

        // Time-based sprite index — independent of project frame rate.
        // speed ÷ project_frame_rate normalises playback across hosts:
        // speed=30 with 30fps → 1 sprite per second (same as speed=60 at 60fps).
        // speed=0 → paused (stays on current sprite).
        let rate = if project_frame_rate > 0.0 {
            project_frame_rate
        } else {
            1.0
        };
        let frame_step =
            (time * self.speed as f64 / rate).floor() as i64 + self.frame_offset as i64;

        // Raw sprite index within the cycle
        let mut i = positive_mod(frame_step, total as i64) as i32;

        // Handle ping-pong reversal for NormalReverse modes
        if matches!(
            self.playback_mode,
            PlaybackMode::NormalReverse | PlaybackMode::NormalReverseMerge
        ) {
            let cycle = ((frame_step / total as i64) as i32).unsigned_abs();
            if cycle % 2 == 1 {
                i = n_adj + m * self.repeat_count
                    - if self.playback_mode != PlaybackMode::NormalReverseMerge {
                        1
                    } else {
                        0
                    }
                    - i;
            }
        }

        // Apply loop offset
        let loop_total = if self.playback_mode == PlaybackMode::NormalReverseMerge {
            n
        } else {
            n_adj
        } + m * self.repeat_count;
        if loop_total > 0 {
            i = positive_mod(
                i as i64 + self.loop_offset as i64,
                loop_total as i64,
            ) as i32;
        }

        // Handle repeat range: extract repeat frames from the index
        let min_repeat_sprite =
            std::cmp::min(self.repeat_range_start, self.sprite_range_start);
        let j = if m > 0 {
            (i - min_repeat_sprite) / m
        } else {
            0
        };
        let j = if j < 0 { 0 } else { j };
        i = i - std::cmp::min(j, self.repeat_count) * m;

        // Map to absolute sprite index in the sheet
        let abs_idx = if self.sprite_range_start <= self.sprite_range_end {
            self.sprite_range_start + i
        } else {
            self.sprite_range_start - i
        };

        // Grid layout computation
        // Sprite size is derived from sheet dimensions divided by columns/rows.
        let columns = self.sprite_columns.max(1) as u32;
        let rows = self.sprite_rows.max(1) as u32;
        let sw = (sheet_w / columns).max(1) as i32;
        let sh = (sheet_h / rows).max(1) as i32;
        let cut_x = self.sprites_cut_x.max(1);
        let cut_y = self.sprites_cut_y.max(1);

        let cols = if vertical_read {
            (sheet_h as i32 / sh / cut_y).max(1)
        } else {
            (sheet_w as i32 / sw / cut_x).max(1)
        };

        let sum = (sheet_h as i32 / sh / cut_y) * (sheet_w as i32 / sw / cut_x);
        let sum = sum.max(1);

        let rows_per_block = (sum / cols).max(1);

        let mut r = abs_idx / cols % rows_per_block;
        let mut c = abs_idx % cols;

        if backward_read {
            c = cols - 1 - c;
        }

        if s_shaped {
            let start_offset =
                (self.sprite_range_start / cols % rows_per_block).unsigned_abs() as i32;
            if ((r + start_offset) % 2) != 0 {
                c = cols - 1 - c;
            }
        }

        // SpritesCut block handling
        let block_idx = abs_idx / sum;
        let cols_crop = if vertical_read { cut_y } else { cut_x }.max(1);
        let rr = block_idx / cols_crop;
        let mut cc = block_idx % cols_crop;
        if backward_read {
            cc = cols_crop - 1 - cc;
        }
        r += rr * rows_per_block;
        c += cc * cols;

        // Swap row/column for vertical reading
        if vertical_read {
            std::mem::swap(&mut r, &mut c);
        }

        let x = c * sw;
        let y = r * sh;

        // Clamp crop rectangle within the sheet boundaries so an out-of-range
        // sprite index never reads past the decoded image buffer.
        let x = (x.max(0) as u32).min(sheet_w.saturating_sub(sw as u32));
        let y = (y.max(0) as u32).min(sheet_h.saturating_sub(sh as u32));
        let sw = (sw as u32).min(sheet_w.saturating_sub(x));
        let sh = (sh as u32).min(sheet_h.saturating_sub(y));

        Some((x, y, sw, sh))
    }

    /// Extract the sprite at `crop_rect` from the decoded sheet and render it
    /// into `dst` (RGBA8, row-major). Fills with transparent black where the
    /// crop rect falls outside the sheet boundaries. Applies scaling if
    /// `self.scale != 1.0`.
    pub fn render_sprite(
        &self,
        crop_rect: (u32, u32, u32, u32),
        sheet_rgba: &[u8],
        sheet_w: u32,
        sheet_h: u32,
        dst: &mut [u8],
        dst_w: usize,
        dst_h: usize,
    ) {
        let (cx, cy, cw, ch) = crop_rect;

        // First fill dst with transparent black
        dst.fill(0);

        if cw == 0 || ch == 0 || sheet_rgba.is_empty() {
            return;
        }

        // --- GPU path (Nearest / bilinear) ---
        if let Some(filter_mode) = match self.scale_algorithm {
            ScaleAlgorithm::Nearest => Some(0u32),
            ScaleAlgorithm::Triangle => Some(1u32), // bilinear on GPU
            _ => None,
        } {
            let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::gpu::sprite_sheet::try_sprite_sheet_gpu_render(
                    crop_rect,
                    sheet_rgba,
                    sheet_w,
                    sheet_h,
                    self.scale,
                    filter_mode,
                    dst,
                    dst_w as u32,
                    dst_h as u32,
                )
            }));
            match gpu_result {
                Ok(Ok(true)) => return,
                _ => {} // fall through to CPU
            }
        }

        let out_w = (cw as f32 * self.scale).round() as usize;
        let out_h = (ch as f32 * self.scale).round() as usize;
        let out_w = out_w.max(1);
        let out_h = out_h.max(1);

        // Build an intermediate RGBA8 buffer for the unscaled sprite crop
        let mut crop_buf = vec![0u8; (cw as usize) * (ch as usize) * 4];

        for row in 0..ch as usize {
            let src_y = cy as usize + row;
            for col in 0..cw as usize {
                let src_x = cx as usize + col;
                let dst_idx = (row * cw as usize + col) * 4;
                if src_x < sheet_w as usize && src_y < sheet_h as usize {
                    let src_idx = (src_y * sheet_w as usize + src_x) * 4;
                    crop_buf[dst_idx..dst_idx + 4]
                        .copy_from_slice(&sheet_rgba[src_idx..src_idx + 4]);
                }
            }
        }

        // Scale if needed, using the image crate
        let scaled = if self.scale != 1.0 {
            let src_img = image::RgbaImage::from_raw(cw, ch, crop_buf)
                .unwrap_or_else(|| image::RgbaImage::new(cw, ch));
            let filter = match self.scale_algorithm {
                ScaleAlgorithm::Nearest => image::imageops::FilterType::Nearest,
                ScaleAlgorithm::Triangle => image::imageops::FilterType::Triangle,
                ScaleAlgorithm::CatmullRom => image::imageops::FilterType::CatmullRom,
                ScaleAlgorithm::Gaussian => image::imageops::FilterType::Gaussian,
                ScaleAlgorithm::Lanczos3 => image::imageops::FilterType::Lanczos3,
            };
            let resized =
                image::imageops::resize(&src_img, out_w as u32, out_h as u32, filter);
            resized.into_raw()
        } else {
            crop_buf
        };

        // Center the scaled sprite in the output buffer
        let (sw, sh) = if self.scale != 1.0 {
            (out_w, out_h)
        } else {
            (cw as usize, ch as usize)
        };

        let offset_x = if dst_w >= sw { (dst_w - sw) / 2 } else { 0 };
        let offset_y = if dst_h >= sh { (dst_h - sh) / 2 } else { 0 };

        for row in 0..sh {
            let dst_y = offset_y + row;
            if dst_y >= dst_h {
                break;
            }
            for col in 0..sw {
                let dst_x = offset_x + col;
                if dst_x >= dst_w {
                    break;
                }
                let src_idx = (row * sw + col) * 4;
                let dst_idx = (dst_y * dst_w + dst_x) * 4;
                dst[dst_idx..dst_idx + 4]
                    .copy_from_slice(&scaled[src_idx..src_idx + 4]);
            }
        }
    }
}
