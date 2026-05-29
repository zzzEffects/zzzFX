//! Shared i18n runtime for the Example Effect plugin family.
//!
//! Mirrors `zzzfx-core::i18n` but operates on [`ExTrKey`] instead of `TrKey`.
//!
//! Language is detected once at plugin load time and never changes during the session.

use std::ffi::CStr;
use std::sync::atomic::{AtomicU8, Ordering};

use effect_settings::ExTrKey;

mod zh_cn;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Lang {
    En = 0,
    ZhCn = 1,
}

static CURRENT_LANG: AtomicU8 = AtomicU8::new(Lang::En as u8);

pub fn set_lang(lang: Lang) {
    CURRENT_LANG.store(lang as u8, Ordering::Release);
}

pub fn lang() -> Lang {
    match CURRENT_LANG.load(Ordering::Acquire) {
        0 => Lang::En,
        1 => Lang::ZhCn,
        _ => Lang::En,
    }
}

/// Zero-allocation translation for `ExTrKey`.
pub fn tr(key: ExTrKey) -> &'static str {
    match lang() {
        Lang::En => key.en(),
        Lang::ZhCn => zh_cn::translate_cstr(key).to_str().unwrap(),
    }
}

/// Zero-allocation translation returning `&'static CStr` for `ExTrKey`.
pub fn tr_cstr(key: ExTrKey) -> &'static CStr {
    match lang() {
        Lang::En => key.en_cstr(),
        Lang::ZhCn => zh_cn::translate_cstr(key),
    }
}

/// Map a host-provided locale tag (e.g., `"zh_CN"`, `"en_US"`) to [`Lang`].
///
/// Returns `None` for unsupported languages — callers should default to
/// [`Lang::En`] rather than falling back to OS-level detection, so the host
/// application's language choice is respected even when we don't have
/// translations for that specific language.
pub fn lang_from_locale_tag(tag: &str) -> Option<Lang> {
    let s = tag.to_lowercase();
    if s.starts_with("zh") || s.contains("chinese") {
        Some(Lang::ZhCn)
    } else {
        None
    }
}

/// Detect system language from environment variables.
pub fn detect_system_lang() -> Lang {
    let check = |s: &str| -> Option<Lang> {
        let s = s.to_lowercase();
        if s.starts_with("zh") || s.contains("chinese") {
            Some(Lang::ZhCn)
        } else {
            None
        }
    };

    #[cfg(target_os = "windows")]
    {
        if let Ok(loc) = std::env::var("LANG") {
            if let Some(l) = check(&loc) {
                return l;
            }
        }
        unsafe {
            use std::ffi::OsString;
            use std::os::windows::ffi::OsStringExt;
            let mut buf = vec![0u16; 85];
            let len = windows_locale_name(&mut buf);
            if len > 0 {
                buf.truncate(len as usize);
                if let Ok(s) = OsString::from_wide(&buf).into_string() {
                    if let Some(l) = check(&s) {
                        return l;
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(val) = std::env::var(var) {
                if let Some(l) = check(&val) {
                    return l;
                }
            }
        }
    }

    Lang::En
}

#[cfg(target_os = "windows")]
unsafe fn windows_locale_name(buf: &mut [u16]) -> u32 {
    unsafe extern "system" {
        fn GetUserDefaultLocaleName(lpLocaleName: *mut u16, cchLocaleName: i32) -> i32;
    }
    (unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) }) as u32
}
