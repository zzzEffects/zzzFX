use rayon::prelude::*;

use crate::settings::cast_shadow::ZzzCastShadow;

const RCP_255: f32 = 1.0 / 255.0;

impl ZzzCastShadow {
    pub fn is_identity(&self) -> bool {
        let shadow_a = self.shadow_color_a.clamp(0.0, 1.0);
        let scale = self.scale.clamp(0.1, 3.0);
        shadow_a <= 0.0 || scale < 0.001
    }

    pub fn apply_effect(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        let len = width * height * 4;
        if src.len() < len || dst.len() < len || width == 0 || height == 0 {
            return;
        }
        if self.is_identity() {
            dst[..len].copy_from_slice(&src[..len]);
            return;
        }

        #[cfg(feature = "gpu")]
        {
            let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::gpu::cast_shadow::try_cast_shadow_gpu_render(
                    self, src, dst, width, height,
                )
            }));
            match gpu_result {
                Ok(Ok(true)) => return,
                _ => {}
            }
        }

        self.render_cpu(src, dst, width, height);
    }

    fn render_cpu(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        let total = width * height;

        let scale = self.scale.clamp(0.1, 3.0);
        let softness = self.softness.clamp(0.0, 1.0);
        let shear_angle = self.shear_angle.clamp(0.0, 360.0);
        let shear_amount = self.shear_amount.clamp(0.0, 1.0);
        let threshold = self.alpha_threshold.clamp(0.0, 1.0);
        let source_opacity = self.source_opacity.clamp(0.0, 1.0);
        let pivot_angle = self.pivot_angle.clamp(0.0, 360.0);
        let pivot_mode = self.pivot_mode;
        let fade = self.fade.clamp(0.0, 1.0);

        let sr = self.shadow_color_r.clamp(0.0, 1.0);
        let sg = self.shadow_color_g.clamp(0.0, 1.0);
        let sb = self.shadow_color_b.clamp(0.0, 1.0);
        let sa = self.shadow_color_a.clamp(0.0, 1.0);

        let wf = width as f32;
        let hf = height as f32;

        // --- Build alpha mask ---
        let mut alpha = vec![0.0f32; total];
        let bbox = alpha
            .par_iter_mut()
            .enumerate()
            .map(|(i, a)| {
                let src_a = src[i * 4 + 3] as f32 * RCP_255;
                let v = if src_a >= threshold { src_a } else { 0.0 };
                *a = v;
                if v > 0.0 {
                    let x = (i % width) as u32;
                    let y = (i / width) as u32;
                    Some((x, x, y, y))
                } else {
                    None
                }
            })
            .reduce(
                || None,
                |acc, b| match (acc, b) {
                    (None, b) => b,
                    (a, None) => a,
                    (Some((x1a, x2a, y1a, y2a)), Some((x1b, x2b, y1b, y2b))) => Some((
                        x1a.min(x1b),
                        x2a.max(x2b),
                        y1a.min(y1b),
                        y2a.max(y2b),
                    )),
                },
            );

        // --- Displacement from Shadow Offset ---
        let total_dx = (self.offset_x.clamp(0.0, 1.0) - 0.5) * wf;
        let total_dy = (self.offset_y.clamp(0.0, 1.0) - 0.5) * hf;

        // --- Compute axes based on pivot_mode ---

        let axes: Vec<AxisData>;
        let component_labels: Vec<u32>; // 0=none, 1..N=component index (AutoMulti only)

        match pivot_mode {
            crate::settings::cast_shadow::PivotMode::AutoSingle => {
                axes = compute_axis(bbox, pivot_angle, wf, hf)
                    .into_iter()
                    .collect();
                component_labels = vec![0; total];
            }
            crate::settings::cast_shadow::PivotMode::AutoMulti => {
                let (components, labels) = find_components(&alpha, width, height);
                component_labels = labels;
                if components.is_empty() {
                    axes = compute_axis(bbox, pivot_angle, wf, hf)
                        .into_iter()
                        .collect();
                } else {
                    axes = components
                        .into_iter()
                        .filter_map(|comp| {
                            compute_axis(Some(comp), pivot_angle, wf, hf)
                        })
                        .collect();
                }
            }
            crate::settings::cast_shadow::PivotMode::ManualSingle => {
                let mo_x = (self.manual_center_x.clamp(0.0, 1.0) - 0.5) * wf;
                let mo_y = (self.manual_center_y.clamp(0.0, 1.0) - 0.5) * hf;
                axes = compute_axis_manual(pivot_angle, wf, hf, mo_x, mo_y)
                    .into_iter()
                    .collect();
                component_labels = vec![0; total];
            }
        };

        // --- Step 2: Inverse-transform project for each axis ---
        let mut shadow = vec![0.0f32; total];
        let inv_scale = 1.0 / scale;
        let has_labels = pivot_mode == crate::settings::cast_shadow::PivotMode::AutoMulti
            && axes.len() > 1;

        for (axis_idx, axis) in axes.iter().enumerate() {
            let comp_idx = (axis_idx + 1) as u32;
            shadow
                .par_iter_mut()
                .enumerate()
                .for_each(|(i, out)| {
                    let ox = (i % width) as f32;
                    let oy = (i / width) as f32;

                    let px = ox - total_dx + 0.5;
                    let py = oy - total_dy + 0.5;

                    // Approximate edge_dist for shear computation
                    let rx0 = px - axis.contact_x;
                    let ry0 = py - axis.contact_y;
                    let perp_approx = (rx0 * axis.nx + ry0 * axis.ny) * inv_scale;
                    let edge_dist = perp_approx;
                    if edge_dist <= 0.0 {
                        return;
                    }

                    // 2D shear in screen space, proportional to edge_dist
                    let dist_ratio = edge_dist / axis.bbox_perp.max(1.0);
                    let shear_rad = shear_angle.to_radians();
                    let shear_dim = f32::min(wf, hf) * 0.5;
                    let sx_s = shear_amount * dist_ratio * shear_dim * shear_rad.cos();
                    let sy_s = shear_amount * dist_ratio * shear_dim * shear_rad.sin();

                    // Remove shear from output position
                    let px_adj = px - sx_s;
                    let py_adj = py - sy_s;

                    // Standard inverse scale from pivot
                    let rx = px_adj - axis.contact_x;
                    let ry = py_adj - axis.contact_y;
                    let perp_out = rx * axis.nx + ry * axis.ny;
                    let along_out = rx * axis.ax + ry * axis.ay;
                    let perp_src = perp_out * inv_scale;
                    let along_src = along_out * inv_scale;

                    let sx = axis.contact_x + along_src * axis.ax + perp_src * axis.nx;
                    let sy = axis.contact_y + along_src * axis.ay + perp_src * axis.ny;

                    // For AutoMulti: only project from this component's own alpha pixels
                    if has_labels {
                        let sx_i = sx.round() as isize;
                        let sy_i = sy.round() as isize;
                        if sx_i < 0 || sx_i >= width as isize || sy_i < 0 || sy_i >= height as isize {
                            return;
                        }
                        let src_idx = (sy_i as usize) * width + (sx_i as usize);
                        if src_idx < total && component_labels[src_idx] != comp_idx {
                            return; // not from this component
                        }
                    }

                    let a = bilinear_alpha(&alpha, width, height, sx, sy);
                    if a <= 0.0 {
                        return;
                    }

                    let inv_bp = if axis.bbox_perp > 0.0 {
                        1.0 / axis.bbox_perp
                    } else {
                        0.0
                    };
                    let fade_factor = (1.0 - fade * edge_dist * inv_bp).max(0.0);
                    let fa = a * fade_factor;
                    if fa > *out {
                        *out = fa;
                    }
                });
        }

        // --- Step 3: Separable box blur ---
        let blur_radius = (softness * f32::min(wf, hf) * 0.08).ceil() as usize;
        if blur_radius > 0 {
            let mut tmp = vec![0.0f32; total];
            tmp.par_iter_mut().enumerate().for_each(|(i, out)| {
                let y = i / width;
                let x0 = i % width;
                let x1 = (x0 + blur_radius + 1).min(width);
                let x0c = if x0 >= blur_radius {
                    x0 - blur_radius
                } else {
                    0
                };
                let row_start = y * width;
                let mut sum = 0.0f64;
                for k in x0c..x1 {
                    sum += shadow[row_start + k] as f64;
                }
                *out = (sum / (x1 - x0c) as f64) as f32;
            });
            shadow.par_iter_mut().enumerate().for_each(|(i, out)| {
                let x = i % width;
                let y = i / width;
                let y0 = if y >= blur_radius {
                    y - blur_radius
                } else {
                    0
                };
                let y1 = (y + blur_radius + 1).min(height);
                let mut sum = 0.0f64;
                for k in y0..y1 {
                    sum += tmp[k * width + x] as f64;
                }
                *out = (sum / (y1 - y0) as f64) as f32;
            });
        }

        // --- Step 4: Color + composite ---
        dst.par_chunks_mut(width * 4)
            .enumerate()
            .for_each(|(y, row)| {
                for x in 0..width {
                    let i = y * width + x;
                    let o = x * 4;

                    let sh_alpha = shadow[i] * sa;

                    let src_r = src[i * 4] as f32 * RCP_255;
                    let src_g = src[i * 4 + 1] as f32 * RCP_255;
                    let src_b = src[i * 4 + 2] as f32 * RCP_255;
                    let src_a = src[i * 4 + 3] as f32 * RCP_255 * source_opacity;

                    let inv = 1.0 - src_a;
                    row[o] = ((src_r * src_a + sr * sh_alpha * inv).clamp(0.0, 1.0) * 255.0)
                        .round() as u8;
                    row[o + 1] = ((src_g * src_a + sg * sh_alpha * inv).clamp(0.0, 1.0) * 255.0)
                        .round() as u8;
                    row[o + 2] = ((src_b * src_a + sb * sh_alpha * inv).clamp(0.0, 1.0) * 255.0)
                        .round() as u8;
                    row[o + 3] = ((src_a + sh_alpha * inv).clamp(0.0, 1.0) * 255.0)
                        .round() as u8;
                }
            });
    }
}

// ---------------------------------------------------------------------------
// Axis computation (shared logic for single axis)
// ---------------------------------------------------------------------------

struct AxisData {
    contact_x: f32,
    contact_y: f32,
    nx: f32,
    ny: f32,
    ax: f32,
    ay: f32,
    bbox_perp: f32,
}

fn compute_axis(
    bbox: Option<(u32, u32, u32, u32)>,
    pivot_angle: f32,
    wf: f32,
    hf: f32,
) -> Option<AxisData> {
    match bbox {
        Some((min_x, max_x, min_y, max_y)) => {
            let min_xf = min_x as f32;
            let max_xf = max_x as f32;
            let min_yf = min_y as f32;
            let max_yf = max_y as f32;
            let bcx = (min_xf + max_xf) * 0.5;
            let bcy = (min_yf + max_yf) * 0.5;
            let hw = (max_xf - min_xf) * 0.5;
            let hh = (max_yf - min_yf) * 0.5;
            let bbox_w = max_xf - min_xf;
            let bbox_h = max_yf - min_yf;

            let rad = pivot_angle.to_radians();
            let dx = rad.sin();
            let dy = -rad.cos();
            let tx = if dx.abs() < 1e-6 {
                f32::MAX
            } else if dx > 0.0 {
                hw / dx
            } else {
                -hw / dx
            };
            let ty = if dy.abs() < 1e-6 {
                f32::MAX
            } else if dy > 0.0 {
                hh / dy
            } else {
                -hh / dy
            };
            let t = tx.min(ty);
            let cx = bcx + t * dx;
            let cy = bcy + t * dy;
            let enx = -dx;
            let eny = -dy;
            let eax = eny;
            let eay = -enx;
            let bp = bbox_w * enx.abs() + bbox_h * eny.abs();
            Some(AxisData {
                contact_x: cx,
                contact_y: cy,
                nx: enx,
                ny: eny,
                ax: eax,
                ay: eay,
                bbox_perp: bp,
            })
        }
        None => axis_at_center(pivot_angle, wf, hf, 0.0, 0.0),
    }
}

fn axis_at_center(
    pivot_angle: f32,
    wf: f32,
    hf: f32,
    offset_x: f32,
    offset_y: f32,
) -> Option<AxisData> {
    let rad = pivot_angle.to_radians();
    let dx = rad.sin();
    let dy = -rad.cos();
    let enx = -dx;
    let eny = -dy;
    let eax = eny;
    let eay = -enx;
    let bp = wf * enx.abs() + hf * eny.abs();
    Some(AxisData {
        contact_x: wf * 0.5 + offset_x,
        contact_y: hf * 0.5 + offset_y,
        nx: enx,
        ny: eny,
        ax: eax,
        ay: eay,
        bbox_perp: bp,
    })
}

fn compute_axis_manual(
    pivot_angle: f32,
    wf: f32,
    hf: f32,
    offset_x: f32,
    offset_y: f32,
) -> Option<AxisData> {
    axis_at_center(pivot_angle, wf, hf, offset_x, offset_y)
}

// ---------------------------------------------------------------------------
// Connected-component detection for AutoMulti mode
// ---------------------------------------------------------------------------

fn find_components(
    alpha: &[f32],
    width: usize,
    height: usize,
) -> (Vec<(u32, u32, u32, u32)>, Vec<u32>) {
    let total = width * height;
    let mut labels = vec![0u32; total];
    let mut components = Vec::new();
    let mut queue = Vec::with_capacity(1024);

    let mut comp_idx = 1u32;
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if labels[idx] != 0 || alpha[idx] <= 0.0 {
                continue;
            }

            // BFS flood-fill from this unvisited alpha pixel
            labels[idx] = comp_idx;
            queue.clear();
            queue.push((x, y));
            let mut head = 0usize;
            let mut min_x = x as u32;
            let mut max_x = x as u32;
            let mut min_y = y as u32;
            let mut max_y = y as u32;

            while head < queue.len() {
                let (cx, cy) = queue[head];
                head += 1;

                if cx > 0 {
                    let nidx = cy * width + (cx - 1);
                    if labels[nidx] == 0 && alpha[nidx] > 0.0 {
                        labels[nidx] = comp_idx;
                        queue.push((cx - 1, cy));
                        min_x = min_x.min((cx - 1) as u32);
                    }
                }
                if cx + 1 < width {
                    let nidx = cy * width + (cx + 1);
                    if labels[nidx] == 0 && alpha[nidx] > 0.0 {
                        labels[nidx] = comp_idx;
                        queue.push((cx + 1, cy));
                        max_x = max_x.max((cx + 1) as u32);
                    }
                }
                if cy > 0 {
                    let nidx = (cy - 1) * width + cx;
                    if labels[nidx] == 0 && alpha[nidx] > 0.0 {
                        labels[nidx] = comp_idx;
                        queue.push((cx, cy - 1));
                        min_y = min_y.min((cy - 1) as u32);
                    }
                }
                if cy + 1 < height {
                    let nidx = (cy + 1) * width + cx;
                    if labels[nidx] == 0 && alpha[nidx] > 0.0 {
                        labels[nidx] = comp_idx;
                        queue.push((cx, cy + 1));
                        max_y = max_y.max((cy + 1) as u32);
                    }
                }
            }

            components.push((min_x, max_x, min_y, max_y));
            comp_idx += 1;
        }
    }

    (components, labels)
}

// ---------------------------------------------------------------------------
// Bilinear sample from the pre-built float alpha mask
// ---------------------------------------------------------------------------

#[inline]
fn bilinear_alpha(alpha: &[f32], width: usize, height: usize, x: f32, y: f32) -> f32 {
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;
    let ix0 = x0 as isize;
    let iy0 = y0 as isize;
    let ix1 = ix0 + 1;
    let iy1 = iy0 + 1;
    let w = width as isize;
    let h = height as isize;

    let v00 = if ix0 >= 0 && ix0 < w && iy0 >= 0 && iy0 < h {
        alpha[(iy0 as usize) * width + (ix0 as usize)]
    } else {
        0.0
    };
    let v10 = if ix1 >= 0 && ix1 < w && iy0 >= 0 && iy0 < h {
        alpha[(iy0 as usize) * width + (ix1 as usize)]
    } else {
        0.0
    };
    let v01 = if ix0 >= 0 && ix0 < w && iy1 >= 0 && iy1 < h {
        alpha[(iy1 as usize) * width + (ix0 as usize)]
    } else {
        0.0
    };
    let v11 = if ix1 >= 0 && ix1 < w && iy1 >= 0 && iy1 < h {
        alpha[(iy1 as usize) * width + (ix1 as usize)]
    } else {
        0.0
    };

    let top = v00 + (v10 - v00) * fx;
    let bot = v01 + (v11 - v01) * fx;
    top + (bot - top) * fy
}
