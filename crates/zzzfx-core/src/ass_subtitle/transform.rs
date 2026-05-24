//! `\t` transform interpolation.

use super::types::*;

/// Apply `\t` animated transforms active at the current time.
pub(crate) fn apply_transforms(
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
        let t2 = if tform.end_t == 0 {
            ev_end - ev_start
        } else {
            tform.end_t
        };
        if elapsed < t1 || elapsed > t2 {
            continue;
        }
        let dur = (t2 - t1).max(1);
        let raw_t = (elapsed - t1) as f32 / dur as f32;
        // Acceleration curve: t = t^accel (accel > 1 = slow start, < 1 = fast start)
        let t = raw_t.powf(tform.acceleration);
        let target = &tform.tags;

        // Interpolate float fields
        interpolate_f32(&mut result.fontsize, target.fontsize, base.fontsize, t);
        interpolate_color(
            &mut result.primary_color,
            target.primary_color,
            base.primary_color,
            t,
        );
        interpolate_color(
            &mut result.secondary_color,
            target.secondary_color,
            base.secondary_color,
            t,
        );
        interpolate_color(
            &mut result.outline_color,
            target.outline_color,
            base.outline_color,
            t,
        );
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
                    pts[i].0 =
                        base_clip.points[i].0 + (tgt_clip.points[i].0 - base_clip.points[i].0) * t;
                    pts[i].1 =
                        base_clip.points[i].1 + (tgt_clip.points[i].1 - base_clip.points[i].1) * t;
                }
                result.clip = Some(ClipData {
                    points: pts,
                    ..base_clip.clone()
                });
            } else {
                result.clip = Some(tgt_clip.clone());
            }
        }

        // Pos: interpolate
        if let Some(tgt_pos) = target.pos {
            result.pos = Some(if let Some(base_pos) = base.pos {
                (
                    base_pos.0 + (tgt_pos.0 - base_pos.0) * t,
                    base_pos.1 + (tgt_pos.1 - base_pos.1) * t,
                )
            } else {
                tgt_pos
            });
        }

        // Bool/binary fields: switch at time midpoint (raw_t), unaffected by acceleration
        if raw_t > 0.5 {
            if target.bold.is_some() {
                result.bold = target.bold;
            }
            if target.italic.is_some() {
                result.italic = target.italic;
            }
            if target.underline.is_some() {
                result.underline = target.underline;
            }
            if target.strikeout.is_some() {
                result.strikeout = target.strikeout;
            }
            if target.fontname.is_some() {
                result.fontname = target.fontname.clone();
            }
            if target.alignment.is_some() {
                result.alignment = target.alignment;
            }
        }
    }

    result
}

fn interpolate_f32(target: &mut Option<f32>, tgt_val: Option<f32>, base_val: Option<f32>, t: f32) {
    if let Some(tv) = tgt_val {
        *target = Some(if let Some(bv) = base_val {
            bv + (tv - bv) * t
        } else {
            tv
        });
    }
}

fn interpolate_color(
    target: &mut Option<[f32; 4]>,
    tgt_val: Option<[f32; 4]>,
    base_val: Option<[f32; 4]>,
    t: f32,
) {
    if let Some(tv) = tgt_val {
        *target = Some(if let Some(bv) = base_val {
            [
                bv[0] + (tv[0] - bv[0]) * t,
                bv[1] + (tv[1] - bv[1]) * t,
                bv[2] + (tv[2] - bv[2]) * t,
                bv[3] + (tv[3] - bv[3]) * t,
            ]
        } else {
            tv
        });
    }
}
