use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_int, c_void},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use zzzfx_core::settings::TrKey;
use zzzfx_core::settings::{
    EnumValue, SettingDescriptor, SettingID, SettingKind, Settings, SettingsList,
};

use crate::bindings::*;

// SAFETY
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
    #[allow(dead_code)]
    pub host_info: HostInfo,
    pub property_suite: &'static OfxPropertySuiteV1,
    pub image_effect_suite: &'static OfxImageEffectSuiteV1,
    #[allow(dead_code)]
    pub memory_suite: &'static OfxMemorySuiteV1,
    pub parameter_suite: &'static OfxParameterSuiteV1,
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
        let memory_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _,
            kOfxMemorySuite.as_ptr(),
            1,
        ) as *const OfxMemorySuiteV1;
        let parameter_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _,
            kOfxParameterSuite.as_ptr(),
            1,
        ) as *const OfxParameterSuiteV1;

        Ok(Self {
            host_info,
            property_suite: property_suite
                .as_ref()
                .ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            image_effect_suite: image_effect_suite
                .as_ref()
                .ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            memory_suite: memory_suite
                .as_ref()
                .ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            parameter_suite: parameter_suite
                .as_ref()
                .ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
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
    for descriptor in settings_list.all_descriptors() {
        let id = &descriptor.id;
        let id_str = CString::new(descriptor.id.name).unwrap();
        let label = CString::new(zzzfx_core::i18n::tr(descriptor.label_key)).unwrap();
        let description = descriptor
            .description_key
            .map(|k| CString::new(zzzfx_core::i18n::tr(k)).unwrap());
        let group_name = if let SettingKind::Group { .. } = descriptor.kind {
            Some(CString::new(format!("{}_group", descriptor.id.name)).unwrap())
        } else {
            None
        };
        strings.insert(id.clone(), (id_str, label, description, group_name));
        if let SettingKind::Enumeration { options } = &descriptor.kind {
            for item in options {
                let lbl = CString::new(zzzfx_core::i18n::tr(item.label_key)).unwrap();
                menu_item_strings.insert(
                    (id.clone(), item.index),
                    (lbl, item.description_key.map(|k| CString::new(zzzfx_core::i18n::tr(k)).unwrap())),
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

    let ds = strings.get(&descriptor.id).unwrap();
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
                    .unwrap();
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
        SettingKind::Group { children } => {
            let dv = default_settings
                .get_field::<bool>(&descriptor.id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            let gnc: &CStr = ds.3.as_ref().expect("group name").as_c_str();
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
            let enabled_label = CString::new(zzzfx_core::i18n::tr(TrKey::CommonEnabled)).unwrap();
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
    let ds = strings.get(&desc.id).unwrap();
    let id_cstr = ds.0.as_c_str();

    let mut p: OfxParamHandle = ptr::null_mut();
    pgh(param_set, id_cstr.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;

    match &desc.kind {
        SettingKind::Enumeration { options } => {
            let mut idx: c_int = 0;
            pgv(p, time, &mut idx).ofx_ok()?;
            if idx >= 0 && (idx as usize) < options.len() {
                dst.set_field::<EnumValue>(&desc.id, EnumValue(options[idx as usize].index))
                    .unwrap();
            }
        }
        SettingKind::Percentage { .. } => {
            let mut v: f64 = 0.0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<f32>(&desc.id, v.clamp(0.0, 1.0) as f32)
                .unwrap();
        }
        SettingKind::FloatRange { range, .. } => {
            let mut v: f64 = 0.0;
            pgv(p, time, &mut v).ofx_ok()?;
            let lo = *range.start() as f64;
            let hi = *range.end() as f64;
            dst.set_field::<f32>(&desc.id, v.clamp(lo, hi) as f32)
                .unwrap();
        }
        SettingKind::IntRange { range } => {
            let mut v: c_int = 0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<i32>(&desc.id, v.clamp(*range.start(), *range.end()))
                .unwrap();
        }
        SettingKind::Boolean => {
            let mut v: c_int = 0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<bool>(&desc.id, v != 0).unwrap();
        }
        SettingKind::Group { .. } => {}
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
    match depth {
        4 => {
            for y in 0..height {
                ptr::copy_nonoverlapping(
                    (sp as *const u8).add(y * src_stride),
                    dst.as_mut_ptr().add(y * row_bytes_u8),
                    row_bytes_u8,
                );
            }
        }
        8 => {
            for y in 0..height {
                let host_row = (sp as *const u8).add(y * src_stride) as *const u16;
                let u8_row = dst.as_mut_ptr().add(y * row_bytes_u8);
                for x in 0..(width * 4) {
                    let v = *host_row.add(x) as u32;
                    *u8_row.add(x) = ((v * 255 + 32767) / 65535) as u8;
                }
            }
        }
        _ => {
            for y in 0..height {
                let host_row = (sp as *const u8).add(y * src_stride) as *const f32;
                let u8_row = dst.as_mut_ptr().add(y * row_bytes_u8);
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
    match depth {
        4 => {
            for y in 0..height {
                ptr::copy_nonoverlapping(
                    src.as_ptr().add(y * row_bytes_u8),
                    (dp as *mut u8).add(y * dst_stride),
                    row_bytes_u8,
                );
            }
        }
        8 => {
            for y in 0..height {
                let u8_row = src.as_ptr().add(y * row_bytes_u8);
                let host_row = (dp as *mut u8).add(y * dst_stride) as *mut u16;
                for x in 0..(width * 4) {
                    let v = *u8_row.add(x) as u16;
                    *host_row.add(x) = (v << 8) | v;
                }
            }
        }
        _ => {
            for y in 0..height {
                let u8_row = src.as_ptr().add(y * row_bytes_u8);
                let host_row = (dp as *mut u8).add(y * dst_stride) as *mut f32;
                for x in 0..(width * 4) {
                    *host_row.add(x) = *u8_row.add(x) as f32 / 255.0;
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
