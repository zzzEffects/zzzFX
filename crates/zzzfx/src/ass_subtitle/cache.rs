//! Render cache — reusable per-frame state to amortize allocations.

use std::collections::HashMap;

use oximedia_subtitle::text::TextLayoutEngine;

use super::types::*;

// ---------------------------------------------------------------------------
// Cache capacity constants
// ---------------------------------------------------------------------------

/// Max cached event data entries before eviction kicks in.
pub(crate) const MAX_EVENT_CACHE: usize = 2000;
/// Max cached TextLayoutEngine instances.
pub(crate) const MAX_FONT_ENGINES: usize = 32;
/// Max cached TextLayout results.
pub(crate) const MAX_TEXT_LAYOUTS: usize = 500;

// ---------------------------------------------------------------------------
// Cache keys
// ---------------------------------------------------------------------------

/// Key for caching TextLayoutEngine instances per font+size combination.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct FontEngineKey {
    /// Pointer identity of the underlying font data Arc.
    pub data_ptr: usize,
    /// Bucketed effective pixel size to reduce cache fragmentation.
    pub size_bucket: u32,
}

/// Key for caching precomputed TextLayout results.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct LayoutCacheKey {
    pub event_ptr: usize,
    pub size_bucket: u32,
    /// Hash of clean_text computed once during event data construction.
    pub text_hash: u64,
    /// Pointer identity of the font data Arc. Ensures cache misses when
    /// font_override or font resolution changes to a different font face.
    pub font_data_ptr: usize,
}

// ---------------------------------------------------------------------------
// RenderCache
// ---------------------------------------------------------------------------

/// Reusable per-frame rendering state to amortize allocations.
pub struct RenderCache {
    pub temp_buf: Vec<u8>,
    pub(crate) generation: u64,
    /// Event data cache keyed by (generation, pointer_addr).
    pub(crate) event_cache: HashMap<(u64, usize), CachedEventData>,
    /// Cached TextLayoutEngine instances keyed by (font_data_ptr, size_bucket).
    pub(crate) font_engines: HashMap<FontEngineKey, TextLayoutEngine>,
    /// Cached TextLayout results keyed by (event_ptr, size_bucket, text_hash).
    pub(crate) text_layouts: HashMap<LayoutCacheKey, oximedia_subtitle::text::TextLayout>,
    pub(crate) prev_dirty: DirtyRect,
    pub(crate) first_frame: bool,
    /// Reusable per-event GPU glyph metadata buffer.
    #[cfg(feature = "gpu")]
    pub(crate) glyph_gpu_data_buf: Vec<crate::gpu::ass_glyph::GlyphGpuData>,
    /// Reusable per-event GPU glyph bitmap buffer.
    #[cfg(feature = "gpu")]
    pub(crate) bitmap_bytes_buf: Vec<u8>,
}

impl RenderCache {
    pub fn new() -> Self {
        Self {
            temp_buf: Vec::new(),
            generation: 0,
            event_cache: HashMap::new(),
            font_engines: HashMap::new(),
            text_layouts: HashMap::new(),
            prev_dirty: DirtyRect::default(),
            first_frame: true,
            #[cfg(feature = "gpu")]
            glyph_gpu_data_buf: Vec::new(),
            #[cfg(feature = "gpu")]
            bitmap_bytes_buf: Vec::new(),
        }
    }

    /// Signal that the ASS script has been reloaded, invalidating all cached data
    /// keyed by event pointer addresses from the old script.
    pub fn invalidate_script_cache(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.event_cache.clear();
        self.font_engines.clear();
        self.text_layouts.clear();
        #[cfg(feature = "gpu")]
        {
            self.glyph_gpu_data_buf.clear();
            self.bitmap_bytes_buf.clear();
        }
    }

    /// Evict entries from a HashMap when it exceeds `max` and `key` is not present.
    /// Keeps ~75% of entries, dropping the rest via iteration order.
    pub(crate) fn evict_if_full<K: Eq + std::hash::Hash, V>(map: &mut HashMap<K, V>, max: usize, key: &K) {
        if map.len() >= max && !map.contains_key(key) {
            let keep = (max * 3) / 4;
            let mut n = 0usize;
            map.retain(|_, _| { n += 1; n <= keep });
        }
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}
