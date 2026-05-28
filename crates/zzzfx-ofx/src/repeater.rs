use std::{
    ffi::{CStr, c_char, c_int, c_void},
    ptr,
    sync::OnceLock,
};

use zzzfx_core::settings::TrKey;
use zzzfx_core::{
    CompositorLayer, ZzzRepeater, ZzzRepeaterFullSettings, ZzzStrokeBlendMode,
    settings::{SettingID, SettingKind, Settings, SettingsList},
};

use crate::bindings::*;
use crate::i18n;
use crate::shared::{
    self, HostInfo, SuiteCache, StringCache, MenuItemCache,
    build_string_cache, define_single_param, read_generic_param,
    copy_u8_to_output, detect_pixel_depth,
    action_load_common, action_get_clip_preferences_common,
    action_get_regions_of_interest_common, action_get_region_of_definition_common,
    get_project_canonical_region,
};

// ---------------------------------------------------------------------------
// Native OFX parameter names
// ---------------------------------------------------------------------------

const POSITION_PARAM: &CStr = c"position";

fn is_native_grouped_name(name: &str) -> bool {
    matches!(name, "position_x" | "position_y")
}

// ---------------------------------------------------------------------------
// Per-effect globals
// ---------------------------------------------------------------------------

static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();

struct EffectData {
    suites: SuiteCache,
    settings_list: SettingsList<ZzzRepeaterFullSettings>,
    strings: StringCache<ZzzRepeaterFullSettings>,
    menu_item_strings: MenuItemCache<ZzzRepeaterFullSettings>,
}

static EFFECT_DATA: OnceLock<EffectData> = OnceLock::new();

fn data() -> OfxResult<&'static EffectData> {
    EFFECT_DATA.get().ok_or(OfxStat::kOfxStatFailed)
}

// ---------------------------------------------------------------------------
// Plugin info accessor
// ---------------------------------------------------------------------------

pub fn get_plugin() -> *const OfxPlugin {
    std::panic::set_hook(Box::new(|info| { println!("{info:?}"); }));
    let pi = PLUGIN_INFO.get_or_init(|| OfxPlugin {
        pluginApi: kOfxImageEffectPluginApi.as_ptr(),
        apiVersion: 1,
        pluginIdentifier: c"com.example:zzzRepeater".as_ptr(),
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
    let settings_list = SettingsList::<ZzzRepeaterFullSettings>::new();
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
    } else if action == kOfxImageEffectActionGetRegionOfDefinition {
        action_get_region_of_definition(effect, outArgs)
    } else if action == kOfxImageEffectActionGetRegionsOfInterest {
        action_get_regions_of_interest(effect, inArgs, outArgs)
    } else if action == kOfxImageEffectActionGetClipPreferences {
        action_get_clip_preferences(outArgs)
    } else if action == kOfxImageEffectActionGetTimeDomain {
        action_get_time_domain(inArgs, outArgs)
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

    ps(ep, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::EffectRepeaterName).as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginPropGrouping.as_ptr(), 0, c"zzzFX".as_ptr()).ofx_ok()?;
    ps(ep, kOfxPropPluginDescription.as_ptr(), 0, i18n::tr_cstr(TrKey::EffectRepeaterDesc).as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedContexts.as_ptr(), 0, kOfxImageEffectContextFilter.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedContexts.as_ptr(), 1, kOfxImageEffectContextGeneral.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 0, kOfxBitDepthFloat.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 1, kOfxBitDepthShort.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 2, kOfxBitDepthByte.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginRenderThreadSafety.as_ptr(), 0, kOfxImageEffectRenderFullySafe.as_ptr()).ofx_ok()?;
    pi(ep, kOfxImageEffectPluginPropHostFrameThreading.as_ptr(), 0, 0).ofx_ok()?;
    pi(ep, kOfxImageEffectPropSupportsTiles.as_ptr(), 0, 1).ofx_ok()?;
    pi(ep, kOfxImageEffectPropSupportsMultiResolution.as_ptr(), 0, 1).ofx_ok()?;
    pi(ep, kOfxImageEffectPropTemporalClipAccess.as_ptr(), 0, 1).ofx_ok()?;
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
    let defaults = ZzzRepeaterFullSettings::default();

    // --- Output / Source clips ---
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    cd(desc, c"Output".as_ptr(), &mut props).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 0, kOfxImageComponentRGBA.as_ptr()).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 1, kOfxImageComponentRGB.as_ptr()).ofx_ok()?;
    cd(desc, c"Source".as_ptr(), &mut props).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 0, kOfxImageComponentRGBA.as_ptr()).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 1, kOfxImageComponentRGB.as_ptr()).ofx_ok()?;
    pi(props, kOfxImageEffectPropTemporalClipAccess.as_ptr(), 0, 1).ofx_ok()?;

    // --- Parameter set ---
    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gp(desc, &mut param_set).ofx_ok()?;

    // --- Block A: Params before Position ---
    for desc in d.settings_list.setting_descriptors.iter() {
        if desc.id.name == "position_x" { break; }
        if is_native_grouped_name(desc.id.name) { continue; }
        define_single_param(su, param_set, desc, &defaults, c"", &d.strings, &d.menu_item_strings)?;
    }

    // --- Native Double2D: Position ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeDouble2D.as_ptr(), POSITION_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativePosition).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(TrKey::NativePositionHint).as_ptr()).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 0.5).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 0.5).ofx_ok()?;
    }

    // --- Block C: Remaining params ---
    let mut after_position = false;
    for desc in d.settings_list.setting_descriptors.iter() {
        if desc.id.name == "position_x" { after_position = true; continue; }
        if !after_position || desc.id.name == "position_y" { continue; }
        if is_native_grouped_name(desc.id.name) { continue; }
        define_single_param(su, param_set, desc, &defaults, c"", &d.strings, &d.menu_item_strings)?;
    }

    Ok(())
}

unsafe fn action_get_region_of_definition(
    effect: OfxImageEffectHandle,
    outArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    action_get_region_of_definition_common(&data()?.suites, effect, outArgs)
}

unsafe fn action_get_regions_of_interest(
    effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
    outArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    action_get_regions_of_interest_common(&data()?.suites, effect, inArgs, outArgs)
}

unsafe fn action_get_time_domain(
    inArgs: OfxPropertySetHandle,
    outArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let pg = su.property_suite.propGetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let psn = su.property_suite.propSetDoubleN.ok_or(OfxStat::kOfxStatFailed)?;

    let mut t: OfxTime = 0.0;
    pg(inArgs, kOfxPropTime.as_ptr(), 0, &mut t).ofx_ok()?;

    let mut range = [0.0, t];
    psn(outArgs, c"OfxImageClipPropFrameRange_Source".as_ptr(), 2, range.as_mut_ptr() as *mut _).ofx_ok()?;
    Ok(())
}

unsafe fn action_get_clip_preferences(outArgs: OfxPropertySetHandle) -> OfxResult<()> {
    action_get_clip_preferences_common(&data()?.suites, outArgs, 1, kOfxImageOpaque)
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
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgnk = su.parameter_suite.paramGetNumKeys.ok_or(OfxStat::kOfxStatFailed)?;

    let mut time: OfxTime = 0.0;
    pgd(inArgs, kOfxPropTime.as_ptr(), 0, &mut time).ofx_ok()?;

    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gps(effect, &mut param_set).ofx_ok()?;

    let mut settings = ZzzRepeaterFullSettings::default();
    apply_params(param_set, time, &mut settings)?;

    let ds = d.strings.iter().find(|(k, _)| k.name == "time_offset").unwrap();
    let id_cstr = ds.1.0.as_c_str();
    let mut p: OfxParamHandle = ptr::null_mut();
    pgh(param_set, id_cstr.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;

    let mut num_keys: u32 = 0;
    pgnk(p, &mut num_keys).ofx_ok()?;

    let has_active_keyframes = if num_keys == 0 {
        false
    } else if num_keys == 1 {
        let pgkt = su.parameter_suite.paramGetKeyTime.ok_or(OfxStat::kOfxStatFailed)?;
        let mut kt0: f64 = 0.0;
        pgkt(p, 0, &mut kt0).ofx_ok()?;
        kt0 > 0.0
    } else {
        true
    };

    if !has_active_keyframes
        && settings.time_offset == 0.0
        && settings.position_x == 0.5
        && settings.position_y == 0.5
        && settings.rotation == 0.0
        && settings.blend_mode == ZzzStrokeBlendMode::Normal
    {
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
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
    let pgnk = su.parameter_suite.paramGetNumKeys.ok_or(OfxStat::kOfxStatFailed)?;
    let pgkt = su.parameter_suite.paramGetKeyTime.ok_or(OfxStat::kOfxStatFailed)?;

    let mut time: OfxTime = 0.0;
    pgd(inArgs, kOfxPropTime.as_ptr(), 0, &mut time).ofx_ok()?;

    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gps(effect, &mut param_set).ofx_ok()?;

    let mut settings = ZzzRepeaterFullSettings::default();
    apply_params(param_set, time, &mut settings)?;

    let mut sc: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"Source".as_ptr(), &mut sc, ptr::null_mut()).ofx_ok()?;
    let mut dc: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"Output".as_ptr(), &mut dc, ptr::null_mut()).ofx_ok()?;

    // --- Build layer list ---
    let to_ds = d.strings.iter().find(|(k, _)| k.name == "time_offset").unwrap();
    let to_id_cstr = to_ds.1.0.as_c_str();
    let mut to_p: OfxParamHandle = ptr::null_mut();
    pgh(param_set, to_id_cstr.as_ptr(), &mut to_p, ptr::null_mut()).ofx_ok()?;

    let rot_ds = d.strings.iter().find(|(k, _)| k.name == "rotation").unwrap();
    let rot_id_cstr = rot_ds.1.0.as_c_str();

    struct LayerInfo {
        source_time: f64,
        position_x: f32,
        position_y: f32,
        rotation: f32,
    }

    let read_position_at_time = |param_set: OfxParamSetHandle, t: f64| -> OfxResult<(f32, f32)> {
        let mut pp: OfxParamHandle = ptr::null_mut();
        pgh(param_set, POSITION_PARAM.as_ptr(), &mut pp, ptr::null_mut()).ofx_ok()?;
        let mut x: f64 = 0.0; let mut y: f64 = 0.0;
        pgv(pp, t, &mut x, &mut y).ofx_ok()?;
        Ok((x as f32, y as f32))
    };

    let read_rotation_at_time = |param_set: OfxParamSetHandle, t: f64| -> OfxResult<f32> {
        let mut rp: OfxParamHandle = ptr::null_mut();
        pgh(param_set, rot_id_cstr.as_ptr(), &mut rp, ptr::null_mut()).ofx_ok()?;
        let mut v: f64 = 0.0;
        pgv(rp, t, &mut v).ofx_ok()?;
        Ok(v as f32)
    };

    let (opx, opy) = read_position_at_time(param_set, 0.0)?;
    let orot = read_rotation_at_time(param_set, 0.0)?;

    let mut layers = vec![LayerInfo {
        source_time: (time - settings.time_offset as f64).max(0.0),
        position_x: opx, position_y: opy, rotation: orot,
    }];

    let mut num_keys: u32 = 0;
    pgnk(to_p, &mut num_keys).ofx_ok()?;

    for i in 0..num_keys {
        let mut kt: f64 = 0.0;
        pgkt(to_p, i, &mut kt).ofx_ok()?;
        if kt <= 0.0 || kt > time { continue; }
        let mut kv: f64 = 0.0;
        pgv(to_p, kt, &mut kv).ofx_ok()?;
        let (lpx, lpy) = read_position_at_time(param_set, kt)?;
        let lrot = read_rotation_at_time(param_set, kt)?;
        layers.push(LayerInfo {
            source_time: (kv + time - kt).max(0.0),
            position_x: lpx, position_y: lpy, rotation: lrot,
        });
    }

    let max_layers = settings.max_layers as usize;
    if max_layers > 0 && layers.len() > max_layers {
        let skip = layers.len() - max_layers;
        layers.drain(0..skip);
    }

    // --- Fetch dest image early to get project-size bounds ---
    let mut di: OfxPropertySetHandle = ptr::null_mut();
    let out_region = get_project_canonical_region(su, effect);
    let out_region_ptr: *const OfxRectD = match &out_region {
        Some(r) => r,
        None => ptr::null(),
    };
    cgi(dc, time, out_region_ptr, &mut di).ofx_ok()?;

    // ── Use DESTINATION bounds for working region ──────────────
    let mut dst_l: c_int = 0; let mut dst_b: c_int = 0;
    let mut dst_r: c_int = 0; let mut dst_t: c_int = 0;
    pgi(di, kOfxImagePropBounds.as_ptr(), 0, &mut dst_l).ofx_ok()?;
    pgi(di, kOfxImagePropBounds.as_ptr(), 1, &mut dst_b).ofx_ok()?;
    pgi(di, kOfxImagePropBounds.as_ptr(), 2, &mut dst_r).ofx_ok()?;
    pgi(di, kOfxImagePropBounds.as_ptr(), 3, &mut dst_t).ofx_ok()?;

    let bounds_w = (dst_r - dst_l).max(0) as usize;
    let bounds_h = (dst_t - dst_b).max(0) as usize;

    // Read dest stride to get actual buffer width
    let mut drb: c_int = 0;
    pgi(di, kOfxImagePropRowBytes.as_ptr(), 0, &mut drb).ofx_ok()?;
    let d_stride = drb.max(0) as usize;

    // --- Fetch first source image for depth detection ---
    let mut si0: OfxPropertySetHandle = ptr::null_mut();
    cgi(sc, layers[0].source_time, ptr::null(), &mut si0).ofx_ok()?;

    let depth = detect_pixel_depth(su, si0).ok_or(OfxStat::kOfxStatErrFormat)?;

    // VEGAS allocates full project-width buffers but restricts
    // pixel bounds to the crop region. Use stride for actual
    // buffer width so the effect isn't clipped.
    let stride_w = (d_stride / depth).min(16384);
    let width = bounds_w.max(stride_w);
    let height = bounds_h;
    if width == 0 || height == 0 { return Err(OfxStat::kOfxStatFailed); }
    if width > 16384 || height > 16384 { return Err(OfxStat::kOfxStatErrFormat); }
    let row_bytes_u8 = width * 4;
    let total_u8 = row_bytes_u8 * height;

    // Source bounds for offset within the destination region
    let mut src_l: c_int = 0; let mut src_b: c_int = 0;
    let mut src_r: c_int = 0; let mut src_t: c_int = 0;
    pgi(si0, kOfxImagePropBounds.as_ptr(), 0, &mut src_l).ofx_ok()?;
    pgi(si0, kOfxImagePropBounds.as_ptr(), 1, &mut src_b).ofx_ok()?;
    pgi(si0, kOfxImagePropBounds.as_ptr(), 2, &mut src_r).ofx_ok()?;
    pgi(si0, kOfxImagePropBounds.as_ptr(), 3, &mut src_t).ofx_ok()?;

    let src_w = (src_r - src_l).max(0) as usize;
    let src_h = (src_t - src_b).max(0) as usize;
    let off_x = ((src_l - dst_l).max(0)) as usize;
    let off_y = ((src_b - dst_b).max(0)) as usize;
    let same_region = src_l == dst_l && src_b == dst_b && src_r == dst_r && src_t == dst_t;

    let mut srb0: c_int = 0;
    pgi(si0, kOfxImagePropRowBytes.as_ptr(), 0, &mut srb0).ofx_ok()?;
    let s_stride0 = srb0.max(0) as usize;

    let copy_source_image = |si: OfxPropertySetHandle, s_stride: usize| -> Vec<u8> {
        let mut sp: *mut c_void = ptr::null_mut();
        let _ = pgp(si, kOfxImagePropData.as_ptr(), 0, &mut sp);
        if sp.is_null() { return vec![0u8; total_u8]; }
        let mut buf = vec![0u8; total_u8];
        if same_region {
            self::shared::copy_source_to_u8(sp, s_stride, &mut buf, width, height, row_bytes_u8, depth);
        } else {
            // Copy source into sub-rectangle at offset
            let mut tmp = vec![0u8; src_w * src_h * 4];
            self::shared::copy_source_to_u8(sp, s_stride, &mut tmp, src_w, src_h, src_w * 4, depth);
            let copy_w = (src_w * 4).min(row_bytes_u8.saturating_sub(off_x * 4));
            for row in 0..src_h.min(height.saturating_sub(off_y)) {
                let tmp_row = row * src_w * 4;
                let dst_row = (off_y + row) * row_bytes_u8 + off_x * 4;
                ptr::copy_nonoverlapping(
                    tmp.as_ptr().add(tmp_row),
                    buf.as_mut_ptr().add(dst_row),
                    copy_w,
                );
            }
        }
        buf
    };

    let mut layer_bufs: Vec<Vec<u8>> = Vec::with_capacity(layers.len());
    layer_bufs.push(copy_source_image(si0, s_stride0));
    cri(si0).ofx_ok()?;

    for i in 1..layers.len() {
        let mut si: OfxPropertySetHandle = ptr::null_mut();
        cgi(sc, layers[i].source_time, ptr::null(), &mut si).ofx_ok()?;
        let mut srb: c_int = 0;
        pgi(si, kOfxImagePropRowBytes.as_ptr(), 0, &mut srb).ofx_ok()?;
        layer_bufs.push(copy_source_image(si, srb.max(0) as usize));
        cri(si).ofx_ok()?;
    }

    // --- Composite ---
    let repeater: ZzzRepeater = (&settings).into();
    let bmode = settings.blend_mode;
    let compositor_layers: Vec<CompositorLayer> = layers.iter().zip(layer_bufs.iter())
        .map(|(info, buf)| CompositorLayer {
            rgba: buf.as_slice(),
            position_x: info.position_x, position_y: info.position_y,
            rotation_deg: info.rotation, blend_mode: bmode,
        })
        .collect();

    let mut dst_buf = vec![0u8; total_u8];
    repeater.composite_layers(&compositor_layers, &mut dst_buf, width, height);

    // --- Write output ---
    let mut dp: *mut c_void = ptr::null_mut();
    pgp(di, kOfxImagePropData.as_ptr(), 0, &mut dp).ofx_ok()?;
    let mut drb: c_int = 0;
    pgi(di, kOfxImagePropRowBytes.as_ptr(), 0, &mut drb).ofx_ok()?;
    copy_u8_to_output(&dst_buf, dp, drb.max(0) as usize, width, height, row_bytes_u8, depth);
    cri(di).ofx_ok()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Parameter reading
// ---------------------------------------------------------------------------

unsafe fn apply_params(
    param_set: OfxParamSetHandle,
    time: f64,
    dst: &mut ZzzRepeaterFullSettings,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    // --- Native Double2D: Position ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, POSITION_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut x: f64 = 0.0; let mut y: f64 = 0.0;
        pgv(p, time, &mut x, &mut y).ofx_ok()?;
        let find_id = |name: &str| -> SettingID<ZzzRepeaterFullSettings> {
            d.settings_list.setting_descriptors.iter().find(|d| d.id.name == name).unwrap().id.clone()
        };
        dst.set_field::<f32>(&find_id("position_x"), x.clamp(0.0, 1.0) as f32).unwrap();
        dst.set_field::<f32>(&find_id("position_y"), y.clamp(0.0, 1.0) as f32).unwrap();
    }

    // --- Read remaining generic params ---
    for desc in d.settings_list.setting_descriptors.iter() {
        if is_native_grouped_name(desc.id.name) { continue; }
        if let SettingKind::Group { .. } = &desc.kind {
            let ds = d.strings.get(&desc.id).unwrap();
            let id_cstr = ds.0.as_c_str();
            let mut p: OfxParamHandle = ptr::null_mut();
            pgh(param_set, id_cstr.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
            let mut v: c_int = 0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<bool>(&desc.id, v != 0).unwrap();
        } else {
            read_generic_param(su, param_set, time, desc, dst, &d.strings)?;
        }
    }

    Ok(())
}
