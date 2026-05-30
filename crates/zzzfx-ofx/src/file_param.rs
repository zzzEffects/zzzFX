use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr;

use zzzfx_core::settings::TrKey;

use crate::bindings::*;
use crate::i18n;
use crate::shared::SuiteCache;

// ---------------------------------------------------------------------------
// Shared parameter name constants
// ---------------------------------------------------------------------------

pub const FILE_DATA_PARAM: &CStr = c"file_data";
pub const RELOAD_FILE_PARAM: &CStr = c"reload_file";

// ---------------------------------------------------------------------------
// Define a Custom param for persisting file bytes
// ---------------------------------------------------------------------------

pub unsafe fn define_file_data_param(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    parent: &CStr,
) -> OfxResult<()> {
    let pdef = suites.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = suites.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pi = suites.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let mut pp: OfxPropertySetHandle = ptr::null_mut();
    pdef(param_set, kOfxParamTypeCustom.as_ptr(), FILE_DATA_PARAM.as_ptr(), &mut pp).ofx_ok()?;
    ps(pp, kOfxPropLabel.as_ptr(), 0, c"File Data".as_ptr()).ofx_ok()?;
    pi(pp, kOfxParamPropSecret.as_ptr(), 0, 1).ofx_ok()?;
    pi(pp, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
    ps(pp, kOfxParamPropParent.as_ptr(), 0, parent.as_ptr()).ofx_ok()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Define a hidden Reload push button
// ---------------------------------------------------------------------------

pub unsafe fn define_reload_button(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    parent: &CStr,
) -> OfxResult<()> {
    let pdef = suites.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = suites.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pi = suites.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let mut pp: OfxPropertySetHandle = ptr::null_mut();
    pdef(param_set, kOfxParamTypePushButton.as_ptr(), RELOAD_FILE_PARAM.as_ptr(), &mut pp).ofx_ok()?;
    ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeReloadFile).as_ptr()).ofx_ok()?;
    ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeReloadFileHint).as_ptr()).ofx_ok()?;
    pi(pp, kOfxParamPropSecret.as_ptr(), 0, 1).ofx_ok()?;
    ps(pp, kOfxParamPropParent.as_ptr(), 0, parent.as_ptr()).ofx_ok()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Read raw bytes from a Custom param (used in render for project-reload recovery)
// ---------------------------------------------------------------------------

pub unsafe fn read_custom_param_bytes(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    name: &CStr,
) -> OfxResult<Vec<u8>> {
    let pgh = suites.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = suites.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    let mut p: OfxParamHandle = ptr::null_mut();
    pgh(param_set, name.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;

    let mut data_ptr: *mut c_void = ptr::null_mut();
    let mut data_size: c_int = 0;
    // time=0.0 for non-animated Custom param
    pgv(p, 0.0, &mut data_ptr, &mut data_size).ofx_ok()?;

    if data_ptr.is_null() || data_size <= 0 {
        return Ok(Vec::new());
    }

    let bytes = std::slice::from_raw_parts(data_ptr as *const u8, data_size as usize).to_vec();
    Ok(bytes)
}

// ---------------------------------------------------------------------------
// Write raw bytes to a Custom param
// ---------------------------------------------------------------------------

pub unsafe fn write_custom_param_bytes(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    name: &CStr,
    data: &[u8],
) -> OfxResult<()> {
    let pgh = suites.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let psv = suites.parameter_suite.paramSetValue.ok_or(OfxStat::kOfxStatFailed)?;

    let mut p: OfxParamHandle = ptr::null_mut();
    pgh(param_set, name.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;

    let ptr = data.as_ptr() as *const c_void;
    let len = data.len() as c_int;
    psv(p, ptr, len).ofx_ok()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Read string from a hidden String param
// ---------------------------------------------------------------------------

pub unsafe fn read_string_param(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    name: &CStr,
) -> OfxResult<String> {
    let pgh = suites.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = suites.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    let mut p: OfxParamHandle = ptr::null_mut();
    pgh(param_set, name.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;

    let mut s_ptr: *mut c_char = ptr::null_mut();
    pgv(p, 0.0, &mut s_ptr).ofx_ok()?;

    if s_ptr.is_null() {
        return Ok(String::new());
    }

    Ok(CStr::from_ptr(s_ptr).to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// Write string to a String param
// ---------------------------------------------------------------------------

pub unsafe fn write_string_param(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    name: &CStr,
    value: &str,
) -> OfxResult<()> {
    let pgh = suites.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let psv = suites.parameter_suite.paramSetValue.ok_or(OfxStat::kOfxStatFailed)?;

    let mut p: OfxParamHandle = ptr::null_mut();
    pgh(param_set, name.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;

    let cstr = CString::new(value).map_err(|_| OfxStat::kOfxStatFailed)?;
    psv(p, cstr.as_ptr() as *const c_void).ofx_ok()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Reveal a hidden parameter (set kOfxParamPropSecret = 0)
// ---------------------------------------------------------------------------

pub unsafe fn reveal_param(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    name: &CStr,
) -> OfxResult<()> {
    let pgh = suites.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let psi = suites.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;

    let mut p: OfxParamHandle = ptr::null_mut();
    let mut pp: OfxPropertySetHandle = ptr::null_mut();
    pgh(param_set, name.as_ptr(), &mut p, &mut pp).ofx_ok()?;

    psi(pp, kOfxParamPropSecret.as_ptr(), 0, 0).ofx_ok()?;
    Ok(())
}
