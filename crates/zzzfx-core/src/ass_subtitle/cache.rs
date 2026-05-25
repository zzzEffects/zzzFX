//! Render cache — reusable per-frame state to amortize allocations.

use std::collections::HashMap;

use oximedia_subtitle::text::TextLayoutEngine;

use super::types::*;

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
        }
    }

    /// Signal that the ASS script has been reloaded, invalidating all cached data
    /// keyed by event pointer addresses from the old script.
    pub fn invalidate_script_cache(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.event_cache.clear();
        self.font_engines.clear();
        self.text_layouts.clear();
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}
