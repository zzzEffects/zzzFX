use crate::settings::sprite_sheet::{
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
    /// `integrated_speed_offset` is the pre-computed integral of speed(t)/rate dt
    /// from 0 to `time`. When `None`, falls back to instantaneous speed.
    ///
    /// Returns `None` if the total cycle length is zero (no sprites to display).
    pub fn get_crop_rect(
        &self,
        time: f64,
        project_frame_rate: f64,
        sheet_w: u32,
        sheet_h: u32,
        integrated_speed_offset: Option<f64>,
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

        // Clamp repeat range within sprite range (both are absolute indices)
        let (sr_lo, sr_hi) = if self.sprite_range_start <= self.sprite_range_end {
            (self.sprite_range_start, self.sprite_range_end)
        } else {
            (self.sprite_range_end, self.sprite_range_start)
        };
        let rr_start = self.repeat_range_start.max(sr_lo).min(sr_hi);
        let rr_end = self.repeat_range_end.max(sr_lo).min(sr_hi);
        let m = (rr_end - rr_start).unsigned_abs() as i32 + 1;
        let repeat_offset_in_cycle = rr_start - self.sprite_range_start;

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

        // Time-based sprite index.
        // Use integrated speed offset if provided; otherwise fall back to
        // instantaneous speed × time (for non-OFX consumers or static frames).
        let rate = if project_frame_rate > 0.0 {
            project_frame_rate
        } else {
            1.0
        };
        let offset = integrated_speed_offset
            .unwrap_or(time * self.speed as f64 / rate);
        let frame_step =
            offset.floor() as i64 + (self.frame_offset as f64).floor() as i64;

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

        // Handle repeat range: extract repeat frames from the cycle index
        let j = if m > 0 {
            (i - repeat_offset_in_cycle) / m
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

        // Clamp crop rectangle within the sheet boundaries
        let x = (x.max(0) as u32).min(sheet_w.saturating_sub(sw as u32));
        let y = (y.max(0) as u32).min(sheet_h.saturating_sub(sh as u32));
        let sw = (sw as u32).min(sheet_w.saturating_sub(x));
        let sh = (sh as u32).min(sheet_h.saturating_sub(y));

        Some((x, y, sw, sh))
    }

    /// Compute the absolute frame index for a grid cell using only column/row,
    /// reading direction, and sprites cut. No range or loop-offset filtering.
    fn get_absolute_index(&self, grid_row: i32, grid_col: i32) -> i32 {
        use ReadingDirection::*;
        let dir = self.reading_direction;
        let vertical_read = matches!(dir, VForward | VBackward | VForwardS | VBackwardS);
        let backward_read = matches!(dir, HBackward | VBackward | HBackwardS | VBackwardS);
        let s_shaped = matches!(dir, HForwardS | HBackwardS | VForwardS | VBackwardS);

        let total_cols = self.sprite_columns.max(1);
        let total_rows = self.sprite_rows.max(1);
        let cut_x = self.sprites_cut_x.max(1);
        let cut_y = self.sprites_cut_y.max(1);

        // Per-block dimensions in physical grid coordinates
        let phys_cols_per_block = total_cols / cut_x;
        let phys_rows_per_block = total_rows / cut_y;

        // Reading-direction dimensions (swapped for vertical read)
        let (rd_cols, rd_rows_per_block) = if vertical_read {
            (phys_rows_per_block, phys_cols_per_block)
        } else {
            (phys_cols_per_block, phys_rows_per_block)
        };
        let cells_per_block = phys_cols_per_block * phys_rows_per_block;
        let cols_crop = if vertical_read { cut_y } else { cut_x }.max(1);

        let (mut r, mut c) = (grid_row, grid_col);
        if vertical_read { std::mem::swap(&mut r, &mut c); }

        let rr = r / rd_rows_per_block;
        let r_local = r % rd_rows_per_block;
        let mut cc = c / rd_cols;
        let mut c_local = c % rd_cols;

        if backward_read { cc = cols_crop - 1 - cc; }
        if s_shaped {
            let start_offset =
                (self.sprite_range_start / rd_cols % rd_rows_per_block).unsigned_abs() as i32;
            if ((r_local + start_offset) % 2) != 0 { c_local = rd_cols - 1 - c_local; }
        }
        if backward_read { c_local = rd_cols - 1 - c_local; }

        let block_idx = rr * cols_crop + cc;
        let within_idx = r_local * rd_cols + c_local;
        let abs_idx = block_idx * cells_per_block + within_idx;
        abs_idx
    }

    /// Extract the sprite at `crop_rect` from the decoded sheet and render it
    /// into `dst` (RGBA8, row-major). Fills with transparent black outside the
    /// sprite bounds. Processing order: pixel-based transforms (before scale),
    /// then scale, then non-pixel-based transforms (after scale).
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

        // --- GPU path (only when no pre-scale transforms needed) ---
        let can_use_gpu = self.rotation == 0.0
            && !self.displacement_pixel_based
            && !self.selection_mode;
        if can_use_gpu {
            if let Some(filter_mode) = match self.scale_algorithm {
                ScaleAlgorithm::Nearest => Some(0u32),
                ScaleAlgorithm::Triangle => Some(1u32),
                _ => None,
            } {
                // Convert normalized displacement to pixel offset for GPU
                let dx = (self.displacement_x - 0.5) * dst_w as f32;
                let dy = (0.5 - self.displacement_y) * dst_h as f32;
                let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    crate::gpu::sprite_sheet::try_sprite_sheet_gpu_render(
                        crop_rect, sheet_rgba, sheet_w, sheet_h,
                        self.scale, filter_mode,
                        dx, dy, false,
                        dst, dst_w as u32, dst_h as u32,
                    )
                }));
                match gpu_result {
                    Ok(Ok(true)) => return,
                    _ => {}
                }
            }
        }

        let out_w = (cw as f32 * self.scale).round() as usize;
        let out_h = (ch as f32 * self.scale).round() as usize;
        let _out_w = out_w.max(1);
        let _out_h = out_h.max(1);

        // Build crop buffer in original pixel coords
        let mut crop_buf = vec![0u8; (cw as usize) * (ch as usize) * 4];
        let sh_usize = sheet_h as usize;
        let sw_usize = sheet_w as usize;
        use rayon::prelude::*;
        crop_buf
            .par_chunks_mut(cw as usize * 4)
            .enumerate()
            .for_each(|(row, row_data)| {
                let src_y = cy as usize + row;
                if src_y >= sh_usize { return; }
                for col in 0..cw as usize {
                    let src_x = cx as usize + col;
                    if src_x < sw_usize {
                        let src_idx = (src_y * sw_usize + src_x) * 4;
                        row_data[col * 4..col * 4 + 4]
                            .copy_from_slice(&sheet_rgba[src_idx..src_idx + 4]);
                    }
                }
            });

        let (sw_orig, sh_orig) = (cw as usize, ch as usize);
        let mut sw = sw_orig;
        let mut sh = sh_orig;

        // --- Step 1: Pre-scale transforms (pixel-based, in original pixels) ---

        // Pixel-based displacement: convert normalized→output pixels, quantize to original pixel grid
        let mut pre_offset_x: i32 = 0;
        let mut pre_offset_y: i32 = 0;
        if self.displacement_pixel_based && (self.displacement_x - 0.5).abs() > 0.0001 || (0.5 - self.displacement_y).abs() > 0.0001 {
            let dx_out = (self.displacement_x - 0.5) * dst_w as f32;
            let dy_out = (0.5 - self.displacement_y) * dst_h as f32;
            let scale = self.scale.max(0.01);
            pre_offset_x = (dx_out / scale).round() as i32;
            pre_offset_y = (dy_out / scale).round() as i32;
        }

        // Pixel-based rotation: rotsprite on original pixels
        let pre_rotated = if self.rotation_pixel_based && self.rotation != 0.0 {
            let pixels: &[[u8; 4]] = bytemuck::cast_slice(&crop_buf);
            let empty: [u8; 4] = [0, 0, 0, 0];
            match rotsprite::rotsprite(pixels, &empty, sw_orig, self.rotation as f64) {
                Ok((new_w, new_h, rotated)) => {
                    sw = new_w;
                    sh = new_h;
                    Some(bytemuck::cast_vec::<[u8; 4], u8>(rotated))
                }
                Err(_) => None,
            }
        } else {
            None
        };

        // Pre-rotated buffer or original crop_buf
        let pre_buf = pre_rotated.as_ref().unwrap_or(&crop_buf);

        // --- Step 2: Scale ---
        let scaled = if self.scale != 1.0 {
            let (pw, ph) = if pre_rotated.is_some() { (sw as u32, sh as u32) } else { (cw, ch) };
            let src_img = image::RgbaImage::from_raw(pw, ph, pre_buf.clone())
                .unwrap_or_else(|| image::RgbaImage::new(pw, ph));
            let filter = match self.scale_algorithm {
                ScaleAlgorithm::Nearest => image::imageops::FilterType::Nearest,
                ScaleAlgorithm::Triangle => image::imageops::FilterType::Triangle,
                ScaleAlgorithm::CatmullRom => image::imageops::FilterType::CatmullRom,
                ScaleAlgorithm::Gaussian => image::imageops::FilterType::Gaussian,
                ScaleAlgorithm::Lanczos3 => image::imageops::FilterType::Lanczos3,
            };
            let out_w_s = (pw as f32 * self.scale).round() as u32;
            let out_h_s = (ph as f32 * self.scale).round() as u32;
            let resized = image::imageops::resize(&src_img, out_w_s.max(1), out_h_s.max(1), filter);
            sw = resized.width() as usize;
            sh = resized.height() as usize;
            resized.into_raw()
        } else {
            pre_buf.clone()
        };

        // --- Step 3: Post-scale transforms (non-pixel-based) ---
        let final_buf = if !self.rotation_pixel_based && self.rotation != 0.0 {
            let src_w = sw;
            let src_h = sh;
            let angle_rad = self.rotation as f64 * std::f64::consts::PI / 180.0;
            let cos_a = angle_rad.cos();
            let sin_a = angle_rad.sin();
            let new_w = (src_w as f64 * cos_a.abs() + src_h as f64 * sin_a.abs()).ceil() as u32;
            let new_h = (src_w as f64 * sin_a.abs() + src_h as f64 * cos_a.abs()).ceil() as u32;
            sw = new_w as usize;
            sh = new_h as usize;
            let cx_src = src_w as f64 / 2.0;
            let cy_src = src_h as f64 / 2.0;
            let cx_dst = new_w as f64 / 2.0;
            let cy_dst = new_h as f64 / 2.0;
            let src_img = &scaled;
            let mut rotated = vec![0u8; sw * sh * 4];
            for dy in 0..new_h as usize {
                for dx in 0..new_w as usize {
                    let rx = dx as f64 - cx_dst;
                    let ry = dy as f64 - cy_dst;
                    let src_x = rx * cos_a + ry * sin_a + cx_src;
                    let src_y = -rx * sin_a + ry * cos_a + cy_src;
                    let di = (dy * new_w as usize + dx) * 4;
                    if src_x >= 0.0 && src_x < (src_w as f64 - 1.0)
                        && src_y >= 0.0 && src_y < (src_h as f64 - 1.0)
                    {
                        let sx0 = src_x.floor() as usize;
                        let sy0 = src_y.floor() as usize;
                        let fx = (src_x - sx0 as f64) as f32;
                        let fy = (src_y - sy0 as f64) as f32;
                        let sx1 = (sx0 + 1).min(src_w - 1);
                        let sy1 = (sy0 + 1).min(src_h - 1);
                        let i00 = (sy0 * src_w + sx0) * 4;
                        let i10 = (sy0 * src_w + sx1) * 4;
                        let i01 = (sy1 * src_w + sx0) * 4;
                        let i11 = (sy1 * src_w + sx1) * 4;
                        for c in 0..4 {
                            let top = src_img[i00 + c] as f32 * (1.0 - fx) + src_img[i10 + c] as f32 * fx;
                            let bot = src_img[i01 + c] as f32 * (1.0 - fx) + src_img[i11 + c] as f32 * fx;
                            rotated[di + c] = (top * (1.0 - fy) + bot * fy).round() as u8;
                        }
                    }
                }
            }
            rotated
        } else {
            scaled
        };

        // --- Centering (signed) ---
        let mut offset_x = (dst_w as i32 - sw as i32) / 2;
        let mut offset_y = (dst_h as i32 - sh as i32) / 2;

        // --- Displacement: post-scale (non-pixel-based) or apply pre-scale offset ---
        if self.displacement_pixel_based {
            // Pre-scale offset was computed in original pixels; convert to scaled output offset
            let scale = self.scale.max(0.01);
            offset_x += (pre_offset_x as f32 * scale).round() as i32;
            offset_y += (pre_offset_y as f32 * scale).round() as i32;
        } else {
            // Post-scale: apply in output pixels directly (no quantization)
            let dx = ((self.displacement_x - 0.5) * dst_w as f32).round() as i32;
            let dy = ((0.5 - self.displacement_y) * dst_h as f32).round() as i32;
            offset_x += dx;
            offset_y += dy;
        }

        // --- Copy final buffer to dst with signed offset ---
        dst.par_chunks_mut(dst_w * 4).enumerate().for_each(|(row, dst_row)| {
            let src_row = row as i32 - offset_y;
            if src_row < 0 || src_row >= sh as i32 { return; }
            for col in 0..dst_w {
                let src_col = col as i32 - offset_x;
                if src_col < 0 || src_col >= sw as i32 { continue; }
                let src_idx = (src_row as usize * sw + src_col as usize) * 4;
                let dst_idx = col * 4;
                if src_idx + 4 <= final_buf.len() && dst_idx + 4 <= dst_row.len() {
                    dst_row[dst_idx..dst_idx + 4].copy_from_slice(&final_buf[src_idx..src_idx + 4]);
                }
            }
        });
    }

    /// Render the full sprite sheet in selection mode with grid overlay and
    /// frame numbers. Rotation is skipped per requirements.
    pub fn render_selection_mode(
        &self,
        sheet_rgba: &[u8],
        sheet_w: u32,
        sheet_h: u32,
        dst: &mut [u8],
        dst_w: usize,
        dst_h: usize,
    ) {
        dst.fill(0);
        if sheet_rgba.is_empty() || sheet_w == 0 || sheet_h == 0 { return; }

        let fit_scale = if self.fit_sprite_sheet_to_output {
            let sx = dst_w as f32 / sheet_w as f32;
            let sy = dst_h as f32 / sheet_h as f32;
            sx.min(sy)
        } else {
            self.scale
        }.max(0.01);

        let out_w = ((sheet_w as f32 * fit_scale).round() as usize).max(1);
        let out_h = ((sheet_h as f32 * fit_scale).round() as usize).max(1);

        // Scale the full sheet
        let scaled_sheet = if fit_scale != 1.0 {
            let src_img = image::RgbaImage::from_raw(sheet_w, sheet_h, sheet_rgba.to_vec())
                .unwrap_or_else(|| image::RgbaImage::new(sheet_w, sheet_h));
            image::imageops::resize(&src_img, out_w as u32, out_h as u32, image::imageops::FilterType::Nearest).into_raw()
        } else {
            sheet_rgba.to_vec()
        };

        // Centering offset (signed)
        let mut offset_x = (dst_w as i32 - out_w as i32) / 2;
        let mut offset_y = (dst_h as i32 - out_h as i32) / 2;

        // Apply displacement: convert normalized 0..1 → output pixels
        {
            let dx = ((self.displacement_x - 0.5) * dst_w as f32).round() as i32;
            let dy = ((0.5 - self.displacement_y) * dst_h as f32).round() as i32;
            if self.displacement_pixel_based {
                let ps = fit_scale.max(0.01);
                offset_x += ((dx as f32 / ps).round() * ps) as i32;
                offset_y += ((dy as f32 / ps).round() * ps) as i32;
            } else {
                offset_x += dx;
                offset_y += dy;
            }
        }

        // Copy scaled sheet to dst
        use rayon::prelude::*;
        dst.par_chunks_mut(dst_w * 4).enumerate().for_each(|(row, dst_row)| {
            let src_row = row as i32 - offset_y;
            if src_row < 0 || src_row >= out_h as i32 { return; }
            for col in 0..dst_w {
                let src_col = col as i32 - offset_x;
                if src_col < 0 || src_col >= out_w as i32 { continue; }
                let src_idx = (src_row as usize * out_w + src_col as usize) * 4;
                let dst_idx = col * 4;
                if src_idx + 4 <= scaled_sheet.len() && dst_idx + 4 <= dst_row.len() {
                    dst_row[dst_idx..dst_idx + 4].copy_from_slice(&scaled_sheet[src_idx..src_idx + 4]);
                }
            }
        });

        // --- Grid layout ---
        let columns = self.sprite_columns.max(1) as u32;
        let rows = self.sprite_rows.max(1) as u32;
        let cut_x = self.sprites_cut_x.max(1) as u32;
        let cut_y = self.sprites_cut_y.max(1) as u32;
        let sprite_w = (out_w as u32 / columns).max(1);
        let sprite_h = (out_h as u32 / rows).max(1);
        let full_cols = columns;
        let full_rows = rows;

        // Colors: regular = semitransparent white, cut = solid yellow
        let reg_color: [u8; 4] = [255, 255, 255, 220];
        let cut_color: [u8; 4] = [255, 255, 0, 255];
        let reg_thick: i32 = 2;
        let cut_thick: i32 = 4;

        // --- Cell highlighting (between sprite and grid/frame layers) ---
        let sr_lo = self.sprite_range_start.min(self.sprite_range_end);
        let sr_hi = self.sprite_range_start.max(self.sprite_range_end);
        let rr_start_c = self.repeat_range_start.max(sr_lo).min(sr_hi);
        let rr_end_c = self.repeat_range_end.max(sr_lo).min(sr_hi);
        let rr_lo = rr_start_c.min(rr_end_c);
        let rr_hi = rr_start_c.max(rr_end_c);

        for grid_row in 0..(full_rows as i32) {
            for grid_col in 0..(full_cols as i32) {
                let abs_idx = self.get_absolute_index(grid_row, grid_col);
                let in_sprite = abs_idx >= sr_lo && abs_idx <= sr_hi;
                let in_repeat = self.repeat_count > 0 && abs_idx >= rr_lo && abs_idx <= rr_hi;
                if !in_sprite && !in_repeat { continue; }

                let highlight: [u8; 4] = if in_repeat {
                    [0, 128, 255, 100] // semi-transparent blue
                } else {
                    [255, 0, 0, 100] // semi-transparent red
                };
                let cell_x = offset_x + (grid_col as u32 * sprite_w) as i32;
                let cell_y = offset_y + (grid_row as u32 * sprite_h) as i32;
                let spw = sprite_w as i32;
                let sph = sprite_h as i32;
                for py in 0..sph {
                    for px in 0..spw {
                        let x = cell_x + px;
                        let y = cell_y + py;
                        if x < 0 || x >= dst_w as i32 || y < 0 || y >= dst_h as i32 { continue; }
                        let idx = (y as usize * dst_w + x as usize) * 4;
                        if idx + 4 <= dst.len() {
                            let a = highlight[3] as f32 / 255.0;
                            let ia = 1.0 - a;
                            for c in 0..3 {
                                dst[idx + c] = (dst[idx + c] as f32 * ia + highlight[c] as f32 * a).round() as u8;
                            }
                        }
                    }
                }
            }
        }

        let draw_hline = |dst: &mut [u8], y: i32, x0: i32, x1: i32, color: [u8; 4], thick: i32| {
            for t in 0..thick {
                let py = y + t;
                if py < 0 || py >= dst_h as i32 { continue; }
                for px in x0..x1 {
                    if px < 0 || px >= dst_w as i32 { continue; }
                    let idx = (py as usize * dst_w + px as usize) * 4;
                    if idx + 4 <= dst.len() {
                        let a = color[3] as f32 / 255.0;
                        let ia = 1.0 - a;
                        for c in 0..3 { dst[idx + c] = (dst[idx + c] as f32 * ia + color[c] as f32 * a).round() as u8; }
                        dst[idx + 3] = dst[idx + 3].max(color[3]);
                    }
                }
            }
        };

        let draw_vline = |dst: &mut [u8], x: i32, y0: i32, y1: i32, color: [u8; 4], thick: i32| {
            for t in 0..thick {
                let px = x + t;
                if px < 0 || px >= dst_w as i32 { continue; }
                for py in y0..y1 {
                    if py < 0 || py >= dst_h as i32 { continue; }
                    let idx = (py as usize * dst_w + px as usize) * 4;
                    if idx + 4 <= dst.len() {
                        let a = color[3] as f32 / 255.0;
                        let ia = 1.0 - a;
                        for c in 0..3 { dst[idx + c] = (dst[idx + c] as f32 * ia + color[c] as f32 * a).round() as u8; }
                        dst[idx + 3] = dst[idx + 3].max(color[3]);
                    }
                }
            }
        };

        // Cut boundaries: every (columns/cut_x) for vertical, every (rows/cut_y) for horizontal
        let block_cols = columns / cut_x;
        let block_rows = rows / cut_y;

        // Draw vertical lines
        for cx in 0..=full_cols {
            let is_cut = cx == 0 || cx == full_cols || (cx % block_cols) == 0;
            let (color, thick) = if is_cut { (cut_color, cut_thick) } else { (reg_color, reg_thick) };
            let lx = offset_x + (cx * sprite_w) as i32;
            let y0 = offset_y.max(0);
            let y1 = (offset_y + out_h as i32).min(dst_h as i32);
            draw_vline(dst, lx, y0, y1, color, thick);
        }

        // Draw horizontal lines
        for ry in 0..=full_rows {
            let is_cut = ry == 0 || ry == full_rows || (ry % block_rows) == 0;
            let (color, thick) = if is_cut { (cut_color, cut_thick) } else { (reg_color, reg_thick) };
            let ly = offset_y + (ry * sprite_h) as i32;
            let x0 = offset_x.max(0);
            let x1 = (offset_x + out_w as i32).min(dst_w as i32);
            draw_hline(dst, ly, x0, x1, color, thick);
        }

        // --- Frame numbers ---
        // Compute font scale based on cell size
        let cell_min = sprite_w.min(sprite_h) as i32;
        let font_scale = (cell_min / 8).max(1).min(8); // scale 1-8x
        let char_base_w = 5;
        let char_base_h = 5;
        let scaled_char_w = char_base_w * font_scale;
        let scaled_char_h = char_base_h * font_scale;
        let char_spacing = font_scale.max(1);

        for grid_row in 0..full_rows as i32 {
            for grid_col in 0..full_cols as i32 {
                let frame_num = self.get_absolute_index(grid_row, grid_col);
                let num_str = frame_num.to_string();
                let text_w = num_str.len() as i32 * (scaled_char_w + char_spacing) - char_spacing;
                let text_h = scaled_char_h;
                // Center in cell
                let cell_cx = offset_x + (grid_col as u32 * sprite_w) as i32 + sprite_w as i32 / 2;
                let cell_cy = offset_y + (grid_row as u32 * sprite_h) as i32 + sprite_h as i32 / 2;
                let sx = cell_cx - text_w / 2;
                let sy = cell_cy - text_h / 2;

                draw_number_scaled(
                    dst, dst_w, dst_h, sx, sy, &num_str,
                    font_scale, offset_x, offset_y, out_w as i32, out_h as i32,
                );
            }
        }
    }
}

/// Simple 5x3 digit bitmaps (0-9)
const DIGITS: [[u8; 5]; 10] = [
    [0b01110, 0b10001, 0b10001, 0b10001, 0b01110], // 0
    [0b00100, 0b01100, 0b00100, 0b00100, 0b01110], // 1
    [0b01110, 0b10001, 0b00110, 0b01000, 0b11111], // 2
    [0b01110, 0b10001, 0b00110, 0b10001, 0b01110], // 3
    [0b00010, 0b00110, 0b01010, 0b11111, 0b00010], // 4
    [0b11111, 0b10000, 0b11110, 0b00001, 0b11110], // 5
    [0b01110, 0b10000, 0b11110, 0b10001, 0b01110], // 6
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000], // 7
    [0b01110, 0b10001, 0b01110, 0b10001, 0b01110], // 8
    [0b01110, 0b10001, 0b01111, 0b00001, 0b01110], // 9
];

/// Draw a scaled number string centered at the given position.
fn draw_number_scaled(
    dst: &mut [u8], dst_w: usize, dst_h: usize,
    x: i32, y: i32, num_str: &str, scale: i32,
    offset_x: i32, offset_y: i32, out_w: i32, out_h: i32,
) {
    let char_base_w: i32 = 5;
    let char_spacing = scale.max(1);
    let text_color: [u8; 4] = [255, 255, 255, 255];
    let shadow_color: [u8; 4] = [0, 0, 0, 200];

    for (ci, ch) in num_str.chars().enumerate() {
        let digit = match ch.to_digit(10) { Some(d) => d as usize, None => continue };
        let cx = x + ci as i32 * (char_base_w * scale + char_spacing);

        for row in 0..5 {
            let bits = DIGITS[digit][row];
            for col in 0..5 {
                if bits & (1 << (4 - col)) == 0 { continue; }
                // Scale each dot to scale×scale
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = cx + col as i32 * scale + sx;
                        let py = y + row as i32 * scale + sy;
                        if px < 0 || px >= dst_w as i32 || py < 0 || py >= dst_h as i32 { continue; }
                        // Clamp to sprite sheet area
                        let src_x = px - offset_x;
                        let src_y = py - offset_y;
                        if src_x < 0 || src_x >= out_w || src_y < 0 || src_y >= out_h { continue; }
                        let idx = (py as usize * dst_w + px as usize) * 4;
                        if idx + 4 <= dst.len() {
                            // Draw shadow outline (1px black) then white fill
                            let is_edge = sx == 0 || sy == 0 || sx == scale - 1 || sy == scale - 1;
                            if is_edge && scale >= 2 {
                                let a = shadow_color[3] as f32 / 255.0;
                                let ia = 1.0 - a;
                                for c in 0..3 { dst[idx + c] = (dst[idx + c] as f32 * ia + shadow_color[c] as f32 * a).round() as u8; }
                                dst[idx + 3] = dst[idx + 3].max(shadow_color[3]);
                            } else {
                                dst[idx..idx + 4].copy_from_slice(&text_color);
                            }
                        }
                    }
                }
            }
        }
    }
}
