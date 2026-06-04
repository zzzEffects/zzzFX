use std::{
    ffi::{CStr, c_char, c_int, c_void},
    ptr,
    sync::OnceLock,
};

use zzzfx::settings::TrKey;
use zzzfx::{
    SpriteSheet, SpriteSheetFullSettings,
    settings::{SettingID, SettingKind, Settings, SettingsList},
};

use crate::bindings::*;
use crate::file_param;
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

const INSTANCE_MAGIC: u64 = 0x5AFE_5AFE_5AFE_5AFE;

struct InstanceData {
    magic: u64,
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
    // Output dimensions (set during render, used by interact for coordinate mapping)
    output_w: usize,
    output_h: usize,
}

// ---------------------------------------------------------------------------
// Per-effect globals
// ---------------------------------------------------------------------------

static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();

struct EffectData {
    suites: SuiteCache,
    settings_list: SettingsList<SpriteSheetFullSettings>,
    strings: StringCache<SpriteSheetFullSettings>,
    menu_item_strings: MenuItemCache<SpriteSheetFullSettings>,
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
        pluginIdentifier: c"io.github.zzzEffect:SpriteSheet".as_ptr(),
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
    let settings_list = SettingsList::<SpriteSheetFullSettings>::new();
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
    // Register overlay interact (V2 — uses OfxDrawSuiteV1, required by modern hosts)
    let pp = su.property_suite.propSetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    pp(ep, kOfxImageEffectPluginPropOverlayInteractV2.as_ptr(), 0, overlay_main as *mut c_void).ofx_ok()?;
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
    let defaults = SpriteSheetFullSettings::default();

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

    // --- Reload File button (hidden by default) ---
    file_param::define_reload_button(su, param_set, PAGE_NAME)?;

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

    // --- Custom param (hidden): fileData (persisted binary) ---
    file_param::define_file_data_param(su, param_set, PAGE_NAME)?;

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
        magic: INSTANCE_MAGIC,
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
        output_w: 0,
        output_h: 0,
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
    if !data_ptr.is_null() {
        let idata = &*(data_ptr as *const InstanceData);
        if idata.magic == INSTANCE_MAGIC {
            let _ = Box::from_raw(data_ptr as *mut InstanceData);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// GetClipPreferences (SpriteSheet)
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

    let mut target_type: *mut c_char = ptr::null_mut();
    if propGetString(inArgs, kOfxPropType.as_ptr(), 0, &mut target_type).ofx_ok().is_err() {
        return Ok(());
    }
    if target_type.is_null() { return Ok(()); }

    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    getParamSet(effect, &mut param_set).ofx_ok()?;

    if CStr::from_ptr(target_type) == kOfxTypeParameter {
        let mut target_name: *mut c_char = ptr::null_mut();
        if propGetString(inArgs, kOfxPropName.as_ptr(), 0, &mut target_name).ofx_ok().is_err() {
            return Ok(());
        }
        if target_name.is_null() { return Ok(()); }

        if FILE_SELECT_PARAM == CStr::from_ptr(target_name) {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tiff", "tif", "webp", "gif", "ico"])
                .pick_file()
            else { return Ok(()); };

            let file_bytes = std::fs::read(&path).map_err(|_| OfxStat::kOfxStatFailed)?;
            let img = image::load_from_memory(&file_bytes).map_err(|_| OfxStat::kOfxStatFailed)?.to_rgba8();
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

            file_param::write_custom_param_bytes(su, param_set, file_param::FILE_DATA_PARAM, &file_bytes)?;
            file_param::write_string_param(su, param_set, FILE_PATH_PARAM, &path_str)?;
            file_param::reveal_param(su, param_set, file_param::RELOAD_FILE_PARAM)?;

            return Ok(());
        }

        if file_param::RELOAD_FILE_PARAM == CStr::from_ptr(target_name) {
            let path_str = file_param::read_string_param(su, param_set, FILE_PATH_PARAM)?;
            if path_str.is_empty() { return Ok(()); }

            let file_bytes = std::fs::read(&path_str).map_err(|_| OfxStat::kOfxStatFailed)?;
            let img = image::load_from_memory(&file_bytes).map_err(|_| OfxStat::kOfxStatFailed)?.to_rgba8();
            let (w, h) = img.dimensions();

            file_param::write_custom_param_bytes(su, param_set, file_param::FILE_DATA_PARAM, &file_bytes)?;

            let mut ep: OfxPropertySetHandle = ptr::null_mut();
            (su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?)(effect, &mut ep).ofx_ok()?;
            let mut data_ptr: *mut c_void = ptr::null_mut();
            propGetPointer(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
            if !data_ptr.is_null() {
                let idata = &mut *(data_ptr as *mut InstanceData);
                idata.file_path = path_str;
                idata.decoded_rgba = img.into_raw();
                idata.sheet_width = w;
                idata.sheet_height = h;
            }

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

    let mut settings = SpriteSheetFullSettings::default();
    apply_params(param_set, time, &mut settings)?;

    // Retrieve instance data
    let gp = su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?;
    let gph = su.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    let mut ep: OfxPropertySetHandle = ptr::null_mut();
    gp(effect, &mut ep).ofx_ok()?;
    let mut data_ptr: *mut c_void = ptr::null_mut();
    gph(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
    let mut idata = if data_ptr.is_null() { None } else { Some(&mut *(data_ptr as *mut InstanceData)) };

    // Recover image data from Custom param on project reload
    if let Some(ref mut idata_inner) = idata {
        if idata_inner.decoded_rgba.is_empty() {
            if let Ok(bytes) = file_param::read_custom_param_bytes(su, param_set, file_param::FILE_DATA_PARAM) {
                if !bytes.is_empty() {
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        let rgba_img = img.to_rgba8();
                        idata_inner.sheet_width = rgba_img.width();
                        idata_inner.sheet_height = rgba_img.height();
                        idata_inner.decoded_rgba = rgba_img.into_raw();
                    }
                }
            }
        }
    }

    // If instance data is missing or no image loaded, render empty frame.
    if matches!(&idata, None) || idata.as_deref().is_some_and(|i| i.decoded_rgba.is_empty()) {
        // Still need to get output clip for bounds, then fill transparent
    }

    // Get output clip and its property set (for frame range query)
    let mut dc: OfxImageClipHandle = ptr::null_mut();
    let mut clip_props: OfxPropertySetHandle = ptr::null_mut();
    cgh(effect, c"Output".as_ptr(), &mut dc, &mut clip_props).ofx_ok()?;

    let mut di: OfxPropertySetHandle = ptr::null_mut();
    cgi(dc, time, ptr::null(), &mut di).ofx_ok()?;

    let mut l: c_int = 0; let mut b: c_int = 0;
    let mut r: c_int = 0; let mut t: c_int = 0;
    pgi(di, kOfxImagePropBounds.as_ptr(), 0, &mut l).ofx_ok()?;
    pgi(di, kOfxImagePropBounds.as_ptr(), 1, &mut b).ofx_ok()?;
    pgi(di, kOfxImagePropBounds.as_ptr(), 2, &mut r).ofx_ok()?;
    pgi(di, kOfxImagePropBounds.as_ptr(), 3, &mut t).ofx_ok()?;

    let width = (r - l).max(0) as usize;
    let height = (t - b).max(0) as usize;
    if width == 0 || height == 0 { return Err(OfxStat::kOfxStatFailed); }
    if width > 16384 || height > 16384 { return Err(OfxStat::kOfxStatErrFormat); }

    // Store output dimensions for use by the overlay interact (coordinate mapping)
    if let Some(ref mut idata_inner) = idata {
        idata_inner.output_w = width;
        idata_inner.output_h = height;
    }

    let num_components = {
        let mut comp_ptr: *mut c_char = ptr::null_mut();
        if su.property_suite.propGetString
            .and_then(|pgs| pgs(di, kOfxImageEffectPropComponents.as_ptr(), 0, &mut comp_ptr).ofx_ok().ok())
            .is_some() && !comp_ptr.is_null()
        {
            if CStr::from_ptr(comp_ptr) == kOfxImageComponentRGB { 3 } else { 4 }
        } else {
            4
        }
    };

    let mut frame_rate: f64 = 1.0;
    let _ = pgd(ep, kOfxImageEffectPropFrameRate.as_ptr(), 0, &mut frame_rate);

    let mut ss: SpriteSheet = (&settings).into();

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

    // --- Override speed for negative play_count ---
    // When play_count < 0, auto-compute speed so that |play_count| complete
    // animation cycles fit within the host's total timeline duration.
    // The host provides the total frame range via kOfxImageEffectPropFrameRange
    // in the render inArgs; dividing by frame_rate gives the duration in seconds.
    if ss.play_count < 0 {
        let cycle_frames = ss.cycle_frame_count() as f64;
        let rate_for_speed = if frame_rate > 0.0 { frame_rate as f64 } else { 1.0 };
        // Read the total generator frame range from the Output clip's property set
        let mut frame_range = [0.0f64; 2];
        let _ = pgd(clip_props, kOfxImageEffectPropFrameRange.as_ptr(), 0, &mut frame_range[0]);
        let _ = pgd(clip_props, kOfxImageEffectPropFrameRange.as_ptr(), 1, &mut frame_range[1]);
        let total_dur = ((frame_range[1] - frame_range[0]) / rate_for_speed).max(0.001);
        let abs_n = (-ss.play_count) as f64;
        ss.speed = ((cycle_frames * abs_n) / total_dur) as f32;
    }

    // --- Compute integrated speed offset (trapezoidal integration) ---
    let rate = if frame_rate > 0.0 { frame_rate } else { 1.0 };
    let integrated_speed_offset: Option<f64> = if ss.play_count < 0 || ss.speed == 0.0 || ss.selection_mode {
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
        let first_click = idata.as_ref().and_then(|i| i.first_click_frame);
        let has_range = idata.as_ref()
            .map_or(false, |i| i.selection_range_start.is_some() && i.selection_range_end.is_some());
        // When waiting for second click, suppress range highlighting so only
        // the white first-click cell is visible.
        if !has_range {
            ss.sprite_range_start = -1;
            ss.sprite_range_end = -1;
        }
        if let Some(ref idata_inner) = idata {
            if !idata_inner.decoded_rgba.is_empty() {
                ss.render_selection_mode(
                    &idata_inner.decoded_rgba,
                    idata_inner.sheet_width, idata_inner.sheet_height,
                    &mut dst_buf, width, height,
                    first_click,
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

    let depth_str = {
        let mut depth_ptr: *mut c_char = ptr::null_mut();
        if su.property_suite.propGetString
            .and_then(|pgs| pgs(di, kOfxImageEffectPropPixelDepth.as_ptr(), 0, &mut depth_ptr).ofx_ok().ok())
            .is_some() && !depth_ptr.is_null()
        {
            CStr::from_ptr(depth_ptr)
        } else {
            kOfxBitDepthByte
        }
    };

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
    pi(pp, kOfxParamPropAnimates.as_ptr(), 0, 1).ofx_ok()?;
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
    dst: &mut SpriteSheetFullSettings,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    let find_id = |name: &str| -> OfxResult<SettingID<SpriteSheetFullSettings>> {
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
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        overlay_main_inner(action, handle, inArgs, _outArgs)
    }));
    match result {
        Ok(status) => status,
        Err(_) => OfxStat::kOfxStatFailed,
    }
}

unsafe fn overlay_main_inner(
    action: *const c_char,
    handle: *const c_void,
    inArgs: OfxPropertySetHandle,
    _outArgs: OfxPropertySetHandle,
) -> OfxStatus {
    if action.is_null() { return OfxStat::kOfxStatFailed; }
    let action = CStr::from_ptr(action);

    // OFX interact lifecycle: return OK for describe/create/destroy so the
    // host (e.g. VEGAS Pro) knows the interact is valid and will create the
    // interactive controls for pen events.
    if action == kOfxActionDescribe
        || action == kOfxActionCreateInstance
        || action == kOfxActionDestroyInstance
    {
        return OfxStat::kOfxStatOK;
    }

    // Runtime actions: V2 overlay — handle is OfxInteractHandle.
    // The owning effect handle is read from kOfxPropEffectInstance in inArgs.
    if action == kOfxInteractActionPenDown {
        match interact_pen_down(handle, inArgs) {
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
    interact: *const c_void,
    inArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;

    // V2 overlay: read the owning effect handle from inArgs
    let pgp = su.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    let mut effect_ptr: *mut c_void = ptr::null_mut();
    pgp(inArgs, kOfxPropEffectInstance.as_ptr(), 0, &mut effect_ptr).ofx_ok()?;
    if effect_ptr.is_null() { return Err(OfxStat::kOfxStatFailed); }
    let effect = effect_ptr as OfxImageEffectHandle;

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
    let mut settings = SpriteSheetFullSettings::default();
    let _ = apply_params(param_set, 0.0, &mut settings);
    let ss: SpriteSheet = (&settings).into();

    if !ss.selection_mode {
        return Ok(());
    }

    // Ignore clicks before the first render (no output dimensions known yet)
    if idata.output_w == 0 || idata.output_h == 0 || idata.sheet_width == 0 || idata.sheet_height == 0 {
        return Ok(());
    }

    // Read pen position (OFX convention: [0..1], origin at bottom-left)
    let pgd = su.property_suite.propGetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let mut pen_x: f64 = 0.0;
    let mut pen_y: f64 = 0.0;
    pgd(inArgs, c"OfxInteractPropPenPosition".as_ptr(), 0, &mut pen_x).ofx_ok()?;
    pgd(inArgs, c"OfxInteractPropPenPosition".as_ptr(), 1, &mut pen_y).ofx_ok()?;

    // Compute rendered sheet geometry (must match render_selection_mode in core)
    let fit_scale = if ss.fit_sprite_sheet_to_output {
        (idata.output_w as f32 / idata.sheet_width as f32)
            .min(idata.output_h as f32 / idata.sheet_height as f32)
    } else {
        ss.scale
    }.max(0.01);
    let out_w = ((idata.sheet_width as f32 * fit_scale).round() as i32).max(1);
    let out_h = ((idata.sheet_height as f32 * fit_scale).round() as i32).max(1);
    let offset_x = (idata.output_w as i32 - out_w) / 2;
    let offset_y = (idata.output_h as i32 - out_h) / 2;

    // Convert pen [0..1] from output window to sheet-pixel coords
    let px_sheet = pen_x * idata.output_w as f64 - offset_x as f64;
    let py_sheet = pen_y * idata.output_h as f64 - offset_y as f64; // OFX: 0=bottom
    // Flip Y to match rendering convention (top=0)
    let py_sheet = out_h as f64 - py_sheet;

    // Ignore clicks outside the rendered sheet area (letterbox regions)
    if px_sheet < 0.0 || px_sheet >= out_w as f64 || py_sheet < 0.0 || py_sheet >= out_h as f64 {
        return Ok(());
    }

    // Map sheet-pixel position to grid cell
    let columns = ss.sprite_columns.max(1);
    let rows = ss.sprite_rows.max(1);
    let sprite_w = out_w / columns;
    let sprite_h = out_h / rows;
    let col = ((px_sheet / sprite_w as f64).floor() as i32).clamp(0, columns - 1);
    let row = ((py_sheet / sprite_h as f64).floor() as i32).clamp(0, rows - 1);
    let frame_idx = ss.get_absolute_index(row, col);

    // Get sprite_range param handle for second-click write-back
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let paramSetValue = su.parameter_suite.paramSetValue.ok_or(OfxStat::kOfxStatFailed)?;
    let mut sp: OfxParamHandle = ptr::null_mut();
    pgh(param_set, SPRITE_RANGE_PARAM.as_ptr(), &mut sp, ptr::null_mut()).ofx_ok()?;

    // Two-click selection
    if idata.first_click_frame.is_none() {
        // First click: clear previous selection, highlight clicked cell white.
        idata.selection_range_start = None;
        idata.selection_range_end = None;
        idata.first_click_frame = Some(frame_idx);
        idata.second_click_frame = None;
    } else if let (None, Some(start)) = (idata.second_click_frame, idata.first_click_frame) {
        // Second click: commit the range.
        let end = frame_idx;
        let lo = start.min(end);
        let hi = start.max(end);
        idata.second_click_frame = Some(frame_idx);
        idata.selection_range_start = Some(lo);
        idata.selection_range_end = Some(hi);
        // Write to OFX parameter — triggers re-render with new range
        paramSetValue(sp, lo, hi).ofx_ok()?;
    } else {
        // Third click: reset — clear selection, start fresh with white highlight.
        idata.selection_range_start = None;
        idata.selection_range_end = None;
        idata.first_click_frame = Some(frame_idx);
        idata.second_click_frame = None;
    }

    // Request redraw; the host's Draw action returns kOfxStatOK and on some
    // hosts (including VEGAS Pro) this triggers a full effect re-render.
    if let Some(is) = d.suites.interact_suite {
        if let Some(redraw) = is.interactRedraw {
            let _ = redraw(interact as OfxInteractHandle);
        }
    }

    Ok(())
}
