#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

mod bindings;
mod i18n;
mod shared;
mod stroke;
mod repeater;
mod sprite_sheet;

use std::{ffi::c_int, ptr};

use crate::bindings::*;

// ---------------------------------------------------------------------------
// Global entry points — one DLL, three effects
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn OfxGetNumberOfPlugins() -> c_int {
    3
}

#[unsafe(no_mangle)]
pub extern "C" fn OfxGetPlugin(nth: c_int) -> *const OfxPlugin {
    match nth {
        0 => stroke::get_plugin(),
        1 => repeater::get_plugin(),
        2 => sprite_sheet::get_plugin(),
        _ => ptr::null(),
    }
}
