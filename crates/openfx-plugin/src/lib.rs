#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

//! Minimal OpenFX plugin skeleton.
//!
//! This demonstrates the essential OFX entry points.
//! For a full implementation with parameter mapping and rendering, see the
//! original [ntsc-rs](https://github.com/valadaptive/ntsc-rs) project.
//!
//! ## Building
//!
//! ```bash
//! git submodule update --init
//! cargo build --package example-openfx-plugin --lib
//! ```
//!
//! ## Testing with a real host
//!
//! Use `cargo xtask build-ofx-plugin` to produce an `.ofx.bundle`, then
//! install it in your OFX host's plugins directory.

mod bindings;

use std::{
    ffi::{CStr, c_char, c_int, c_void},
    ptr,
    sync::OnceLock,
};

use bindings::*;

// SAFETY: The host promises raw string pointers in OfxPlugin remain valid.
unsafe impl Send for OfxPlugin {}
unsafe impl Sync for OfxPlugin {}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn OfxGetNumberOfPlugins() -> c_int {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn OfxGetPlugin(nth: c_int) -> *const OfxPlugin {
    static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();
    PLUGIN_INFO.get_or_init(|| OfxPlugin {
        pluginApi: kOfxImageEffectPluginApi.as_ptr(),
        apiVersion: 1,
        pluginIdentifier: c"com.example:ExampleEffect".as_ptr() as *const c_char,
        pluginVersionMajor: 0,
        pluginVersionMinor: 1,
        setHost: Some(set_host),
        mainEntry: Some(main_entry),
    });
    if nth == 0 {
        PLUGIN_INFO.get().unwrap() as *const _
    } else {
        ptr::null()
    }
}

// ---------------------------------------------------------------------------
// Host info (queried in set_host)
// ---------------------------------------------------------------------------

struct HostInfo {
    host: &'static OfxPropertySetStruct,
    fetch_suite: unsafe extern "C" fn(
        host: OfxPropertySetHandle,
        suiteName: *const c_char,
        suiteVersion: c_int,
    ) -> *const c_void,
}

static HOST_INFO: OnceLock<HostInfo> = OnceLock::new();

fn host() -> &'static HostInfo {
    HOST_INFO.get().expect("HostInfo not initialized")
}

unsafe fn fetch_suite_ptr(suite_name: &'static CStr) -> *const c_void {
    let h = host();
    unsafe {
        (h.fetch_suite)(
            h.host as *const _ as OfxPropertySetHandle,
            suite_name.as_ptr(),
            1,
        )
    }
}

// ---------------------------------------------------------------------------
// set_host — called by the OFX host at load time
// ---------------------------------------------------------------------------

unsafe extern "C" fn set_host(host_ptr: *mut OfxHost) {
    unsafe {
        if let Some(h) = host_ptr.as_ref() {
            if let (Some(host_props), Some(fetch)) = (h.host.as_ref(), h.fetchSuite) {
                let _ = HOST_INFO.set(HostInfo {
                    host: host_props,
                    fetch_suite: fetch,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// main_entry — action dispatcher (stub)
// ---------------------------------------------------------------------------

unsafe extern "C" fn main_entry(
    _action: *const c_char,
    _instance: *const c_void,
    _in_data: OfxPropertySetHandle,
    _out_data: OfxPropertySetHandle,
) -> OfxStatus {
    // In a full implementation, you would:
    // 1. Parse the action string (e.g. "OfxActionDescribe")
    // 2. Query host suites via fetch_suite_ptr()
    // 3. Dispatch to action handlers (describe, render, etc.)
    // 4. Map settings from the shared example-effect library to OFX parameters
    //
    // See the AE plugin skeleton for a complete parameter mapping example.

    OfxStat::kOfxStatReplyDefault
}
