//! System font enumeration and caching.
//!
//! Uses font-kit for system font discovery and oximedia-subtitle's `Font`
//! (fontdue-backed) for glyph rasterization.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use font_kit::handle::Handle as FkHandle;
use font_kit::source::SystemSource;

// ---------------------------------------------------------------------------
// Font entry
// ---------------------------------------------------------------------------

pub(crate) struct FontEntry {
    pub family_name: String,
    pub full_name: String,
    pub postscript_name: String,
    pub handle: FkHandle,
}

/// Global cache of all installed font entries, built once per process.
static GLOBAL_FONT_ENTRIES: OnceLock<Vec<FontEntry>> = OnceLock::new();

pub(crate) fn global_font_entries() -> &'static [FontEntry] {
    GLOBAL_FONT_ENTRIES.get_or_init(|| {
        let source = SystemSource::new();
        let mut entries = Vec::new();
        let handles = source.all_fonts().unwrap_or_default();
        for handle in handles {
            if let Ok(font) = handle.load() {
                entries.push(FontEntry {
                    family_name: font.family_name().to_lowercase(),
                    full_name: font.full_name().to_lowercase(),
                    postscript_name: font
                        .postscript_name()
                        .unwrap_or_default()
                        .to_lowercase(),
                    handle,
                });
            }
        }
        entries
    })
}

// ---------------------------------------------------------------------------
// Font cache
// ---------------------------------------------------------------------------

pub struct FontCache {
    /// Loaded raw font bytes keyed by font full name (lowercase).
    loaded: Mutex<HashMap<String, Arc<Vec<u8>>>>,
    /// Memoized font name -> matching indices.
    matching_cache: Mutex<HashMap<String, Arc<Vec<usize>>>>,
    /// Per-character glyph coverage cache: (font_full_name, char) -> has_glyph.
    coverage_cache: Mutex<HashMap<(String, char), bool>>,
}

impl FontCache {
    pub fn new() -> Self {
        Self {
            loaded: Mutex::new(HashMap::new()),
            matching_cache: Mutex::new(HashMap::new()),
            coverage_cache: Mutex::new(HashMap::new()),
        }
    }

    fn matches_name_exact(entry: &FontEntry, q: &str) -> bool {
        entry.family_name == q || entry.full_name == q || entry.postscript_name == q
    }

    /// Load raw font data for an entry.
    fn load_font_data(&self, entry: &FontEntry) -> Option<Arc<Vec<u8>>> {
        let key = &entry.full_name;
        {
            let loaded = self.loaded.lock().ok()?;
            if let Some(data) = loaded.get(key) {
                return Some(Arc::clone(data));
            }
        }
        if let Ok(font) = entry.handle.load() {
            if let Some(data) = font.copy_font_data() {
                let arc: Arc<Vec<u8>> = Arc::new(data.to_vec());
                self.loaded
                    .lock()
                    .unwrap()
                    .insert(key.clone(), Arc::clone(&arc));
                return Some(arc);
            }
        }
        None
    }

    fn find_matching_indices_inner(font_name: &str) -> Vec<usize> {
        let entries = global_font_entries();
        let q = font_name.to_lowercase();
        let mut indices = Vec::new();

        // Tier 1: exact match on any name variant
        for (i, entry) in entries.iter().enumerate() {
            if Self::matches_name_exact(entry, &q) {
                indices.push(i);
            }
        }
        if !indices.is_empty() {
            return indices;
        }
        // Tier 2: substring match
        for (i, entry) in entries.iter().enumerate() {
            if entry.family_name.contains(&q) || entry.full_name.contains(&q) {
                indices.push(i);
            }
        }
        if !indices.is_empty() {
            return indices;
        }
        // Tier 3: prefix match
        for (i, entry) in entries.iter().enumerate() {
            if entry.family_name.starts_with(&q) || entry.full_name.starts_with(&q) {
                indices.push(i);
            }
        }
        indices
    }

    /// Memoized version of find_matching_indices_inner.
    fn find_matching_indices(&self, font_name: &str) -> Arc<Vec<usize>> {
        let q = font_name.to_lowercase();
        if let Ok(cache) = self.matching_cache.lock() {
            if let Some(hit) = cache.get(&q) {
                return Arc::clone(hit);
            }
        }
        let result = Arc::new(Self::find_matching_indices_inner(&q));
        if let Ok(mut cache) = self.matching_cache.lock() {
            cache.insert(q, Arc::clone(&result));
        }
        result
    }

    /// Check if a char has a glyph in the font, with caching.
    fn char_has_glyph(&self, font_name: &str, c: char) -> bool {
        let key = (font_name.to_string(), c);
        if let Ok(cache) = self.coverage_cache.lock() {
            if let Some(&has) = cache.get(&key) {
                return has;
            }
        }
        // Load font and check — expensive, done once per (font, char) pair
        let entries = global_font_entries();
        let q = font_name.to_lowercase();
        let has = entries
            .iter()
            .find(|e| e.full_name == q)
            .and_then(|e| e.handle.load().ok())
            .map_or(false, |f| f.glyph_for_char(c).map_or(false, |gid| gid != 0));
        if let Ok(mut cache) = self.coverage_cache.lock() {
            cache.insert(key, has);
        }
        has
    }

    /// Find a font by name, optionally preferring a bold/italic variant.
    /// Returns raw font bytes.
    pub(crate) fn find_font(
        &self,
        font_name: &str,
        bold: bool,
        italic: bool,
    ) -> Option<Arc<Vec<u8>>> {
        // Try bold/italic variant names first
        if bold || italic {
            for variant in build_variant_names(font_name, bold, italic) {
                if let Some(data) = self.try_load_by_name(&variant) {
                    return Some(data);
                }
            }
        }
        // Fall back to the base font name
        self.try_load_by_name(font_name)
    }

    /// Try to load a font by name using the matching index cache.
    fn try_load_by_name(&self, name: &str) -> Option<Arc<Vec<u8>>> {
        let entries = global_font_entries();
        let indices = self.find_matching_indices(name);
        for &idx in indices.iter() {
            if let Some(data) = self.load_font_data(&entries[idx]) {
                return Some(data);
            }
        }
        None
    }

    /// Find a font that covers all given characters, preferring `preferred_name`.
    pub(crate) fn find_font_for_chars(
        &self,
        chars: &[char],
        preferred_name: &str,
        bold: bool,
        italic: bool,
    ) -> Option<Arc<Vec<u8>>> {
        if chars.is_empty() {
            return None;
        }
        let entries = global_font_entries();
        let q = preferred_name.to_lowercase();

        // Helper: check if an entry covers all chars
        let covers_all = |entry: &FontEntry| {
            chars.iter().all(|&c| self.char_has_glyph(&entry.full_name, c))
        };

        // Try preferred name first — exact match, including style variants
        for entry in entries {
            if Self::matches_name_exact(entry, &q) {
                if covers_all(entry) {
                    return self.load_font_data(entry);
                }
                break;
            }
        }

        // If bold/italic requested, try style variants of the preferred font
        if bold || italic {
            for variant in build_variant_names(preferred_name, bold, italic) {
                if let Some(data) = self.find_font(&variant, false, false)
                    .filter(|_| {
                        // Check coverage of the variant font
                        entries
                            .iter()
                            .find(|e| e.full_name == variant.to_lowercase())
                            .map_or(false, |entry| covers_all(entry))
                    })
                {
                    return Some(data);
                }
            }
        }

        // Scan all fonts for best coverage (use coverage cache)
        let mut best: Option<(Arc<Vec<u8>>, usize)> = None;
        for entry in entries {
            let covered = chars
                .iter()
                .filter(|&&c| self.char_has_glyph(&entry.full_name, c))
                .count();
                if covered == chars.len() {
                    return self.load_font_data(entry);
                }
                match &best {
                    None => {
                        if let Some(d) = self.load_font_data(entry) {
                            best = Some((d, covered));
                        }
                    }
                    Some((_, prev)) if covered > *prev => {
                        if let Some(d) = self.load_font_data(entry) {
                            best = Some((d, covered));
                        }
                    }
                    _ => {}
                }
        }
        best.map(|(d, _)| d)
    }

    /// List all installed font names (sorted, deduplicated).
    pub fn list_font_names(&self) -> Vec<String> {
        let entries = global_font_entries();
        let mut names: Vec<String> = entries.iter().map(|e| e.full_name.clone()).collect();
        names.sort();
        names.dedup();
        names
    }
}

/// Build a prioritized list of variant names for bold/italic font lookup.
/// E.g. for "Arial" + bold → ["Arial Bold", "Arial-Bold", "ArialBold"]
fn build_variant_names(base: &str, bold: bool, italic: bool) -> Vec<String> {
    let mut names = Vec::new();
    match (bold, italic) {
        (true, false) => {
            names.push(format!("{base} Bold"));
            names.push(format!("{base}-Bold"));
            names.push(format!("{base}Bold"));
        }
        (false, true) => {
            names.push(format!("{base} Italic"));
            names.push(format!("{base}-Italic"));
            names.push(format!("{base}Italic"));
        }
        (true, true) => {
            names.push(format!("{base} Bold Italic"));
            names.push(format!("{base}-BoldItalic"));
            names.push(format!("{base} BoldItalic"));
        }
        (false, false) => {}
    }
    names
}
