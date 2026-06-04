use std::{
    ffi::{CStr, c_char, c_int, c_void},
    ptr,
    sync::OnceLock,
};

use zzzfx::settings::TrKey;
use zzzfx::{
    Stroke, StrokeFullSettings,
    settings::{Settings, SettingsList},
};

use crate::bindings::*;
use crate::i18n;
use crate::shared::{
    HostInfo, SuiteCache, StringCache, MenuItemCache,
    build_string_cache, define_single_param, read_generic_param,
    copy_source_to_u8, copy_u8_to_output, detect_pixel_depth,
    action_load_common, action_get_clip_preferences_common,
    action_get_regions_of_interest_common,
};

// ---------------------------------------------------------------------------
// Native OFX parameter names (Double2D position params only — colors use generic)
// ---------------------------------------------------------------------------

const GRADIENT_START_POS_PARAM: &CStr = c"gradient_start_pos";
const GRADIENT_END_POS_PARAM: &CStr = c"gradient_end_pos";
const GRADIENT_GROUP_PARAM: &CStr = c"gradient_group";

fn is_position_component(name: &str) -> bool {
    matches!(name, "gradient_start_x" | "gradient_start_y" | "gradient_end_x" | "gradient_end_y")
}

// ---------------------------------------------------------------------------
// Per-effect globals
// ---------------------------------------------------------------------------

static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();

struct EffectData {
    suites: SuiteCache,
    settings_list: SettingsList<StrokeFullSettings>,
    strings: StringCache<StrokeFullSettings>,
    menu_item_strings: MenuItemCache<StrokeFullSettings>,
}

static EFFECT_DATA: OnceLock<EffectData> = OnceLock::new();

fn data() -> OfxResult<&'static EffectData> {
    EFFECT_DATA.get().ok_or(OfxStat::kOfxStatFailed)
}

// ---------------------------------------------------------------------------
// Plugin info accessor
// ---------------------------------------------------------------------------

pub fn get_plugin() -> *const OfxPlugin {
    let pi = PLUGIN_INFO.get_or_init(|| OfxPlugin {
        pluginApi: kOfxImageEffectPluginApi.as_ptr(),
        apiVersion: 1,
        pluginIdentifier: c"io.github.zzzEffect:Stroke".as_ptr(),
        pluginVersionMajor: 0,
        pluginVersionMinor: 1,
        setHost: Some(set_host_info),
        mainEntry: Some(main_entry),
    });
    pi as *const _
}

// ---------------------------------------------------------------------------
// set_host_info
// ---------------------------------------------------------------------------

unsafe fn set_host_info_inner(host: *mut OfxHost) -> OfxResult<()> {
    let hs = host.as_ref().ok_or(OfxStat::kOfxStatFailed)?;
    let h = hs.host.as_ref().ok_or(OfxStat::kOfxStatFailed)?;
    let fs = hs.fetchSuite.ok_or(OfxStat::kOfxStatFailed)?;

    let host_info = HostInfo { host: h, fetch_suite: fs };
    let suites = SuiteCache::new(host_info)?;
    let settings_list = SettingsList::<StrokeFullSettings>::new();
    i18n::set_lang(i18n::detect_system_lang());
    let (strings, menu_item_strings) = build_string_cache(&settings_list);

    EFFECT_DATA.get_or_init(|| EffectData { suites, settings_list, strings, menu_item_strings });
    Ok(())
}

unsafe extern "C" fn set_host_info(host: *mut OfxHost) {
    let _ = set_host_info_inner(host);
}

// ---------------------------------------------------------------------------
// main_entry
// ---------------------------------------------------------------------------

unsafe extern "C" fn main_entry(
    action: *const c_char,
    handle: *const c_void,
    inArgs: OfxPropertySetHandle,
    outArgs: OfxPropertySetHandle,
) -> OfxStatus {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        main_entry_inner(action, handle, inArgs, outArgs)
    }));
    match result {
        Ok(status) => status,
        Err(_) => OfxStat::kOfxStatFailed,
    }
}

unsafe fn main_entry_inner(
    action: *const c_char,
    handle: *const c_void,
    inArgs: OfxPropertySetHandle,
    outArgs: OfxPropertySetHandle,
) -> OfxStatus {
    if action.is_null() { return OfxStat::kOfxStatFailed; }
    let effect = handle as OfxImageEffectHandle;
    let action = CStr::from_ptr(action);
    let r: OfxResult<()> = if action == kOfxActionLoad {
        action_load()
    } else if action == kOfxActionDescribe {
        action_describe(effect)
    } else if action == kOfxImageEffectActionDescribeInContext {
        action_describe_in_context(effect)
    } else if action == kOfxImageEffectActionGetRegionsOfInterest {
        action_get_regions_of_interest(effect, inArgs, outArgs)
    } else if action == kOfxImageEffectActionGetClipPreferences {
        action_get_clip_preferences(outArgs)
    } else if action == kOfxActionCreateInstance || action == kOfxActionDestroyInstance {
        Ok(())
    } else if action == kOfxActionInstanceChanged {
        action_instance_changed(effect, inArgs)
    } else if action == kOfxImageEffectActionIsIdentity {
        action_is_identity(effect, inArgs, outArgs)
    } else if action == kOfxImageEffectActionRender {
        action_render(effect, inArgs)
    } else {
        Err(OfxStat::kOfxStatReplyDefault)
    };
    match r {
        Ok(()) => OfxStat::kOfxStatOK,
        Err(e) => e,
    }
}

// ---------------------------------------------------------------------------
// Action handlers
// ---------------------------------------------------------------------------

unsafe fn action_load() -> OfxResult<()> {
    action_load_common(&data()?.suites)
}

unsafe fn action_describe(desc: OfxImageEffectHandle) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let mut ep: OfxPropertySetHandle = ptr::null_mut();
    (su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?)(desc, &mut ep).ofx_ok()?;
    let ps = su.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pi = su.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;

    ps(ep, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::EffectStrokeName).as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginPropGrouping.as_ptr(), 0, c"zzzFX".as_ptr()).ofx_ok()?;
    ps(ep, kOfxPropPluginDescription.as_ptr(), 0, i18n::tr_cstr(TrKey::EffectStrokeDesc).as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedContexts.as_ptr(), 0, kOfxImageEffectContextFilter.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedContexts.as_ptr(), 1, kOfxImageEffectContextGeneral.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 0, kOfxBitDepthFloat.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 1, kOfxBitDepthShort.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 2, kOfxBitDepthByte.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginRenderThreadSafety.as_ptr(), 0, kOfxImageEffectRenderFullySafe.as_ptr()).ofx_ok()?;
    pi(ep, kOfxImageEffectPluginPropHostFrameThreading.as_ptr(), 0, 0).ofx_ok()?;
    pi(ep, kOfxImageEffectPropSupportsTiles.as_ptr(), 0, 0).ofx_ok()?;
    Ok(())
}

unsafe fn action_describe_in_context(desc: OfxImageEffectHandle) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let cd = su.image_effect_suite.clipDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let gp = su.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = su.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pd = su.property_suite.propSetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let pi = su.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let pdef = su.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let defaults = StrokeFullSettings::default();

    // --- Output / Source clips ---
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    cd(desc, c"Output".as_ptr(), &mut props).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 0, kOfxImageComponentRGBA.as_ptr()).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 1, kOfxImageComponentRGB.as_ptr()).ofx_ok()?;
    cd(desc, c"Source".as_ptr(), &mut props).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 0, kOfxImageComponentRGBA.as_ptr()).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 1, kOfxImageComponentRGB.as_ptr()).ofx_ok()?;

    // --- Parameter set ---
    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gp(desc, &mut param_set).ofx_ok()?;

    // --- Generic loop: define_single_param handles all types including ColorRGBA ---
    for desc in d.settings_list.setting_descriptors.iter() {
        if is_position_component(desc.id.name) { continue; }
        if let zzzfx::settings::SettingKind::Group { .. } = &desc.kind {
            // Manual gradient group with Double2D position params for better UX
            let ds = d.strings.get(&desc.id).unwrap();
            let id_cstr = ds.0.as_c_str();
            let dv = defaults.get_field::<bool>(&desc.id).map_err(|_| OfxStat::kOfxStatFailed)?;
            let mut gp_h: OfxPropertySetHandle = ptr::null_mut();
            pdef(param_set, kOfxParamTypeGroup.as_ptr(), GRADIENT_GROUP_PARAM.as_ptr(), &mut gp_h).ofx_ok()?;
            ps(gp_h, kOfxPropLabel.as_ptr(), 0, ds.1.as_ptr()).ofx_ok()?;
            if let Some(desc_text) = ds.2.as_deref() {
                ps(gp_h, kOfxParamPropHint.as_ptr(), 0, desc_text.as_ptr()).ofx_ok()?;
            }
            let mut cb: OfxPropertySetHandle = ptr::null_mut();
            pdef(param_set, kOfxParamTypeBoolean.as_ptr(), id_cstr.as_ptr(), &mut cb).ofx_ok()?;
            let enabled_label = std::ffi::CString::new(i18n::tr(TrKey::CommonEnabled))
                .unwrap_or_else(|_| std::ffi::CString::new("").unwrap());
            ps(cb, kOfxPropLabel.as_ptr(), 0, enabled_label.as_ptr()).ofx_ok()?;
            pi(cb, kOfxParamPropDefault.as_ptr(), 0, dv as i32).ofx_ok()?;
            ps(cb, kOfxParamPropParent.as_ptr(), 0, GRADIENT_GROUP_PARAM.as_ptr()).ofx_ok()?;
            pi(cb, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;

            // Children: handle ColorRGBA children via define_single_param, position children as Double2D
            if let zzzfx::settings::SettingKind::Group { children } = &desc.kind {
                for child in children.iter() {
                    if is_position_component(child.id.name) { continue; }
                    define_single_param(su, param_set, child, &defaults, GRADIENT_GROUP_PARAM, &d.strings, &d.menu_item_strings)?;
                }
            }

            // Native Double2D: Gradient Start
            {
                let mut pp: OfxPropertySetHandle = ptr::null_mut();
                pdef(param_set, kOfxParamTypeDouble2D.as_ptr(), GRADIENT_START_POS_PARAM.as_ptr(), &mut pp).ofx_ok()?;
                ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeGradientStart).as_ptr()).ofx_ok()?;
                ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeGradientStartHint).as_ptr()).ofx_ok()?;
                ps(pp, kOfxParamPropParent.as_ptr(), 0, GRADIENT_GROUP_PARAM.as_ptr()).ofx_ok()?;
                pd(pp, kOfxParamPropDefault.as_ptr(), 0, 0.0).ofx_ok()?;
                pd(pp, kOfxParamPropDefault.as_ptr(), 1, 0.0).ofx_ok()?;
            }
            // Native Double2D: Gradient End
            {
                let mut pp: OfxPropertySetHandle = ptr::null_mut();
                pdef(param_set, kOfxParamTypeDouble2D.as_ptr(), GRADIENT_END_POS_PARAM.as_ptr(), &mut pp).ofx_ok()?;
                ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeGradientEnd).as_ptr()).ofx_ok()?;
                ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeGradientEndHint).as_ptr()).ofx_ok()?;
                ps(pp, kOfxParamPropParent.as_ptr(), 0, GRADIENT_GROUP_PARAM.as_ptr()).ofx_ok()?;
                pd(pp, kOfxParamPropDefault.as_ptr(), 0, 1.0).ofx_ok()?;
                pd(pp, kOfxParamPropDefault.as_ptr(), 1, 1.0).ofx_ok()?;
            }
        } else {
            define_single_param(su, param_set, desc, &defaults, c"", &d.strings, &d.menu_item_strings)?;
        }
    }

    Ok(())
}

unsafe fn action_get_regions_of_interest(
    effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
    outArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    action_get_regions_of_interest_common(&data()?.suites, effect, inArgs, outArgs)
}

unsafe fn action_get_clip_preferences(outArgs: OfxPropertySetHandle) -> OfxResult<()> {
    action_get_clip_preferences_common(&data()?.suites, outArgs, 0, kOfxImageOpaque)
}

unsafe fn action_instance_changed(_effect: OfxImageEffectHandle, inArgs: OfxPropertySetHandle) -> OfxResult<()> {
    let d = data()?;
    let pg = d.suites.property_suite.propGetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let mut r: c_int = 0;
    pg(inArgs, kOfxPropChangeReason.as_ptr(), 0, &mut r).ofx_ok()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// IsIdentity
// ---------------------------------------------------------------------------

unsafe fn action_is_identity(
    effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
    outArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let pss = su.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pgd = su.property_suite.propGetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let gps = su.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;

    let mut time: OfxTime = 0.0;
    pgd(inArgs, kOfxPropTime.as_ptr(), 0, &mut time).ofx_ok()?;

    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gps(effect, &mut param_set).ofx_ok()?;

    let mut settings = StrokeFullSettings::default();
    apply_params(param_set, time, &mut settings)?;
    let stroke: Stroke = (&settings).into();

    if stroke.is_identity() {
        pss(outArgs, kOfxPropName.as_ptr(), 0, c"Source".as_ptr()).ofx_ok()?;
        Ok(())
    } else {
        Err(OfxStat::kOfxStatReplyDefault)
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

unsafe fn action_render(effect: OfxImageEffectHandle, inArgs: OfxPropertySetHandle) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;

    let cgh = su.image_effect_suite.clipGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let cgi = su.image_effect_suite.clipGetImage.ok_or(OfxStat::kOfxStatFailed)?;
    let cri = su.image_effect_suite.clipReleaseImage.ok_or(OfxStat::kOfxStatFailed)?;
    let pgp = su.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    let pgi = su.property_suite.propGetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let pgd = su.property_suite.propGetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let gps = su.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;

    let mut time: OfxTime = 0.0;
    pgd(inArgs, kOfxPropTime.as_ptr(), 0, &mut time).ofx_ok()?;

    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gps(effect, &mut param_set).ofx_ok()?;

    let mut settings = StrokeFullSettings::default();
    apply_params(param_set, time, &mut settings)?;
    let stroke: Stroke = (&settings).into();

    let mut sc: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"Source".as_ptr(), &mut sc, ptr::null_mut()).ofx_ok()?;
    let mut dc: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"Output".as_ptr(), &mut dc, ptr::null_mut()).ofx_ok()?;

    let mut si: OfxPropertySetHandle = ptr::null_mut();
    cgi(sc, time, ptr::null(), &mut si).ofx_ok()?;
    let mut di: OfxPropertySetHandle = ptr::null_mut();
    cgi(dc, time, ptr::null(), &mut di).ofx_ok()?;

    let mut sp: *mut c_void = ptr::null_mut();
    pgp(si, kOfxImagePropData.as_ptr(), 0, &mut sp).ofx_ok()?;
    let mut dp: *mut c_void = ptr::null_mut();
    pgp(di, kOfxImagePropData.as_ptr(), 0, &mut dp).ofx_ok()?;

    let mut srb: c_int = 0; let mut drb: c_int = 0;
    pgi(si, kOfxImagePropRowBytes.as_ptr(), 0, &mut srb).ofx_ok()?;
    pgi(di, kOfxImagePropRowBytes.as_ptr(), 0, &mut drb).ofx_ok()?;

    let mut l: c_int = 0; let mut b: c_int = 0;
    let mut r: c_int = 0; let mut t: c_int = 0;
    pgi(si, kOfxImagePropBounds.as_ptr(), 0, &mut l).ofx_ok()?;
    pgi(si, kOfxImagePropBounds.as_ptr(), 1, &mut b).ofx_ok()?;
    pgi(si, kOfxImagePropBounds.as_ptr(), 2, &mut r).ofx_ok()?;
    pgi(si, kOfxImagePropBounds.as_ptr(), 3, &mut t).ofx_ok()?;

    let width = (r - l).max(0) as usize;
    let height = (t - b).max(0) as usize;
    if width == 0 || height == 0 { return Err(OfxStat::kOfxStatFailed); }
    if width > 16384 || height > 16384 { return Err(OfxStat::kOfxStatErrFormat); }
    let s_stride = srb.max(0) as usize;
    let d_stride = drb.max(0) as usize;

    let depth = detect_pixel_depth(su, si).ok_or(OfxStat::kOfxStatErrFormat)?;
    let row_bytes_u8 = width * 4;
    let total_u8 = row_bytes_u8 * height;

    thread_local! {
        static RENDER_BUFS: std::cell::RefCell<(Vec<u8>, Vec<u8>)> =
            std::cell::RefCell::new((Vec::new(), Vec::new()));
    }
    RENDER_BUFS.with(|cell| {
        let (src_buf, dst_buf) = &mut *cell.borrow_mut();
        src_buf.resize(total_u8, 0);
        dst_buf.resize(total_u8, 0);
        copy_source_to_u8(sp, s_stride, src_buf, width, height, row_bytes_u8, depth);
        stroke.apply_effect(src_buf, dst_buf, width, height);
        copy_u8_to_output(dst_buf, dp, d_stride, width, height, row_bytes_u8, depth);
    });

    let _ = cri(si);
    let _ = cri(di);
    Ok(())
}

// ---------------------------------------------------------------------------
// Parameter reading
// ---------------------------------------------------------------------------

unsafe fn apply_params(
    param_set: OfxParamSetHandle,
    time: f64,
    dst: &mut StrokeFullSettings,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    // --- Generic loop: read all descriptors via read_generic_param ---
    for desc in d.settings_list.setting_descriptors.iter() {
        if is_position_component(desc.id.name) { continue; }
        if let zzzfx::settings::SettingKind::Group { children } = &desc.kind {
            // Read Group enable checkbox
            let ds = d.strings.get(&desc.id).unwrap();
            let id_cstr = ds.0.as_c_str();
            let mut p: OfxParamHandle = ptr::null_mut();
            pgh(param_set, id_cstr.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
            let mut v: c_int = 0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<bool>(&desc.id, v != 0).unwrap();

            // Read Group children via read_generic_param (handles ColorRGBA children)
            for child in children.iter() {
                if is_position_component(child.id.name) { continue; }
                read_generic_param(su, param_set, time, child, dst, &d.strings)?;
            }

            // Native Double2D: Gradient Start
            {
                let mut p: OfxParamHandle = ptr::null_mut();
                pgh(param_set, GRADIENT_START_POS_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
                let mut x: f64 = 0.0; let mut y: f64 = 0.0;
                pgv(p, time, &mut x, &mut y).ofx_ok()?;
                for child in children.iter() {
                    match child.id.name {
                        "gradient_start_x" => { dst.set_field::<f32>(&child.id, x as f32).unwrap(); }
                        "gradient_start_y" => { dst.set_field::<f32>(&child.id, y as f32).unwrap(); }
                        _ => {}
                    }
                }
            }
            // Native Double2D: Gradient End
            {
                let mut p: OfxParamHandle = ptr::null_mut();
                pgh(param_set, GRADIENT_END_POS_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
                let mut x: f64 = 0.0; let mut y: f64 = 0.0;
                pgv(p, time, &mut x, &mut y).ofx_ok()?;
                for child in children.iter() {
                    match child.id.name {
                        "gradient_end_x" => { dst.set_field::<f32>(&child.id, x as f32).unwrap(); }
                        "gradient_end_y" => { dst.set_field::<f32>(&child.id, y as f32).unwrap(); }
                        _ => {}
                    }
                }
            }
        } else {
            read_generic_param(su, param_set, time, desc, dst, &d.strings)?;
        }
    }

    Ok(())
}
