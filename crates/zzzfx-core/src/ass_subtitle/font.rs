//! System font enumeration and caching.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use font_kit::handle::Handle as FkHandle;
use font_kit::source::SystemSource;

// ---------------------------------------------------------------------------
// Font entry
// ---------------------------------------------------------------------------

pub(crate) struct FontEntry {
    family_name: String,
    full_name: String,
    postscript_name: String,
    handle: FkHandle,
}

/// Global cache of all installed font entries, built once per process.
static GLOBAL_FONT_ENTRIES: std::sync::OnceLock<Vec<FontEntry>> = std::sync::OnceLock::new();

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
    loaded: Mutex<HashMap<String, Arc<Vec<u8>>>>,
}

impl FontCache {
    pub fn new() -> Self {
        Self {
            loaded: Mutex::new(HashMap::new()),
        }
    }

    /// Try to match a font name against known variants (exact match, case-insensitive).
    fn matches_name_exact(entry: &FontEntry, q: &str) -> bool {
        entry.family_name == q || entry.full_name == q || entry.postscript_name == q
    }

    /// Lazy-load raw font data for an entry via interior mutability.
    fn load_font_data(&self, entry: &FontEntry) -> Option<Arc<Vec<u8>>> {
        let key = &entry.full_name;
        {
            let loaded = self.loaded.lock().unwrap();
            if let Some(data) = loaded.get(key) {
                return Some(Arc::clone(data));
            }
        }
        if let Ok(font) = entry.handle.load() {
            if let Some(data) = font.copy_font_data() {
                self.loaded
                    .lock()
                    .unwrap()
                    .insert(key.clone(), Arc::clone(&data));
                return Some(data);
            }
        }
        None
    }

    /// Look up entries matching a font name query, returning indices into global list.
    fn find_matching_indices(font_name: &str) -> Vec<usize> {
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

    /// Find a font by name.
    pub fn find_font(&self, font_name: &str) -> Option<Arc<Vec<u8>>> {
        let entries = global_font_entries();
        for idx in Self::find_matching_indices(font_name) {
            if let Some(data) = self.load_font_data(&entries[idx]) {
                return Some(data);
            }
        }
        None
    }

    /// Find a font that covers all given characters, preferring `preferred_name`.
    pub fn find_font_for_chars(
        &self,
        chars: &[char],
        preferred_name: &str,
    ) -> Option<Arc<Vec<u8>>> {
        if chars.is_empty() {
            return None;
        }
        let entries = global_font_entries();
        let q = preferred_name.to_lowercase();

        // Try preferred name first
        for entry in entries {
            if Self::matches_name_exact(entry, &q) {
                if let Some(data) = self.load_font_data(entry) {
                    if let Ok(font) = entry.handle.load() {
                        if chars.iter().all(|&c| font.glyph_for_char(c).is_some()) {
                            return Some(data);
                        }
                    }
                }
                break;
            }
        }

        // Scan all fonts for best coverage
        let mut best: Option<(Arc<Vec<u8>>, usize)> = None;
        for entry in entries {
            if let Ok(font) = entry.handle.load() {
                let covered = chars
                    .iter()
                    .filter(|&&c| font.glyph_for_char(c).is_some())
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
        }
        best.map(|(d, _)| d)
    }

    /// Find a font for a single character.
    pub fn find_font_for_char(&self, ch: char, preferred_name: &str) -> Option<Arc<Vec<u8>>> {
        let entries = global_font_entries();
        let q = preferred_name.to_lowercase();
        for entry in entries {
            if Self::matches_name_exact(entry, &q) {
                if let Ok(font) = entry.handle.load() {
                    if font.glyph_for_char(ch).is_some() {
                        return self.load_font_data(entry);
                    }
                }
            }
        }
        for entry in entries {
            if let Ok(font) = entry.handle.load() {
                if font.glyph_for_char(ch).is_some() {
                    return self.load_font_data(entry);
                }
            }
        }
        None
    }

    /// Group characters by Unicode script and find the best font for each group.
    pub fn find_fonts_for_chars_grouped(
        &self,
        chars: &[char],
        preferred_name: &str,
    ) -> HashMap<String, Option<Arc<Vec<u8>>>> {
        let mut groups: HashMap<String, Vec<char>> = HashMap::new();
        for &c in chars {
            groups
                .entry(script_group(c).to_string())
                .or_default()
                .push(c);
        }
        let mut result = HashMap::new();
        for (script, group_chars) in &groups {
            let font = self.find_font_for_chars(group_chars, preferred_name);
            result.insert(script.clone(), font);
        }
        result
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

// ---------------------------------------------------------------------------
// Unicode script classifier
// ---------------------------------------------------------------------------

/// Classify a character into a Unicode script group for font fallback purposes.
fn script_group(c: char) -> &'static str {
    if c <= '\u{007F}' {
        return "Latin";
    }
    if ('\u{4E00}'..='\u{9FFF}').contains(&c) {
        return "CJK";
    }
    if ('\u{3400}'..='\u{4DBF}').contains(&c) {
        return "CJK";
    }
    if ('\u{F900}'..='\u{FAFF}').contains(&c) {
        return "CJK";
    }
    if ('\u{3040}'..='\u{309F}').contains(&c) {
        return "Hiragana";
    }
    if ('\u{30A0}'..='\u{30FF}').contains(&c) {
        return "Katakana";
    }
    if ('\u{AC00}'..='\u{D7AF}').contains(&c) {
        return "Hangul";
    }
    if ('\u{0600}'..='\u{06FF}').contains(&c) {
        return "Arabic";
    }
    if ('\u{0E00}'..='\u{0E7F}').contains(&c) {
        return "Thai";
    }
    if ('\u{0400}'..='\u{04FF}').contains(&c) {
        return "Cyrillic";
    }
    if ('\u{0370}'..='\u{03FF}').contains(&c) {
        return "Greek";
    }
    if ('\u{0590}'..='\u{05FF}').contains(&c) {
        return "Hebrew";
    }
    if ('\u{0900}'..='\u{097F}').contains(&c) {
        return "Devanagari";
    }
    "Other"
}
