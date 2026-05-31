//! Translation key infrastructure shared across all consumers.
//!
//! The [`i18n_keys!`] macro is invoked once per product to generate a
//! translation-key enum. Each generated enum implements [`I18nKey`], which
//! provides `.en()` (English) and `.en_cstr()` (null-terminated for FFI).
//! Consumer crates use the [`Settings`](crate::Settings) trait's associated
//! `Key` type to select which key enum their descriptors use.

use std::ffi::CStr;

/// Trait implemented by all i18n key enums generated via `i18n_keys!`.
pub trait I18nKey: Copy + std::fmt::Debug {
    /// English (canonical) string for this key.
    fn en(self) -> &'static str;

    /// English string as a null-terminated `&CStr` for FFI.
    fn en_cstr(self) -> &'static CStr;
}

#[macro_export]
macro_rules! i18n_keys {
    ($vis:vis $EnumName:ident { $($(#[$comment:meta])* $variant:ident = $en:literal;)* }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $EnumName {
            $($variant,)*
        }

        impl $EnumName {
            /// English (canonical) string for this key.
            pub fn en(self) -> &'static str {
                match self {
                    $($EnumName::$variant => $en,)*
                }
            }

            /// English string as a null-terminated `&CStr` for FFI.
            pub fn en_cstr(self) -> &'static CStr {
                match self {
                    $(
                        $EnumName::$variant => {
                            let bytes: &[u8] = concat!($en, "\0").as_bytes();
                            unsafe { CStr::from_bytes_with_nul_unchecked(bytes) }
                        }
                    ),*
                }
            }

            /// All variants (for iteration / tests).
            pub fn all() -> &'static [$EnumName] {
                &[$($EnumName::$variant,)*]
            }
        }

        impl $crate::i18n::i18n_keys::I18nKey for $EnumName {
            fn en(self) -> &'static str { self.en() }
            fn en_cstr(self) -> &'static CStr { self.en_cstr() }
        }
    };
}
