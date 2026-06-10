use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_int, c_void},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use zzzfx::blend::RECIP_255;
use zzzfx::settings::TrKey;
use zzzfx::settings::{
    EnumValue, SettingDescriptor, SettingID, SettingKind, Settings, SettingsList,
};

use crate::bindings::*;

// ---------------------------------------------------------------------------
// Per-frame buffer pool
// ---------------------------------------------------------------------------

thread_local! {
    static RENDER_POOL: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

/// RAII buffer that reuses a thread-local allocation.
///
/// On construction, takes the buffer from a per-thread pool (resizing as needed)
/// or allocates fresh if the pool is re-entrantly borrowed. On drop, returns the
/// buffer to the pool so the next frame reuses the allocation.
pub struct ScopedBuffer {
    buf: Option<Vec<u8>>,
}

impl ScopedBuffer {
    pub fn new(size: usize) -> Self {
        let buf = RENDER_POOL.with(|cell| {
            cell.try_borrow_mut()
                .map(|mut guard| {
                    let mut old = std::mem::replace(&mut *guard, Vec::new());
                    old.resize(size, 0);
                    old
                })
                .unwrap_or_else(|_| vec![0u8; size])
        });
        ScopedBuffer { buf: Some(buf) }
    }
}

impl Drop for ScopedBuffer {
    fn drop(&mut self) {
        if let Some(buf) = self.buf.take() {
            RENDER_POOL.with(|cell| {
                if let Ok(mut guard) = cell.try_borrow_mut() {
                    let _ = std::mem::replace(&mut *guard, buf);
                }
            });
        }
    }
}

impl std::ops::Deref for ScopedBuffer {
    type Target = Vec<u8>;
    fn deref(&self) -> &Vec<u8> {
        self.buf.as_ref().expect("ScopedBuffer accessed after drop")
    }
}

impl std::ops::DerefMut for ScopedBuffer {
    fn deref_mut(&mut self) -> &mut Vec<u8> {
        self.buf.as_mut().expect("ScopedBuffer accessed after drop")
    }
}

// ---------------------------------------------------------------------------
// OFX → renderer coordinate conversion helpers
// ---------------------------------------------------------------------------
// OFX uses bottom-up coordinates (Y=0 at bottom, +Y=up) and CCW angles.
// The internal pixel buffer is top-down (Y=0 at top, +Y=down) with CW angles.
// These helpers centralize the conversion so every effect stays consistent.

/// Convert OFX Y position (0=bottom, 1=top) to pixel-buffer Y (0=top, 1=bottom).
#[inline]
pub fn ofx_y_to_renderer(ofx_y: f64) -> f64 { 1.0 - ofx_y }

/// Convert OFX angle in degrees (+=CCW, Y-up) to pixel-buffer angle (+=CW, Y-down).
#[inline]
pub fn ofx_angle_to_renderer(ofx_deg: f64) -> f64 { -ofx_deg }

// SAFETY: OfxPlugin is stored in a OnceLock and initialized once at plugin load
// time, before any concurrent access from the host. The contained function pointers
// and `*const c_char` are valid for the lifetime of the plugin (the host guarantees
// suites and string literals remain valid until the plugin is unloaded).
// Send: the plugin descriptor is safe to transfer between threads after init.
// Sync: all access to OfxPlugin fields is read-only after initialization.
unsafe impl Send for OfxPlugin {}
unsafe impl Sync for OfxPlugin {}

// ---------------------------------------------------------------------------
// HostInfo
// ---------------------------------------------------------------------------

pub struct HostInfo {
    pub host: &'static OfxPropertySetStruct,
    pub fetch_suite: unsafe extern "C" fn(
        host: OfxPropertySetHandle,
        suiteName: *const c_char,
        suiteVersion: c_int,
    ) -> *const c_void,
}

// ---------------------------------------------------------------------------
// SuiteCache — fetched once per effect
// ---------------------------------------------------------------------------

pub struct SuiteCache {
    pub host_info: HostInfo,
    pub property_suite: &'static OfxPropertySuiteV1,
    pub image_effect_suite: &'static OfxImageEffectSuiteV1,
    pub parameter_suite: &'static OfxParameterSuiteV1,
    pub interact_suite: Option<&'static OfxInteractSuiteV1>,
    pub supports_multiple_clip_depths: AtomicBool,
}

impl SuiteCache {
    pub unsafe fn new(host_info: HostInfo) -> OfxResult<Self> {
        let property_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _,
            kOfxPropertySuite.as_ptr(),
            1,
        ) as *const OfxPropertySuiteV1;
        let image_effect_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _,
            kOfxImageEffectSuite.as_ptr(),
            1,
        ) as *const OfxImageEffectSuiteV1;
        let parameter_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _,
            kOfxParameterSuite.as_ptr(),
            1,
        ) as *const OfxParameterSuiteV1;
        let interact_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _,
            c"OfxInteractSuite".as_ptr(),
            1,
        ) as *const OfxInteractSuiteV1;
        Ok(Self {
            host_info,
            property_suite: property_suite
                .as_ref()
                .ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            image_effect_suite: image_effect_suite
                .as_ref()
                .ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            parameter_suite: parameter_suite
                .as_ref()
                .ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            interact_suite: unsafe { interact_suite.as_ref() },
            supports_multiple_clip_depths: AtomicBool::new(false),
        })
    }
}

// ---------------------------------------------------------------------------
// String caches — generic over settings type T
// ---------------------------------------------------------------------------

pub type StringCache<T> = HashMap<SettingID<T>, (CString, CString, Option<CString>, Option<CString>)>;
pub type MenuItemCache<T> = HashMap<(SettingID<T>, u32), (CString, Option<CString>)>;

pub unsafe fn build_string_cache<T: Settings<Key = TrKey>>(
    settings_list: &SettingsList<T>,
) -> (StringCache<T>, MenuItemCache<T>)
where
    T: Clone,
{
    let mut strings = HashMap::new();
    let mut menu_item_strings = HashMap::new();
    let safe_cstr = |s: &str| CString::new(s).unwrap_or_else(|_| CString::new("").unwrap());
    for descriptor in settings_list.all_descriptors() {
        let id = &descriptor.id;
        let id_str = safe_cstr(descriptor.id.name);
        let label = safe_cstr(zzzfx::i18n::tr(descriptor.label_key));
        let description = descriptor
            .description_key
            .map(|k| safe_cstr(zzzfx::i18n::tr(k)));
        let group_name = if let SettingKind::Group { .. } = descriptor.kind {
            Some(safe_cstr(&format!("{}_group", descriptor.id.name)))
        } else {
            None
        };
        strings.insert(id.clone(), (id_str, label, description, group_name));
        if let SettingKind::Enumeration { options } = &descriptor.kind {
            for item in options {
                let lbl = safe_cstr(zzzfx::i18n::tr(item.label_key));
                menu_item_strings.insert(
                    (id.clone(), item.index),
                    (lbl, item.description_key.map(|k| safe_cstr(zzzfx::i18n::tr(k)))),
                );
            }
        }
    }
    (strings, menu_item_strings)
}

// ---------------------------------------------------------------------------
// Generic parameter definition helper
// ---------------------------------------------------------------------------

pub unsafe fn define_single_param<T: Settings<Key = TrKey> + Clone>(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    descriptor: &SettingDescriptor<T>,
    default_settings: &T,
    parent: &CStr,
    strings: &StringCache<T>,
    menu_item_strings: &MenuItemCache<T>,
) -> OfxResult<()> {
    let pdef = suites
        .parameter_suite
        .paramDefine
        .ok_or(OfxStat::kOfxStatFailed)?;
    let pd = suites
        .property_suite
        .propSetDouble
        .ok_or(OfxStat::kOfxStatFailed)?;
    let pi = suites
        .property_suite
        .propSetInt
        .ok_or(OfxStat::kOfxStatFailed)?;
    let ps = suites
        .property_suite
        .propSetString
        .ok_or(OfxStat::kOfxStatFailed)?;

    let ds = strings.get(&descriptor.id).ok_or(OfxStat::kOfxStatFailed)?;
    let id_cstr = ds.0.as_c_str();
    let mut pp: OfxPropertySetHandle = ptr::null_mut();

    match &descriptor.kind {
        SettingKind::Enumeration { options } => {
            pdef(
                param_set,
                kOfxParamTypeChoice.as_ptr(),
                id_cstr.as_ptr(),
                &mut pp,
            )
            .ofx_ok()?;
            let dv = default_settings
                .get_field::<EnumValue>(&descriptor.id)
                .map_err(|_| OfxStat::kOfxStatFailed)?
                .0;
            let mut di: usize = 0;
            for (i, mi) in options.iter().enumerate() {
                let is = menu_item_strings
                    .get(&(descriptor.id.clone(), mi.index))
                    .ok_or(OfxStat::kOfxStatFailed)?;
                ps(
                    pp,
                    kOfxParamPropChoiceOption.as_ptr(),
                    i as i32,
                    is.0.as_c_str().as_ptr(),
                )
                .ofx_ok()?;
                if mi.index == dv {
                    di = i;
                }
            }
            pi(pp, kOfxParamPropDefault.as_ptr(), 0, di as i32).ofx_ok()?;
        }
        SettingKind::Percentage { .. } => {
            let dv = default_settings
                .get_field::<f32>(&descriptor.id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            pdef(
                param_set,
                kOfxParamTypeDouble.as_ptr(),
                id_cstr.as_ptr(),
                &mut pp,
            )
            .ofx_ok()?;
            ps(
                pp,
                kOfxParamPropDoubleType.as_ptr(),
                0,
                kOfxParamDoubleTypeScale.as_ptr(),
            )
            .ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 0, dv as f64).ofx_ok()?;
            pd(pp, kOfxParamPropMin.as_ptr(), 0, 0.0).ofx_ok()?;
            pd(pp, kOfxParamPropDisplayMin.as_ptr(), 0, 0.0).ofx_ok()?;
            pd(pp, kOfxParamPropMax.as_ptr(), 0, 1.0).ofx_ok()?;
            pd(pp, kOfxParamPropDisplayMax.as_ptr(), 0, 1.0).ofx_ok()?;
        }
        SettingKind::FloatRange { range, .. } => {
            let dv = default_settings
                .get_field::<f32>(&descriptor.id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            pdef(
                param_set,
                kOfxParamTypeDouble.as_ptr(),
                id_cstr.as_ptr(),
                &mut pp,
            )
            .ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 0, dv as f64).ofx_ok()?;
            pd(pp, kOfxParamPropMin.as_ptr(), 0, *range.start() as f64).ofx_ok()?;
            pd(
                pp,
                kOfxParamPropDisplayMin.as_ptr(),
                0,
                *range.start() as f64,
            )
            .ofx_ok()?;
            pd(pp, kOfxParamPropMax.as_ptr(), 0, *range.end() as f64).ofx_ok()?;
            pd(
                pp,
                kOfxParamPropDisplayMax.as_ptr(),
                0,
                *range.end() as f64,
            )
            .ofx_ok()?;
        }
        SettingKind::IntRange { range } => {
            let dv = default_settings
                .get_field::<i32>(&descriptor.id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            pdef(
                param_set,
                kOfxParamTypeInteger.as_ptr(),
                id_cstr.as_ptr(),
                &mut pp,
            )
            .ofx_ok()?;
            pi(pp, kOfxParamPropDefault.as_ptr(), 0, dv).ofx_ok()?;
            pi(pp, kOfxParamPropMin.as_ptr(), 0, *range.start()).ofx_ok()?;
            pi(
                pp,
                kOfxParamPropDisplayMin.as_ptr(),
                0,
                *range.start(),
            )
            .ofx_ok()?;
            pi(pp, kOfxParamPropMax.as_ptr(), 0, *range.end()).ofx_ok()?;
            pi(
                pp,
                kOfxParamPropDisplayMax.as_ptr(),
                0,
                *range.end(),
            )
            .ofx_ok()?;
        }
        SettingKind::Boolean => {
            let dv = default_settings
                .get_field::<bool>(&descriptor.id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            pdef(
                param_set,
                kOfxParamTypeBoolean.as_ptr(),
                id_cstr.as_ptr(),
                &mut pp,
            )
            .ofx_ok()?;
            pi(pp, kOfxParamPropDefault.as_ptr(), 0, dv as i32).ofx_ok()?;
        }
        SettingKind::String { secret, multiline, animates } => {
            let dv = default_settings
                .get_field::<String>(&descriptor.id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            pdef(
                param_set,
                kOfxParamTypeString.as_ptr(),
                id_cstr.as_ptr(),
                &mut pp,
            )
            .ofx_ok()?;
            let dv_cstr = CString::new(dv.as_str()).unwrap_or_else(|_| CString::new("").unwrap());
            ps(pp, kOfxParamPropDefault.as_ptr(), 0, dv_cstr.as_ptr()).ofx_ok()?;
            if *multiline {
                ps(pp, kOfxParamPropStringMode.as_ptr(), 0, kOfxParamStringIsMultiLine.as_ptr()).ofx_ok()?;
            }
            if *secret {
                pi(pp, kOfxParamPropSecret.as_ptr(), 0, 1).ofx_ok()?;
            }
            if !animates {
                pi(pp, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
            }
        }
        SettingKind::PushButton { secret } => {
            pdef(
                param_set,
                kOfxParamTypePushButton.as_ptr(),
                id_cstr.as_ptr(),
                &mut pp,
            )
            .ofx_ok()?;
            if *secret {
                pi(pp, kOfxParamPropSecret.as_ptr(), 0, 1).ofx_ok()?;
            }
        }
        SettingKind::ColorRGBA { r_id, g_id, b_id, a_id } => {
            let dv_r = default_settings
                .get_field::<f32>(r_id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            let dv_g = default_settings
                .get_field::<f32>(g_id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            let dv_b = default_settings
                .get_field::<f32>(b_id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            let dv_a = default_settings
                .get_field::<f32>(a_id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            pdef(
                param_set,
                kOfxParamTypeRGBA.as_ptr(),
                id_cstr.as_ptr(),
                &mut pp,
            )
            .ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 0, dv_r as f64).ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 1, dv_g as f64).ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 2, dv_b as f64).ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 3, dv_a as f64).ofx_ok()?;
        }
        SettingKind::ColorRGB { r_id, g_id, b_id } => {
            let dv_r = default_settings
                .get_field::<f32>(r_id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            let dv_g = default_settings
                .get_field::<f32>(g_id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            let dv_b = default_settings
                .get_field::<f32>(b_id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            pdef(
                param_set,
                kOfxParamTypeRGB.as_ptr(),
                id_cstr.as_ptr(),
                &mut pp,
            )
            .ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 0, dv_r as f64).ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 1, dv_g as f64).ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 2, dv_b as f64).ofx_ok()?;
        }
        SettingKind::Group { children } => {
            let dv = default_settings
                .get_field::<bool>(&descriptor.id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            let gnc: &CStr = ds.3.as_ref().ok_or(OfxStat::kOfxStatFailed)?.as_c_str();
            pdef(
                param_set,
                kOfxParamTypeGroup.as_ptr(),
                gnc.as_ptr(),
                &mut pp,
            )
            .ofx_ok()?;
            let mut cb: OfxPropertySetHandle = ptr::null_mut();
            pdef(
                param_set,
                kOfxParamTypeBoolean.as_ptr(),
                id_cstr.as_ptr(),
                &mut cb,
            )
            .ofx_ok()?;
            let enabled_label = CString::new(zzzfx::i18n::tr(TrKey::CommonEnabled))
                .unwrap_or_else(|_| CString::new("").unwrap());
            ps(cb, kOfxPropLabel.as_ptr(), 0, enabled_label.as_ptr()).ofx_ok()?;
            pi(cb, kOfxParamPropDefault.as_ptr(), 0, dv as i32).ofx_ok()?;
            ps(cb, kOfxParamPropParent.as_ptr(), 0, gnc.as_ptr()).ofx_ok()?;
            pi(cb, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
            for child in children {
                define_single_param(
                    suites,
                    param_set,
                    child,
                    default_settings,
                    gnc,
                    strings,
                    menu_item_strings,
                )?;
            }
        }
    }

    if !pp.is_null() {
        ps(pp, kOfxPropLabel.as_ptr(), 0, ds.1.as_ptr()).ofx_ok()?;
        if let Some(desc) = ds.2.as_deref() {
            ps(pp, kOfxParamPropHint.as_ptr(), 0, desc.as_ptr()).ofx_ok()?;
        }
        ps(pp, kOfxParamPropParent.as_ptr(), 0, parent.as_ptr()).ofx_ok()?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Generic parameter reading helper
// ---------------------------------------------------------------------------

pub unsafe fn read_generic_param<T: Settings<Key = TrKey> + Clone>(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    time: f64,
    desc: &SettingDescriptor<T>,
    dst: &mut T,
    strings: &StringCache<T>,
) -> OfxResult<()> {
    let pgh = suites
        .parameter_suite
        .paramGetHandle
        .ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = suites
        .parameter_suite
        .paramGetValueAtTime
        .ok_or(OfxStat::kOfxStatFailed)?;
    let ds = strings.get(&desc.id).ok_or(OfxStat::kOfxStatFailed)?;
    let id_cstr = ds.0.as_c_str();

    let mut p: OfxParamHandle = ptr::null_mut();
    pgh(param_set, id_cstr.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;

    match &desc.kind {
        SettingKind::Enumeration { options } => {
            let mut idx: c_int = 0;
            pgv(p, time, &mut idx).ofx_ok()?;
            if idx >= 0 && (idx as usize) < options.len() {
                dst.set_field::<EnumValue>(&desc.id, EnumValue(options[idx as usize].index))
                    .ok();
            }
        }
        SettingKind::Percentage { .. } => {
            let mut v: f64 = 0.0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<f32>(&desc.id, v.clamp(0.0, 1.0) as f32)
                .ok();
        }
        SettingKind::FloatRange { range, .. } => {
            let mut v: f64 = 0.0;
            pgv(p, time, &mut v).ofx_ok()?;
            let lo = *range.start() as f64;
            let hi = *range.end() as f64;
            dst.set_field::<f32>(&desc.id, v.clamp(lo, hi) as f32)
                .ok();
        }
        SettingKind::IntRange { range } => {
            let mut v: c_int = 0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<i32>(&desc.id, v.clamp(*range.start(), *range.end()))
                .ok();
        }
        SettingKind::Boolean => {
            let mut v: c_int = 0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<bool>(&desc.id, v != 0).ok();
        }
        SettingKind::String { .. } => {
            let mut s_ptr: *mut c_char = ptr::null_mut();
            pgv(p, time, &mut s_ptr).ofx_ok()?;
            let s = if s_ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(s_ptr).to_string_lossy().into_owned()
            };
            dst.set_field::<String>(&desc.id, s).ok();
        }
        SettingKind::PushButton { .. } => {}
        SettingKind::ColorRGBA { r_id, g_id, b_id, a_id } => {
            let mut r: f64 = 0.0;
            let mut g: f64 = 0.0;
            let mut b: f64 = 0.0;
            let mut a: f64 = 0.0;
            pgv(p, time, &mut r, &mut g, &mut b, &mut a).ofx_ok()?;
            dst.set_field::<f32>(r_id, r as f32).ok();
            dst.set_field::<f32>(g_id, g as f32).ok();
            dst.set_field::<f32>(b_id, b as f32).ok();
            dst.set_field::<f32>(a_id, a as f32).ok();
        }
        SettingKind::ColorRGB { r_id, g_id, b_id } => {
            let mut r: f64 = 0.0;
            let mut g: f64 = 0.0;
            let mut b: f64 = 0.0;
            pgv(p, time, &mut r, &mut g, &mut b).ofx_ok()?;
            dst.set_field::<f32>(r_id, r as f32).ok();
            dst.set_field::<f32>(g_id, g as f32).ok();
            dst.set_field::<f32>(b_id, b as f32).ok();
        }
        SettingKind::Group { .. } => {
            let mut v: c_int = 0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<bool>(&desc.id, v != 0).ok();
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Pixel depth helpers
// ---------------------------------------------------------------------------

pub const DEPTH_BYTE: usize = 4;
pub const DEPTH_SHORT: usize = 8;
pub const DEPTH_FLOAT: usize = 16;

pub unsafe fn detect_pixel_depth(
    suites: &SuiteCache,
    image_props: OfxPropertySetHandle,
) -> Option<usize> {
    let pgs = suites.property_suite.propGetString?;
    let mut depth_ptr: *mut c_char = ptr::null_mut();
    pgs(
        image_props,
        kOfxImageEffectPropPixelDepth.as_ptr(),
        0,
        &mut depth_ptr,
    )
    .ofx_ok()
    .ok()?;
    let s = CStr::from_ptr(depth_ptr);
    if s == kOfxBitDepthFloat {
        Some(DEPTH_FLOAT)
    } else if s == kOfxBitDepthShort {
        Some(DEPTH_SHORT)
    } else if s == kOfxBitDepthByte {
        Some(DEPTH_BYTE)
    } else {
        None
    }
}

pub unsafe fn copy_source_to_u8(
    sp: *const c_void,
    src_stride: usize,
    dst: &mut [u8],
    width: usize,
    height: usize,
    row_bytes_u8: usize,
    depth: usize,
) {
    // Safety guard: if stride is insufficient, abort to avoid UB from copy_nonoverlapping
    if src_stride < row_bytes_u8 {
        return;
    }
    match depth {
        4 => {
            for y in 0..height {
                ptr::copy_nonoverlapping(
                    (sp as *const u8).add(y * src_stride),
                    dst.as_mut_ptr().add((height - 1 - y) * row_bytes_u8),
                    row_bytes_u8,
                );
            }
        }
        8 => {
            for y in 0..height {
                let host_row = (sp as *const u8).add(y * src_stride) as *const u16;
                let u8_row = dst.as_mut_ptr().add((height - 1 - y) * row_bytes_u8);
                for x in 0..(width * 4) {
                    let v = *host_row.add(x) as u32;
                    *u8_row.add(x) = ((v * 255 + 32767) / 65535) as u8;
                }
            }
        }
        _ => {
            for y in 0..height {
                let host_row = (sp as *const u8).add(y * src_stride) as *const f32;
                let u8_row = dst.as_mut_ptr().add((height - 1 - y) * row_bytes_u8);
                for x in 0..(width * 4) {
                    let v = *host_row.add(x);
                    *u8_row.add(x) = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                }
            }
        }
    }
}

pub unsafe fn copy_u8_to_output(
    src: &[u8],
    dp: *mut c_void,
    dst_stride: usize,
    width: usize,
    height: usize,
    row_bytes_u8: usize,
    depth: usize,
) {
    // Safety guard: if stride is insufficient, abort to avoid UB from copy_nonoverlapping
    if dst_stride < row_bytes_u8 {
        return;
    }
    match depth {
        4 => {
            for y in 0..height {
                ptr::copy_nonoverlapping(
                    src.as_ptr().add(y * row_bytes_u8),
                    (dp as *mut u8).add((height - 1 - y) * dst_stride),
                    row_bytes_u8,
                );
            }
        }
        8 => {
            for y in 0..height {
                let u8_row = src.as_ptr().add(y * row_bytes_u8);
                let host_row = (dp as *mut u8).add((height - 1 - y) * dst_stride) as *mut u16;
                for x in 0..(width * 4) {
                    let v = *u8_row.add(x) as u16;
                    *host_row.add(x) = (v << 8) | v;
                }
            }
        }
        _ => {
            for y in 0..height {
                let u8_row = src.as_ptr().add(y * row_bytes_u8);
                let host_row = (dp as *mut u8).add((height - 1 - y) * dst_stride) as *mut f32;
                for x in 0..(width * 4) {
                    *host_row.add(x) = *u8_row.add(x) as f32 * RECIP_255;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Common action helpers
// ---------------------------------------------------------------------------

pub unsafe fn action_load_common(suites: &SuiteCache) -> OfxResult<()> {
    let pg = suites
        .property_suite
        .propGetInt
        .ok_or(OfxStat::kOfxStatFailed)?;
    let mut v: c_int = 0;
    pg(
        suites.host_info.host as *const _ as _,
        kOfxImageEffectPropSupportsMultipleClipDepths.as_ptr(),
        0,
        &mut v,
    )
    .ofx_ok()?;
    suites
        .supports_multiple_clip_depths
        .store(v != 0, Ordering::Release);

    // Eagerly warm up the shared wgpu device so the first render
    // doesn't stall. If init fails, do NOT permanently blacklist —
    // render-time will try again (the GPU may be available in a
    // different thread context than kOfxActionLoad).
    #[cfg(feature = "gpu")]
    {
        zzzfx::gpu::try_init_shared_device();
    }

    Ok(())
}

pub unsafe fn action_get_clip_preferences_common(
    suites: &SuiteCache,
    out_args: OfxPropertySetHandle,
    frame_varying: i32,
    pre_multiplication: &CStr,
) -> OfxResult<()> {
    let pi = suites
        .property_suite
        .propSetInt
        .ok_or(OfxStat::kOfxStatFailed)?;
    let ps = suites
        .property_suite
        .propSetString
        .ok_or(OfxStat::kOfxStatFailed)?;
    pi(out_args, kOfxImageEffectFrameVarying.as_ptr(), 0, frame_varying).ofx_ok()?;
    ps(
        out_args,
        kOfxImageEffectPropPreMultiplication.as_ptr(),
        0,
        pre_multiplication.as_ptr(),
    )
    .ofx_ok()?;
    Ok(())
}

pub unsafe fn action_get_regions_of_interest_common(
    suites: &SuiteCache,
    effect: OfxImageEffectHandle,
    in_args: OfxPropertySetHandle,
    out_args: OfxPropertySetHandle,
) -> OfxResult<()> {
    let pg = suites
        .property_suite
        .propGetDouble
        .ok_or(OfxStat::kOfxStatFailed)?;
    let psn = suites
        .property_suite
        .propSetDoubleN
        .ok_or(OfxStat::kOfxStatFailed)?;
    let cgh = suites
        .image_effect_suite
        .clipGetHandle
        .ok_or(OfxStat::kOfxStatFailed)?;
    let crod = suites
        .image_effect_suite
        .clipGetRegionOfDefinition
        .ok_or(OfxStat::kOfxStatFailed)?;

    let mut sc: OfxImageClipHandle = ptr::null_mut();
    cgh(
        effect,
        c"Source".as_ptr(),
        &mut sc,
        ptr::null_mut(),
    )
    .ofx_ok()?;
    let mut rod = OfxRectD {
        x1: 0.0,
        x2: 0.0,
        y1: 0.0,
        y2: 0.0,
    };
    let mut t: OfxTime = 0.0;
    pg(in_args, kOfxPropTime.as_ptr(), 0, &mut t).ofx_ok()?;
    crod(sc, t, &mut rod).ofx_ok()?;

    psn(
        out_args,
        c"OfxImageClipPropRoI_Source".as_ptr(),
        4,
        ptr::addr_of_mut!(rod) as *mut _,
    )
    .ofx_ok()?;
    Ok(())
}

/// GetRegionOfDefinition for Generator effects (no Source clip).
/// Returns the Output clip's region which equals the project canvas size.
pub unsafe fn action_get_region_of_definition_generator(
    suites: &SuiteCache,
    effect: OfxImageEffectHandle,
    in_args: OfxPropertySetHandle,
    out_args: OfxPropertySetHandle,
) -> OfxResult<()> {
    let pg = suites
        .property_suite
        .propGetDouble
        .ok_or(OfxStat::kOfxStatFailed)?;
    let psn = suites
        .property_suite
        .propSetDoubleN
        .ok_or(OfxStat::kOfxStatFailed)?;
    let cgh = suites
        .image_effect_suite
        .clipGetHandle
        .ok_or(OfxStat::kOfxStatFailed)?;
    let crod = suites
        .image_effect_suite
        .clipGetRegionOfDefinition
        .ok_or(OfxStat::kOfxStatFailed)?;

    let mut oc: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"Output".as_ptr(), &mut oc, ptr::null_mut()).ofx_ok()?;
    let mut rod = OfxRectD {
        x1: 0.0,
        x2: 0.0,
        y1: 0.0,
        y2: 0.0,
    };
    let mut t: OfxTime = 0.0;
    pg(in_args, kOfxPropTime.as_ptr(), 0, &mut t).ofx_ok()?;
    crod(oc, t, &mut rod).ofx_ok()?;

    psn(
        out_args,
        c"OfxImageEffectPropRegionOfDefinition".as_ptr(),
        4,
        ptr::addr_of_mut!(rod) as *mut _,
    )
    .ofx_ok()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared render helpers — functions consolidated from per-effect files
// ---------------------------------------------------------------------------

/// Fill a u8 RGBA buffer with a solid color ([f32; 4] straight alpha, 0..1 range).
pub fn fill_buf_bg(buf: &mut [u8], bg: [f32; 4]) {
    let rb = (bg[0] * 255.0).round() as u8;
    let gb = (bg[1] * 255.0).round() as u8;
    let bb = (bg[2] * 255.0).round() as u8;
    let ab = (bg[3] * 255.0).round() as u8;
    for chunk in buf.chunks_exact_mut(4) {
        chunk[0] = rb;
        chunk[1] = gb;
        chunk[2] = bb;
        chunk[3] = ab;
    }
}

/// RAII guard that calls clipReleaseImage on drop.
pub struct ClipImageGuard {
    pub img: OfxPropertySetHandle,
    pub release_fn: unsafe extern "C" fn(OfxPropertySetHandle) -> OfxStatus,
}

impl Drop for ClipImageGuard {
    fn drop(&mut self) {
        if !self.img.is_null() {
            unsafe {
                let _ = (self.release_fn)(self.img);
            }
        }
    }
}

/// Detect the number of color components (3=RGB, 4=RGBA) from an image property set.
/// Defaults to 4 if the property is unavailable.
pub unsafe fn detect_num_components(
    suites: &SuiteCache,
    image_props: OfxPropertySetHandle,
) -> usize {
    let mut comp_ptr: *mut c_char = ptr::null_mut();
    if suites.property_suite.propGetString
        .and_then(|pgs| pgs(image_props, kOfxImageEffectPropComponents.as_ptr(), 0, &mut comp_ptr).ofx_ok().ok())
        .is_some()
        && !comp_ptr.is_null()
    {
        if CStr::from_ptr(comp_ptr) == kOfxImageComponentRGB { 3 } else { 4 }
    } else {
        4
    }
}

/// Pre-multiply alpha in-place on a u8 RGBA buffer (straight → premultiplied).
pub fn premultiply_alpha(buf: &mut [u8]) {
    for pixel in buf.chunks_exact_mut(4) {
        let a = pixel[3] as f32 / 255.0;
        pixel[0] = (pixel[0] as f32 * a).round() as u8;
        pixel[1] = (pixel[1] as f32 * a).round() as u8;
        pixel[2] = (pixel[2] as f32 * a).round() as u8;
    }
}

/// Read pixel bounds from an image property set and validate dimensions.
/// Returns (width, height, left, bottom, right, top).
pub unsafe fn read_render_bounds(
    suites: &SuiteCache,
    image_props: OfxPropertySetHandle,
) -> OfxResult<(usize, usize, c_int, c_int, c_int, c_int)> {
    let pgi = suites.property_suite.propGetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let mut l: c_int = 0;
    let mut b: c_int = 0;
    let mut r: c_int = 0;
    let mut t: c_int = 0;
    pgi(image_props, kOfxImagePropBounds.as_ptr(), 0, &mut l).ofx_ok()?;
    pgi(image_props, kOfxImagePropBounds.as_ptr(), 1, &mut b).ofx_ok()?;
    pgi(image_props, kOfxImagePropBounds.as_ptr(), 2, &mut r).ofx_ok()?;
    pgi(image_props, kOfxImagePropBounds.as_ptr(), 3, &mut t).ofx_ok()?;
    let width = (r - l).max(0) as usize;
    let height = (t - b).max(0) as usize;
    if width == 0 || height == 0 { return Err(OfxStat::kOfxStatFailed); }
    if width > 16384 || height > 16384 { return Err(OfxStat::kOfxStatErrFormat); }
    Ok((width, height, l, b, r, t))
}

/// Copy a u8 RGBA source buffer to host output, respecting num_components (3 or 4).
pub unsafe fn copy_u8_to_output_nc(
    src: &[u8],
    src_row_bytes: usize,
    dp: *mut c_void,
    dst_stride: usize,
    width: usize,
    height: usize,
    num_components: usize,
    depth: usize,
) {
    if dst_stride < width * num_components {
        return;
    }
    match depth {
        4 => {
            for y in 0..height {
                let src_row = (height - 1 - y) * src_row_bytes;
                let host_row = (dp as *mut u8).add(y * dst_stride);
                for x in 0..width {
                    let si = src_row + x * 4;
                    host_row.add(x * num_components)
                        .copy_from_nonoverlapping(src.as_ptr().add(si), num_components);
                }
            }
        }
        8 => {
            for y in 0..height {
                let src_row = (height - 1 - y) * src_row_bytes;
                let host_row = (dp as *mut u8).add(y * dst_stride) as *mut u16;
                for x in 0..width {
                    for c in 0..num_components {
                        let v = *src.as_ptr().add(src_row + x * 4 + c) as u16;
                        *host_row.add(x * num_components + c) = (v << 8) | v;
                    }
                }
            }
        }
        _ => {
            for y in 0..height {
                let src_row = (height - 1 - y) * src_row_bytes;
                let host_row = (dp as *mut u8).add(y * dst_stride) as *mut f32;
                for x in 0..width {
                    for c in 0..num_components {
                        let v = *src.as_ptr().add(src_row + x * 4 + c) as f32 * RECIP_255;
                        *host_row.add(x * num_components + c) = v;
                    }
                }
            }
        }
    }
}

/// Define a native OFX Double2D parameter (X/Y pair, range constrained).
pub unsafe fn define_native_double2d(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    name: &CStr,
    label: &CStr,
    hint: &CStr,
    default_x: f64,
    default_y: f64,
    min: f64,
    max: f64,
    parent: &CStr,
) -> OfxResult<()> {
    let pdef = suites.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let pd = suites.property_suite.propSetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = suites.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let mut pp: OfxPropertySetHandle = ptr::null_mut();
    pdef(param_set, kOfxParamTypeDouble2D.as_ptr(), name.as_ptr(), &mut pp).ofx_ok()?;
    ps(pp, kOfxPropLabel.as_ptr(), 0, label.as_ptr()).ofx_ok()?;
    ps(pp, kOfxParamPropHint.as_ptr(), 0, hint.as_ptr()).ofx_ok()?;
    pd(pp, kOfxParamPropDefault.as_ptr(), 0, default_x).ofx_ok()?;
    pd(pp, kOfxParamPropDefault.as_ptr(), 1, default_y).ofx_ok()?;
    pd(pp, kOfxParamPropMin.as_ptr(), 0, min).ofx_ok()?;
    pd(pp, kOfxParamPropMin.as_ptr(), 1, min).ofx_ok()?;
    pd(pp, kOfxParamPropMax.as_ptr(), 0, max).ofx_ok()?;
    pd(pp, kOfxParamPropMax.as_ptr(), 1, max).ofx_ok()?;
    ps(pp, kOfxParamPropParent.as_ptr(), 0, parent.as_ptr()).ofx_ok()?;
    Ok(())
}
