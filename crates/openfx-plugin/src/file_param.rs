use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;

use zzzfx::settings::TrKey;

use crate::bindings::*;
use crate::i18n;
use crate::shared::SuiteCache;

// ---------------------------------------------------------------------------
// Shared parameter name constants
// ---------------------------------------------------------------------------

/// Hidden String param for persisting file data (plain text or base64).
/// String params are natively handled by all OFX hosts, including undo.
pub const FILE_DATA_PARAM: &CStr = c"file_data";
pub const RELOAD_FILE_PARAM: &CStr = c"reload_file";

// ---------------------------------------------------------------------------
// Define a hidden String param for persisting file content
// ---------------------------------------------------------------------------

pub unsafe fn define_file_data_string_param(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    parent: &CStr,
) -> OfxResult<()> {
    let pdef = suites.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = suites.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pi = suites.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let mut pp: OfxPropertySetHandle = ptr::null_mut();
    pdef(param_set, kOfxParamTypeString.as_ptr(), FILE_DATA_PARAM.as_ptr(), &mut pp).ofx_ok()?;
    ps(pp, kOfxPropLabel.as_ptr(), 0, c"File Data".as_ptr()).ofx_ok()?;
    pi(pp, kOfxParamPropSecret.as_ptr(), 0, 1).ofx_ok()?;
    pi(pp, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
    ps(pp, kOfxParamPropDefault.as_ptr(), 0, c"".as_ptr()).ofx_ok()?;
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
// Read/write a hidden String param as plain text
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
// Base64 helpers (for binary file formats like MIDI, SVG)
// ---------------------------------------------------------------------------

pub unsafe fn read_file_data_base64(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
) -> OfxResult<Vec<u8>> {
    let b64 = read_string_param(suites, param_set, FILE_DATA_PARAM)?;
    if b64.is_empty() {
        return Ok(Vec::new());
    }
    base64_decode(&b64).map_err(|_| OfxStat::kOfxStatFailed)
}

pub unsafe fn write_file_data_base64(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    data: &[u8],
) -> OfxResult<()> {
    let b64 = base64_encode(data);
    write_string_param(suites, param_set, FILE_DATA_PARAM, &b64)
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 { CHARS[((triple >> 6) & 0x3F) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { CHARS[(triple & 0x3F) as usize] as char } else { '=' });
    }
    out
}

fn base64_decode(input: &str) -> Result<Vec<u8>, ()> {
    let input = input.trim_end_matches('=');
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer: u32 = 0;
    let mut bits = 0u32;
    for ch in input.chars() {
        let val = match ch {
            'A'..='Z' => ch as u32 - 'A' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 26,
            '0'..='9' => ch as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return Err(()),
        };
        buffer = (buffer << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Ok(out)
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
