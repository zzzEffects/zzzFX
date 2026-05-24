//! Render cache — reusable per-frame state to amortize allocations.

use std::collections::HashMap;
use std::sync::Arc;

use super::types::*;

// ---------------------------------------------------------------------------
// RenderCache
// ---------------------------------------------------------------------------

/// Reusable per-frame rendering state to amortize allocations.
pub struct RenderCache {
    pub temp_buf: Vec<u8>,
    pub(crate) generation: u64,
    pub(crate) glyph_cache: HashMap<GlyphCacheKey, CachedGlyph>,
    /// Font data cache keyed by `(generation, pointer_addr)`.
    pub(crate) font_data_cache: HashMap<(u64, usize), Option<Arc<Vec<u8>>>>,
    /// Event data cache keyed by `(generation, pointer_addr)`.
    pub(crate) event_cache: HashMap<(u64, usize), CachedEventData>,
    pub(crate) prev_dirty: DirtyRect,
    pub(crate) first_frame: bool,
    /// Bounded outline offset cache (replaces the old global static).
    pub(crate) outline_offsets: OutlineOffsetCache,
}

/// Outline offset cache with bounded size.
pub(crate) struct OutlineOffsetCache {
    pub map: HashMap<(i64, i64), Arc<[(f32, f32)]>>,
}

impl RenderCache {
    pub fn new() -> Self {
        Self {
            temp_buf: Vec::new(),
            generation: 0,
            glyph_cache: HashMap::new(),
            font_data_cache: HashMap::new(),
            event_cache: HashMap::new(),
            prev_dirty: DirtyRect::default(),
            first_frame: true,
            outline_offsets: OutlineOffsetCache {
                map: HashMap::new(),
            },
        }
    }

    /// Signal that the ASS script has been reloaded, invalidating all cached data
    /// keyed by event pointer addresses from the old script.
    pub fn invalidate_script_cache(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.event_cache.clear();
        self.font_data_cache.clear();
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}
