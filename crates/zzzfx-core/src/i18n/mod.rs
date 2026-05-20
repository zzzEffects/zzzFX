//! Shared i18n runtime — used by both AE and OFX backends.
//!
//! Language is detected once at plugin load time and never changes during the session.

use std::ffi::CStr;
use std::sync::atomic::{AtomicU8, Ordering};

use effect_settings::TrKey;

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

/// Zero-allocation translation. Returns `&'static str` for any key.
///
/// The `&str` is derived from `translate_cstr()` via `.to_str()` — language modules
/// only need to provide `translate_cstr()` returning `&CStr`.
pub fn tr(key: TrKey) -> &'static str {
    match lang() {
        Lang::En => key.en(),
        Lang::ZhCn => zh_cn::translate_cstr(key).to_str().unwrap(),
    }
}

/// Zero-allocation translation returning `&'static CStr` (for OFX native params).
pub fn tr_cstr(key: TrKey) -> &'static CStr {
    match lang() {
        Lang::En => key.en_cstr(),
        Lang::ZhCn => zh_cn::translate_cstr(key),
    }
}

/// Detect system language from environment variables.
/// Checks `LANG`, `LC_ALL`, `LC_MESSAGES` on Unix; locale name on Windows.
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
