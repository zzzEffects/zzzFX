#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

mod bindings;
mod i18n;
mod shared;
mod ambient_light;
mod cast_shadow;
mod chroma_key;
mod stroke;
mod repeater;
mod sprite_sheet;
mod ass_subtitle;
mod ascii_art;
mod long_shadow;
mod midi_display;
mod pixel_art;
mod latex_display;
mod file_param;
mod svg_display;

use std::{ffi::c_int, ptr};

use crate::bindings::*;

// ---------------------------------------------------------------------------
// Global entry points — one DLL, five effects
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn OfxGetNumberOfPlugins() -> c_int {
    13
}

#[unsafe(no_mangle)]
pub extern "C" fn OfxGetPlugin(nth: c_int) -> *const OfxPlugin {
    match nth {
        0 => chroma_key::get_plugin(),
        1 => stroke::get_plugin(),
        2 => repeater::get_plugin(),
        3 => sprite_sheet::get_plugin(),
        4 => ass_subtitle::get_plugin(),
        5 => ascii_art::get_plugin(),
        6 => pixel_art::get_plugin(),
        7 => long_shadow::get_plugin(),
        8 => ambient_light::get_plugin(),
        9 => midi_display::get_plugin(),
        10 => svg_display::get_plugin(),
        11 => cast_shadow::get_plugin(),
        12 => latex_display::get_plugin(),
        _ => ptr::null(),
    }
}
