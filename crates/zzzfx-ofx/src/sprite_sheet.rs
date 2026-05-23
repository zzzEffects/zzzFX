use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    ptr,
    sync::OnceLock,
};

use zzzfx_core::settings::TrKey;
use zzzfx_core::{
    ZzzSpriteSheet, ZzzSpriteSheetFullSettings,
    settings::{SettingID, SettingKind, Settings, SettingsList},
};

use crate::bindings::*;
use crate::i18n;
use crate::shared::{
    HostInfo, SuiteCache, StringCache, MenuItemCache,
    build_string_cache, define_single_param, read_generic_param,
    action_load_common, action_get_clip_preferences_common,
};

// ---------------------------------------------------------------------------
// Native OFX parameter names
// ---------------------------------------------------------------------------

const SPRITE_RANGE_PARAM: &CStr = c"sprite_range";
const REPEAT_RANGE_PARAM: &CStr = c"repeat_range";
const DISPLACEMENT_PARAM: &CStr = c"displacement";
const FILE_SELECT_PARAM: &CStr = c"file_select";
const FILE_PATH_PARAM: &CStr = c"file_path";
const PAGE_NAME: &CStr = c"Controls";

fn is_native_grouped_name(name: &str) -> bool {
    matches!(
        name,
        "sprite_range_start" | "sprite_range_end"
        | "repeat_range_start" | "repeat_range_end"
        | "displacement_x" | "displacement_y"
    )
}

// ---------------------------------------------------------------------------
// Instance data
// ---------------------------------------------------------------------------

struct InstanceData {
    file_path: String,
    decoded_rgba: Vec<u8>,
    sheet_width: u32,
    sheet_height: u32,
    cached_dst: Vec<u8>,
    cache_valid: bool,
    cached_crop_x: u32,
    cached_crop_y: u32,
    cached_crop_w: u32,
    cached_crop_h: u32,
    cached_scale: f32,
    cached_filter: u32,
    cached_output_w: usize,
    cached_output_h: usize,
    cached_file_path: String,
    // Selection mode overlay state
    first_click_frame: Option<i32>,
    second_click_frame: Option<i32>,
    selection_range_start: Option<i32>,
    selection_range_end: Option<i32>,
}

// ---------------------------------------------------------------------------
// Per-effect globals
// ---------------------------------------------------------------------------

static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();

struct EffectData {
    suites: SuiteCache,
    settings_list: SettingsList<ZzzSpriteSheetFullSettings>,
    strings: StringCache<ZzzSpriteSheetFullSettings>,
    menu_item_strings: MenuItemCache<ZzzSpriteSheetFullSettings>,
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
        pluginIdentifier: c"com.example:zzzSpriteSheet".as_ptr(),
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
    let settings_list = SettingsList::<ZzzSpriteSheetFullSettings>::new();
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
    let effect = handle as OfxImageEffectHandle;
    let action = CStr::from_ptr(action);
    let r: OfxResult<()> = if action == kOfxActionLoad {
        action_load()
    } else if action == kOfxActionDescribe {
        action_describe(effect)
    } else if action == kOfxImageEffectActionDescribeInContext {
        action_describe_in_context(effect)
    } else if action == kOfxActionCreateInstance {
        action_create_instance(effect)
    } else if action == kOfxActionDestroyInstance {
        action_destroy_instance(effect)
    } else if action == kOfxActionInstanceChanged {
        action_instance_changed(effect, inArgs)
    } else if action == kOfxImageEffectActionRender {
        action_render(effect, inArgs)
    } else if action == kOfxImageEffectActionGetClipPreferences {
        action_get_clip_preferences(outArgs)
    } else if action == kOfxImageEffectActionIsIdentity {
        Err(OfxStat::kOfxStatReplyDefault)
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

    ps(ep, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::EffectSpritesheetName).as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginPropGrouping.as_ptr(), 0, c"zzzFX".as_ptr()).ofx_ok()?;
    ps(ep, kOfxPropPluginDescription.as_ptr(), 0, i18n::tr_cstr(TrKey::EffectSpritesheetDesc).as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedContexts.as_ptr(), 0, kOfxImageEffectContextGenerator.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 0, kOfxBitDepthFloat.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 1, kOfxBitDepthShort.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 2, kOfxBitDepthByte.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginRenderThreadSafety.as_ptr(), 0, kOfxImageEffectRenderFullySafe.as_ptr()).ofx_ok()?;
    pi(ep, kOfxImageEffectPluginPropHostFrameThreading.as_ptr(), 0, 0).ofx_ok()?;
    pi(ep, kOfxImageEffectPropSupportsTiles.as_ptr(), 0, 1).ofx_ok()?;
    pi(ep, kOfxImageEffectPropTemporalClipAccess.as_ptr(), 0, 0).ofx_ok()?;
    // Register overlay interact (V1 — receives image effect handle directly)
    let pp = su.property_suite.propSetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    pp(ep, kOfxImageEffectPluginPropOverlayInteractV1.as_ptr(), 0, overlay_main as *mut c_void).ofx_ok()?;
    Ok(())
}

unsafe fn action_describe_in_context(desc: OfxImageEffectHandle) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let cd = su.image_effect_suite.clipDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let gp = su.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = su.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pi = su.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let pdef = su.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let defaults = ZzzSpriteSheetFullSettings::default();

    // --- Output clip only (no Source for Generator) ---
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    cd(desc, c"Output".as_ptr(), &mut props).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 0, kOfxImageComponentRGBA.as_ptr()).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 1, kOfxImageComponentRGB.as_ptr()).ofx_ok()?;

    // --- Parameter set ---
    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gp(desc, &mut param_set).ofx_ok()?;

    // --- Page: Controls ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypePage.as_ptr(), PAGE_NAME.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeControls).as_ptr()).ofx_ok()?;
    }

    // --- Native PushButton: fileSelect ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypePushButton.as_ptr(), FILE_SELECT_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeSelectSpriteSheet).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeSelectSpriteSheetHint).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    }

    // --- Single-pass param definition with interleaved native Int2Ds ---
    let mut defined_range = false;
    let mut defined_repeat = false;
    let mut defined_displacement = false;

    for desc in d.settings_list.setting_descriptors.iter() {
        if !defined_range && desc.id.name == "sprite_range_start" {
            define_native_int2d(
                su, param_set, SPRITE_RANGE_PARAM,
                i18n::tr_cstr(TrKey::NativeSpriteRange),
                i18n::tr_cstr(TrKey::NativeSpriteRangeHint),
                defaults.sprite_range_start, defaults.sprite_range_end, 0, 1000,
            )?;
            defined_range = true;
        }
        if !defined_repeat && desc.id.name == "repeat_range_start" {
            define_native_int2d(
                su, param_set, REPEAT_RANGE_PARAM,
                i18n::tr_cstr(TrKey::NativeRepeatRange),
                i18n::tr_cstr(TrKey::NativeRepeatRangeHint),
                defaults.repeat_range_start, defaults.repeat_range_end, 0, 1000,
            )?;
            defined_repeat = true;
        }
        if !defined_displacement && desc.id.name == "displacement_x" {
            define_native_double2d(
                su, param_set, DISPLACEMENT_PARAM,
                i18n::tr_cstr(TrKey::ParamSpriteDisplacement),
                i18n::tr_cstr(TrKey::ParamSpriteDisplacementDesc),
                defaults.displacement_x as f64, defaults.displacement_y as f64,
                0.0, 1.0,
            )?;
            defined_displacement = true;
        }
        if is_native_grouped_name(desc.id.name) { continue; }
        define_single_param(su, param_set, desc, &defaults, PAGE_NAME, &d.strings, &d.menu_item_strings)?;
    }

    // --- Native String (hidden): filePath ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeString.as_ptr(), FILE_PATH_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeFilePath).as_ptr()).ofx_ok()?;
        pi(pp, kOfxParamPropSecret.as_ptr(), 0, 1).ofx_ok()?;
        pi(pp, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
        ps(pp, kOfxParamPropDefault.as_ptr(), 0, c"".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// CreateInstance / DestroyInstance
// ---------------------------------------------------------------------------

unsafe fn action_create_instance(effect: OfxImageEffectHandle) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let gp = su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?;
    let psp = su.property_suite.propSetPointer.ok_or(OfxStat::kOfxStatFailed)?;

    let mut ep: OfxPropertySetHandle = ptr::null_mut();
    gp(effect, &mut ep).ofx_ok()?;

    let idata = Box::new(InstanceData {
        file_path: String::new(), decoded_rgba: Vec::new(),
        sheet_width: 0, sheet_height: 0,
        cached_dst: Vec::new(), cache_valid: false,
        cached_crop_x: 0, cached_crop_y: 0, cached_crop_w: 0, cached_crop_h: 0,
        cached_scale: 0.0, cached_filter: 0,
        cached_output_w: 0, cached_output_h: 0,
        cached_file_path: String::new(),
        first_click_frame: None,
        second_click_frame: None,
        selection_range_start: None,
        selection_range_end: None,
    });
    psp(ep, kOfxPropInstanceData.as_ptr(), 0, Box::into_raw(idata) as *mut c_void).ofx_ok()?;
    Ok(())
}

unsafe fn action_destroy_instance(effect: OfxImageEffectHandle) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let gp = su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?;
    let gph = su.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;

    let mut ep: OfxPropertySetHandle = ptr::null_mut();
    gp(effect, &mut ep).ofx_ok()?;

    let mut data_ptr: *mut c_void = ptr::null_mut();
    gph(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
    if !data_ptr.is_null() { let _ = Box::from_raw(data_ptr as *mut InstanceData); }
    Ok(())
}

// ---------------------------------------------------------------------------
// GetClipPreferences
// ---------------------------------------------------------------------------

unsafe fn action_get_clip_preferences(outArgs: OfxPropertySetHandle) -> OfxResult<()> {
    action_get_clip_preferences_common(&data()?.suites, outArgs, 1, kOfxImagePreMultiplied)
}

// ---------------------------------------------------------------------------
// InstanceChanged (PushButton handling)
// ---------------------------------------------------------------------------

unsafe fn action_instance_changed(
    effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;

    let propGetString = su.property_suite.propGetString.ok_or(OfxStat::kOfxStatFailed)?;
    let getParamSet = su.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;
    let propGetPointer = su.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    let paramGetHandle = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let paramSetValue = su.parameter_suite.paramSetValue.ok_or(OfxStat::kOfxStatFailed)?;

    let mut target_type: *mut c_char = ptr::null_mut();
    propGetString(inArgs, kOfxPropType.as_ptr(), 0, &mut target_type).ofx_ok()?;

    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    getParamSet(effect, &mut param_set).ofx_ok()?;

    if CStr::from_ptr(target_type) == kOfxTypeParameter {
        let mut target_name: *mut c_char = ptr::null_mut();
        propGetString(inArgs, kOfxPropName.as_ptr(), 0, &mut target_name).ofx_ok()?;

        if FILE_SELECT_PARAM == CStr::from_ptr(target_name) {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tiff", "tif", "webp", "gif", "ico"])
                .pick_file()
            else { return Ok(()); };

            let img = image::open(&path).map_err(|_| OfxStat::kOfxStatFailed)?.to_rgba8();
            let (w, h) = img.dimensions();
            let rgba = img.into_raw();
            let path_str = path.to_string_lossy().to_string();

            let mut ep: OfxPropertySetHandle = ptr::null_mut();
            (su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?)(effect, &mut ep).ofx_ok()?;
            let mut data_ptr: *mut c_void = ptr::null_mut();
            propGetPointer(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
            if !data_ptr.is_null() {
                let idata = &mut *(data_ptr as *mut InstanceData);
                idata.file_path = path_str.clone();
                idata.decoded_rgba = rgba;
                idata.sheet_width = w;
                idata.sheet_height = h;
            }

            let mut p: OfxParamHandle = ptr::null_mut();
            paramGetHandle(param_set, FILE_PATH_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
            let path_cstr = CString::new(path_str).unwrap();
            paramSetValue(p, path_cstr.as_ptr() as *const c_void).ofx_ok()?;

            return Ok(());
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

unsafe fn action_render(
    effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
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

    let mut settings = ZzzSpriteSheetFullSettings::default();
    apply_params(param_set, time, &mut settings)?;

    // Retrieve instance data
    let gp = su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?;
    let gph = su.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    let mut ep: OfxPropertySetHandle = ptr::null_mut();
    gp(effect, &mut ep).ofx_ok()?;
    let mut data_ptr: *mut c_void = ptr::null_mut();
    gph(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
    let mut idata = if data_ptr.is_null() { None } else { Some(&mut *(data_ptr as *mut InstanceData)) };

    // Try to recover from hidden String param if instance data is empty
    if matches!(&idata, None) || idata.as_deref().is_some_and(|i| i.decoded_rgba.is_empty()) {
        let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
        let pgvt = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, FILE_PATH_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut path_ptr: *mut c_char = ptr::null_mut();
        if pgvt(p, 0.0, &mut path_ptr).ofx_ok().is_ok() && !path_ptr.is_null() {
            let path = CStr::from_ptr(path_ptr).to_string_lossy();
            if !path.is_empty() {
                if let Ok(img) = image::open(std::path::Path::new(path.as_ref())) {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let raw = rgba.into_raw();
                    if let Some(idata) = &mut idata {
                        idata.file_path = path.into_owned();
                        idata.decoded_rgba = raw;
                        idata.sheet_width = w;
                        idata.sheet_height = h;
                    }
                }
            }
        }
    }

    // Get output clip
    let mut dc: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"Output".as_ptr(), &mut dc, ptr::null_mut()).ofx_ok()?;

    let mut di: OfxPropertySetHandle = ptr::null_mut();
    cgi(dc, time, ptr::null(), &mut di).ofx_ok()?;

    let mut l: c_int = 0; let mut b: c_int = 0;
    let mut r: c_int = 0; let mut t: c_int = 0;
    pgi(di, kOfxImagePropBounds.as_ptr(), 0, &mut l).ofx_ok()?;
    pgi(di, kOfxImagePropBounds.as_ptr(), 1, &mut b).ofx_ok()?;
    pgi(di, kOfxImagePropBounds.as_ptr(), 2, &mut r).ofx_ok()?;
    pgi(di, kOfxImagePropBounds.as_ptr(), 3, &mut t).ofx_ok()?;

    let width = (r - l) as usize;
    let height = (t - b) as usize;

    let mut comp_ptr: *mut c_char = ptr::null_mut();
    let _ = su.property_suite.propGetString.ok_or(OfxStat::kOfxStatFailed)?(
        di, kOfxImageEffectPropComponents.as_ptr(), 0, &mut comp_ptr,
    );
    let num_components = if CStr::from_ptr(comp_ptr) == kOfxImageComponentRGB { 3 } else { 4 };

    let mut frame_rate: f64 = 1.0;
    let _ = pgd(ep, kOfxImageEffectPropFrameRate.as_ptr(), 0, &mut frame_rate);

    let mut ss: ZzzSpriteSheet = (&settings).into();

    // Apply pending selection from overlay clicks, or clear if mode toggled off
    if ss.selection_mode {
        if let Some(ref idata_inner) = idata {
            if let (Some(start), Some(end)) = (idata_inner.selection_range_start, idata_inner.selection_range_end) {
                ss.sprite_range_start = start;
                ss.sprite_range_end = end;
            }
        }
    } else if let Some(ref mut idata_inner) = idata {
        // Clear stale selection state when selection mode is off
        idata_inner.first_click_frame = None;
        idata_inner.second_click_frame = None;
        idata_inner.selection_range_start = None;
        idata_inner.selection_range_end = None;
    }

    // --- Compute integrated speed offset (trapezoidal integration) ---
    let rate = if frame_rate > 0.0 { frame_rate } else { 1.0 };
    let integrated_speed_offset: Option<f64> = if ss.speed == 0.0 || ss.selection_mode {
        None // static or selection mode: use instantaneous speed
    } else {
        let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
        let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
        // Find the speed descriptor to get its OFX param name
        let speed_desc = d.settings_list.all_descriptors()
            .find(|desc| desc.id.name == "speed");
        if let Some(speed_desc) = speed_desc {
            let ds = d.strings.get(&speed_desc.id);
            if let Some(ds) = ds {
                let mut speed_handle: OfxParamHandle = ptr::null_mut();
                pgh(param_set, ds.0.as_c_str().as_ptr(), &mut speed_handle, ptr::null_mut()).ofx_ok()?;
                // Cap samples at 64 for performance, use trapezoidal integration
                let n_samples = ((time * rate).ceil() as usize).min(64);
                let mut integral = 0.0f64;
                let mut prev_speed: f64 = 0.0;
                let mut prev_t: f64 = 0.0;
                for i in 1..=n_samples {
                    let t = time * (i as f64) / (n_samples as f64);
                    let mut sp: f64 = 0.0;
                    pgv(speed_handle, t, &mut sp).ofx_ok()?;
                    let dt = t - prev_t;
                    integral += (prev_speed + sp) * 0.5 * dt / rate;
                    prev_speed = sp;
                    prev_t = t;
                }
                Some(integral)
            } else {
                None
            }
        } else {
            None
        }
    };

    let is_static = (ss.sprite_rows == 1 && ss.sprite_columns == 1)
        || ss.sprite_range_start == ss.sprite_range_end
        || ss.speed == 0.0;
    let filter_discriminant = ss.scale_algorithm as u32;

    let mut dst_buf = vec![0u8; width * height * 4];

    // Selection mode: render full sheet with grid
    if ss.selection_mode {
        if let Some(ref idata_inner) = idata {
            if !idata_inner.decoded_rgba.is_empty() {
                ss.render_selection_mode(
                    &idata_inner.decoded_rgba,
                    idata_inner.sheet_width, idata_inner.sheet_height,
                    &mut dst_buf, width, height,
                );
            }
        }
        // Invalidate cache in selection mode
        if let Some(ref mut idata_inner) = idata {
            idata_inner.cache_valid = false;
        }
    } else {
        let cache_hit = 'cache: {
            if !is_static { break 'cache false; }
            let idata_ref = idata.as_deref();
            let Some(idata_ref) = idata_ref else { break 'cache false };
            if !idata_ref.cache_valid
                || idata_ref.cached_output_w != width
                || idata_ref.cached_output_h != height
                || idata_ref.cached_scale != ss.scale
                || idata_ref.cached_filter != filter_discriminant
                || idata_ref.cached_file_path != idata_ref.file_path
            { break 'cache false; }
            if let Some(crop_rect) = ss.get_crop_rect(time, frame_rate, idata_ref.sheet_width, idata_ref.sheet_height, None) {
                if crop_rect == (idata_ref.cached_crop_x, idata_ref.cached_crop_y, idata_ref.cached_crop_w, idata_ref.cached_crop_h) {
                    dst_buf.copy_from_slice(&idata_ref.cached_dst);
                    break 'cache true;
                }
            }
            false
        };

        let mut rendered_rect: Option<(u32, u32, u32, u32)> = None;

        if !cache_hit {
            if let Some(ref idata_inner) = idata {
                if !idata_inner.decoded_rgba.is_empty() {
                    if let Some(crop_rect) = ss.get_crop_rect(time, frame_rate, idata_inner.sheet_width, idata_inner.sheet_height, integrated_speed_offset) {
                        ss.render_sprite(
                            crop_rect, &idata_inner.decoded_rgba,
                            idata_inner.sheet_width, idata_inner.sheet_height,
                            &mut dst_buf, width, height,
                        );
                        rendered_rect = Some(crop_rect);
                    }
                }
            }

            let cache_file_path = idata.as_deref().map(|i| i.file_path.clone()).unwrap_or_default();
            if let Some(crop_rect) = rendered_rect {
                if is_static {
                    if let Some(ref mut idata_inner) = idata {
                        idata_inner.cached_dst = dst_buf.clone();
                        idata_inner.cache_valid = true;
                        idata_inner.cached_crop_x = crop_rect.0;
                        idata_inner.cached_crop_y = crop_rect.1;
                        idata_inner.cached_crop_w = crop_rect.2;
                        idata_inner.cached_crop_h = crop_rect.3;
                        idata_inner.cached_scale = ss.scale;
                        idata_inner.cached_filter = filter_discriminant;
                        idata_inner.cached_output_w = width;
                        idata_inner.cached_output_h = height;
                        idata_inner.cached_file_path = cache_file_path;
                    }
                }
            } else if !is_static {
                if let Some(ref mut idata_inner) = idata {
                    idata_inner.cache_valid = false;
                }
            }
        }
    }

    if num_components == 4 {
        for pixel in dst_buf.chunks_exact_mut(4) {
            let a = pixel[3] as f32 / 255.0;
            pixel[0] = (pixel[0] as f32 * a).round() as u8;
            pixel[1] = (pixel[1] as f32 * a).round() as u8;
            pixel[2] = (pixel[2] as f32 * a).round() as u8;
        }
    }

    let mut dp: *mut c_void = ptr::null_mut();
    pgp(di, kOfxImagePropData.as_ptr(), 0, &mut dp).ofx_ok()?;
    let mut drb: c_int = 0;
    pgi(di, kOfxImagePropRowBytes.as_ptr(), 0, &mut drb).ofx_ok()?;
    let d_stride = drb.max(0) as usize;

    let mut depth_ptr: *mut c_char = ptr::null_mut();
    let _ = su.property_suite.propGetString.ok_or(OfxStat::kOfxStatFailed)?(
        di, kOfxImageEffectPropPixelDepth.as_ptr(), 0, &mut depth_ptr,
    );
    let depth_str = CStr::from_ptr(depth_ptr);

    let src_row_bytes = width * 4;
    if depth_str == kOfxBitDepthByte {
        for y in 0..height {
            let src_row = (height - 1 - y) * src_row_bytes;
            let host_row = (dp as *mut u8).add(y * d_stride);
            for x in 0..width {
                let si = src_row + x * 4;
                let di = x * num_components;
                host_row.add(di).copy_from_nonoverlapping(dst_buf.as_ptr().add(si), num_components);
            }
        }
    } else if depth_str == kOfxBitDepthShort {
        for y in 0..height {
            let src_row = (height - 1 - y) * src_row_bytes;
            let host_row = (dp as *mut u8).add(y * d_stride) as *mut u16;
            for x in 0..width {
                for c in 0..num_components {
                    let v = *dst_buf.as_ptr().add(src_row + x * 4 + c) as u16;
                    *host_row.add(x * num_components + c) = (v << 8) | v;
                }
            }
        }
    } else {
        for y in 0..height {
            let src_row = (height - 1 - y) * src_row_bytes;
            let host_row = (dp as *mut u8).add(y * d_stride) as *mut f32;
            for x in 0..width {
                for c in 0..num_components {
                    let v = *dst_buf.as_ptr().add(src_row + x * 4 + c) as f32 / 255.0;
                    *host_row.add(x * num_components + c) = v;
                }
            }
        }
    }

    cri(di).ofx_ok()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Native Int2D helper
// ---------------------------------------------------------------------------

unsafe fn define_native_int2d(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    name: &CStr,
    label: &CStr,
    hint: &CStr,
    default_x: i32,
    default_y: i32,
    min: i32,
    max: i32,
) -> OfxResult<()> {
    let pdef = suites.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = suites.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pi = suites.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let mut pp: OfxPropertySetHandle = ptr::null_mut();
    pdef(param_set, kOfxParamTypeInteger2D.as_ptr(), name.as_ptr(), &mut pp).ofx_ok()?;
    ps(pp, kOfxPropLabel.as_ptr(), 0, label.as_ptr()).ofx_ok()?;
    ps(pp, kOfxParamPropHint.as_ptr(), 0, hint.as_ptr()).ofx_ok()?;
    pi(pp, kOfxParamPropDefault.as_ptr(), 0, default_x).ofx_ok()?;
    pi(pp, kOfxParamPropDefault.as_ptr(), 1, default_y).ofx_ok()?;
    pi(pp, kOfxParamPropMin.as_ptr(), 0, min).ofx_ok()?;
    pi(pp, kOfxParamPropMin.as_ptr(), 1, min).ofx_ok()?;
    pi(pp, kOfxParamPropMax.as_ptr(), 0, max).ofx_ok()?;
    pi(pp, kOfxParamPropMax.as_ptr(), 1, max).ofx_ok()?;
    ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    Ok(())
}

unsafe fn define_native_double2d(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    name: &CStr,
    label: &CStr,
    hint: &CStr,
    default_x: f64,
    default_y: f64,
    min: f64,
    max: f64,
) -> OfxResult<()> {
    let pdef = suites.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = suites.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pd = suites.property_suite.propSetDouble.ok_or(OfxStat::kOfxStatFailed)?;
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
    ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Parameter reading
// ---------------------------------------------------------------------------

unsafe fn apply_params(
    param_set: OfxParamSetHandle,
    time: f64,
    dst: &mut ZzzSpriteSheetFullSettings,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    let find_id = |name: &str| -> OfxResult<SettingID<ZzzSpriteSheetFullSettings>> {
        d.settings_list.all_descriptors()
            .find(|d| d.id.name == name)
            .map(|d| d.id.clone())
            .ok_or(OfxStat::kOfxStatFailed)
    };

    // --- Native Integer2D: spriteRange ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, SPRITE_RANGE_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut x: c_int = 0; let mut y: c_int = 0;
        pgv(p, time, &mut x, &mut y).ofx_ok()?;
        dst.set_field::<i32>(&find_id("sprite_range_start")?, x.clamp(0, 1000)).unwrap();
        dst.set_field::<i32>(&find_id("sprite_range_end")?, y.clamp(0, 1000)).unwrap();
    }

    // --- Native Integer2D: repeatRange ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, REPEAT_RANGE_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut x: c_int = 0; let mut y: c_int = 0;
        pgv(p, time, &mut x, &mut y).ofx_ok()?;
        dst.set_field::<i32>(&find_id("repeat_range_start")?, x.clamp(0, 1000)).unwrap();
        dst.set_field::<i32>(&find_id("repeat_range_end")?, y.clamp(0, 1000)).unwrap();
    }

    // --- Native Double2D: displacement ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, DISPLACEMENT_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut x: f64 = 0.0; let mut y: f64 = 0.0;
        pgv(p, time, &mut x, &mut y).ofx_ok()?;
        dst.set_field::<f32>(&find_id("displacement_x")?, x.clamp(0.0, 1.0) as f32).unwrap();
        dst.set_field::<f32>(&find_id("displacement_y")?, y.clamp(0.0, 1.0) as f32).unwrap();
    }

    // --- Read generic params ---
    for desc in d.settings_list.all_descriptors() {
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

// ---------------------------------------------------------------------------
// Overlay Interact (V1 — receives image effect handle directly)
// ---------------------------------------------------------------------------

unsafe extern "C" fn overlay_main(
    action: *const c_char,
    handle: *const c_void,
    inArgs: OfxPropertySetHandle,
    _outArgs: OfxPropertySetHandle,
) -> OfxStatus {
    let effect = handle as OfxImageEffectHandle;
    let action = CStr::from_ptr(action);
    if action == kOfxInteractActionPenDown {
        match interact_pen_down(effect, inArgs) {
            Ok(()) => OfxStat::kOfxStatOK,
            Err(_) => OfxStat::kOfxStatReplyDefault,
        }
    } else if action == kOfxInteractActionDraw
        || action == kOfxInteractActionPenUp
        || action == kOfxInteractActionPenMotion
    {
        // Grid is rendered in the output image; overlay draw is a no-op.
        OfxStat::kOfxStatOK
    } else {
        OfxStat::kOfxStatReplyDefault
    }
}

unsafe fn interact_pen_down(
    effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;

    // Get instance data
    let gp = su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?;
    let gph = su.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    let gps = su.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;

    let mut ep: OfxPropertySetHandle = ptr::null_mut();
    gp(effect, &mut ep).ofx_ok()?;
    let mut data_ptr: *mut c_void = ptr::null_mut();
    gph(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
    if data_ptr.is_null() { return Ok(()); }
    let idata = &mut *(data_ptr as *mut InstanceData);

    // Read current params to check selection mode
    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gps(effect, &mut param_set).ofx_ok()?;
    let mut settings = ZzzSpriteSheetFullSettings::default();
    let _ = apply_params(param_set, 0.0, &mut settings);
    let ss: ZzzSpriteSheet = (&settings).into();

    if !ss.selection_mode {
        return Ok(());
    }

    // Read pen position
    let pgd = su.property_suite.propGetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let mut pen_x: f64 = 0.0;
    let mut pen_y: f64 = 0.0;
    pgd(inArgs, c"OfxInteractPropPenPosition".as_ptr(), 0, &mut pen_x).ofx_ok()?;
    pgd(inArgs, c"OfxInteractPropPenPosition".as_ptr(), 1, &mut pen_y).ofx_ok()?;

    // Convert pen coords to grid cell using reading-direction-aware absolute index
    let columns = ss.sprite_columns.max(1);
    let rows = ss.sprite_rows.max(1);
    let col = ((pen_x * columns as f64).floor() as i32).clamp(0, columns - 1);
    let row = ((pen_y * rows as f64).floor() as i32).clamp(0, rows - 1);
    let frame_idx = ss.get_absolute_index(row, col);

    // Two-click selection: first click sets start, second click sets end
    if idata.first_click_frame.is_none() {
        idata.first_click_frame = Some(frame_idx);
        idata.second_click_frame = None;
    } else if idata.second_click_frame.is_none() {
        let start = idata.first_click_frame.unwrap();
        let end = frame_idx;
        idata.second_click_frame = Some(frame_idx);

        // Store the selected range for use in action_render
        let lo = start.min(end);
        let hi = start.max(end);
        idata.selection_range_start = Some(lo);
        idata.selection_range_end = Some(hi);
    } else {
        // Reset: start new selection, clear stale range
        idata.first_click_frame = Some(frame_idx);
        idata.second_click_frame = None;
        idata.selection_range_start = None;
        idata.selection_range_end = None;
    }

    Ok(())
}
