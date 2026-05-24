//! Outline offset generation with bounded caching.
//! Offsets are placed on proper concentric circles with uniform angular spacing.

use std::f32::consts::TAU;
use std::sync::Arc;

/// Number of directions per ring for the base outline.
pub(crate) const BASE_DIRECTIONS: usize = 32;

/// Number of directions per ring for blur rings.
const BLUR_DIRECTIONS: usize = 32;

/// Generate outline sampling offsets on concentric circles.
///
/// Base outline: `BASE_DIRECTIONS` equally-spaced directions on a circle of
/// radius `width`. Blur rings add extra circles at increasing radii with
/// `BLUR_DIRECTIONS` directions each, approximating a soft outline glow.
pub(crate) fn gen_outline_offsets(width: f32, blur: f32) -> Vec<(f32, f32)> {
    let mut offsets = Vec::new();

    // Base outline ring
    for k in 0..BASE_DIRECTIONS {
        let theta = TAU * k as f32 / BASE_DIRECTIONS as f32;
        offsets.push((width * theta.cos(), width * theta.sin()));
    }

    // Blur rings
    if blur > 0.0 {
        let steps = (blur * 2.0).round() as i32;
        for ring in 1..=steps {
            let r = width + blur * (ring as f32) / steps as f32;
            for k in 0..BLUR_DIRECTIONS {
                let theta = TAU * k as f32 / BLUR_DIRECTIONS as f32;
                offsets.push((r * theta.cos(), r * theta.sin()));
            }
        }
    }

    offsets
}

/// Cached outline offsets — returns `Arc<[(f32, f32)]>` for common (width, blur) pairs.
/// The cache is bounded at `MAX_ENTRIES` to prevent unbounded memory growth.
pub(crate) fn get_outline_offsets_cached(
    outline_w: f32,
    blur: f32,
    cache: &mut super::cache::OutlineOffsetCache,
) -> Arc<[(f32, f32)]> {
    let key = (
        (outline_w * 100.0).round() as i64,
        (blur * 100.0).round() as i64,
    );
    if let Some(offsets) = cache.map.get(&key) {
        return Arc::clone(offsets);
    }

    const MAX_ENTRIES: usize = 64;
    if cache.map.len() >= MAX_ENTRIES {
        cache.map.clear();
    }

    let v: Arc<[(f32, f32)]> = gen_outline_offsets(outline_w, blur).into();
    cache.map.insert(key, Arc::clone(&v));
    v
}
