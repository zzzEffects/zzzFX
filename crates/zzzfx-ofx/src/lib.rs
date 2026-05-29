#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

mod bindings;
mod i18n;
mod shared;
mod ambient_light;
mod stroke;
mod repeater;
mod sprite_sheet;
mod ass_subtitle;
mod ascii_art;
mod long_shadow;
mod midi_display;
mod pixel_art;

use std::{ffi::c_int, ptr};

use crate::bindings::*;

// ---------------------------------------------------------------------------
// Global entry points — one DLL, five effects
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn OfxGetNumberOfPlugins() -> c_int {
    9
}

#[unsafe(no_mangle)]
pub extern "C" fn OfxGetPlugin(nth: c_int) -> *const OfxPlugin {
    match nth {
        0 => stroke::get_plugin(),
        1 => repeater::get_plugin(),
        2 => sprite_sheet::get_plugin(),
        3 => ass_subtitle::get_plugin(),
        4 => ascii_art::get_plugin(),
        5 => pixel_art::get_plugin(),
        6 => long_shadow::get_plugin(),
        7 => ambient_light::get_plugin(),
        8 => midi_display::get_plugin(),
        _ => ptr::null(),
    }
}
