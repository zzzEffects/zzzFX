use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    ptr,
    sync::OnceLock,
};

use zzzfx_core::{
    ZzzAssSubtitle, ZzzAssSubtitleFullSettings,
    ass_subtitle::{AssScript, FontCache, RenderCache, parse_ass_file, render_ass_subtitle_frame},
    settings::{SettingID, SettingKind, Settings, SettingsList, TrKey},
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

const FILE_SELECT_PARAM: &CStr = c"select_file";
const FILE_PATH_PARAM: &CStr = c"file_path";
const FONT_OVERRIDE_CHOICE_PARAM: &CStr = c"font_override_choice";
const FONT_OVERRIDE_STRING_PARAM: &CStr = c"font_override_name";
const PAGE_NAME: &CStr = c"Controls";

/// Global cache of installed font names, built lazily on first access.
fn cached_font_names() -> &'static Vec<String> {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let cache = FontCache::new();
        cache.list_font_names()
    })
}

// ---------------------------------------------------------------------------
// Instance data
// ---------------------------------------------------------------------------

struct InstanceData {
    ass_script: Option<AssScript>,
    file_path: String,
    font_cache: FontCache,
    font_override: Option<String>,
    available_fonts: Vec<String>,
    render_cache: RenderCache,
    dst_buf: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Per-effect globals
// ---------------------------------------------------------------------------

static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();

struct EffectData {
    suites: SuiteCache,
    settings_list: SettingsList<ZzzAssSubtitleFullSettings>,
    strings: StringCache<ZzzAssSubtitleFullSettings>,
    menu_item_strings: MenuItemCache<ZzzAssSubtitleFullSettings>,
}

static EFFECT_DATA: OnceLock<EffectData> = OnceLock::new();

fn data() -> OfxResult<&'static EffectData> {
    EFFECT_DATA.get().ok_or(OfxStat::kOfxStatFailed)
}

// ---------------------------------------------------------------------------
// File encoding helpers
// ---------------------------------------------------------------------------

/// Decode an ASS file from raw bytes, handling common encodings:
/// UTF-8 BOM, UTF-16 LE BOM, GBK (CP 936), or plain UTF-8.
fn decode_ass_file(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return String::from_utf8(bytes[3..].to_vec()).map_err(|e| e.to_string());
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let u16s: Vec<u16> = bytes[2..]
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16(&u16s).map_err(|e| e.to_string());
    }
    // Try UTF-8 first
    if let Ok(s) = String::from_utf8(bytes.to_vec()) {
        return Ok(s);
    }
    // Try GBK (CP 936) on Windows
    #[cfg(target_os = "windows")]
    {
        if let Some(s) = decode_codepage(bytes, 936) {
            return Ok(s);
        }
    }
    // Fall back to lossy UTF-8 (preserves ASCII structure, CJK garbled)
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(target_os = "windows")]
fn decode_codepage(bytes: &[u8], codepage: u32) -> Option<String> {
    unsafe {
        unsafe extern "system" {
            fn MultiByteToWideChar(
                CodePage: u32, dwFlags: u32,
                lpMultiByteStr: *const u8, cbMultiByte: i32,
                lpWideCharStr: *mut u16, cchWideChar: i32,
            ) -> i32;
        }
        let len = MultiByteToWideChar(codepage, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0);
        if len <= 0 { return None; }
        let mut wide: Vec<u16> = vec![0; len as usize];
        let ret = MultiByteToWideChar(codepage, 0, bytes.as_ptr(), bytes.len() as i32, wide.as_mut_ptr(), len);
        if ret <= 0 { return None; }
        String::from_utf16(&wide).ok()
    }
}

// ---------------------------------------------------------------------------
// Plugin info accessor
// ---------------------------------------------------------------------------

pub fn get_plugin() -> *const OfxPlugin {
    std::panic::set_hook(Box::new(|info| { println!("{info:?}"); }));
    let pi = PLUGIN_INFO.get_or_init(|| OfxPlugin {
        pluginApi: kOfxImageEffectPluginApi.as_ptr(),
        apiVersion: 1,
        pluginIdentifier: c"com.example:zzzAssSubtitle".as_ptr(),
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
    let settings_list = SettingsList::<ZzzAssSubtitleFullSettings>::new();
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

    ps(ep, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::EffectAssSubtitleName).as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginPropGrouping.as_ptr(), 0, c"zzzFX".as_ptr()).ofx_ok()?;
    ps(ep, kOfxPropPluginDescription.as_ptr(), 0, i18n::tr_cstr(TrKey::EffectAssSubtitleDesc).as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedContexts.as_ptr(), 0, kOfxImageEffectContextGenerator.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 0, kOfxBitDepthFloat.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 1, kOfxBitDepthShort.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 2, kOfxBitDepthByte.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginRenderThreadSafety.as_ptr(), 0, kOfxImageEffectRenderFullySafe.as_ptr()).ofx_ok()?;
    pi(ep, kOfxImageEffectPluginPropHostFrameThreading.as_ptr(), 0, 0).ofx_ok()?;
    pi(ep, kOfxImageEffectPropSupportsTiles.as_ptr(), 0, 1).ofx_ok()?;
    pi(ep, kOfxImageEffectPropTemporalClipAccess.as_ptr(), 0, 0).ofx_ok()?;
    Ok(())
}

unsafe fn action_describe_in_context(desc: OfxImageEffectHandle) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let cd = su.image_effect_suite.clipDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let gp = su.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = su.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pi = su.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let pd = su.property_suite.propSetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let pdef = su.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let defaults = ZzzAssSubtitleFullSettings::default();

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

    // --- Native PushButton: select_file ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypePushButton.as_ptr(), FILE_SELECT_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeSelectAssFile).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeSelectAssFileHint).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    }

    // --- Block A: Generic params before Position (time_offset_s, scale) ---
    for desc in d.settings_list.setting_descriptors.iter() {
        if desc.id.name == "position_x" { break; }
        define_single_param(su, param_set, desc, &defaults, PAGE_NAME, &d.strings, &d.menu_item_strings)?;
    }

    // --- Native Double2D: Position ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeDouble2D.as_ptr(), c"position".as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeAssPosition).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeAssPositionHint).as_ptr()).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 0.5).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 0.5).ofx_ok()?;
    }

    // --- Block B: Params between Position and Font Scale (blend_mode) ---
    let mut after_position = false;
    for desc in d.settings_list.setting_descriptors.iter() {
        if desc.id.name == "position_x" { after_position = true; continue; }
        if !after_position { continue; }
        if desc.id.name == "font_scale_x" { break; }
        if desc.id.name == "position_y" { continue; }
        define_single_param(su, param_set, desc, &defaults, PAGE_NAME, &d.strings, &d.menu_item_strings)?;
    }

    // --- Native Double2D: Font Scale ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeDouble2D.as_ptr(), c"font_scale".as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeAssFontScale).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeAssFontScaleHint).as_ptr()).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 1.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 1.0).ofx_ok()?;
    }

    // --- Block C: Remaining params after Font Scale (use_native_size) ---
    let mut after_font_scale = false;
    for desc in d.settings_list.setting_descriptors.iter() {
        if desc.id.name == "font_scale_x" { after_font_scale = true; continue; }
        if !after_font_scale || desc.id.name == "font_scale_y" { continue; }
        define_single_param(su, param_set, desc, &defaults, PAGE_NAME, &d.strings, &d.menu_item_strings)?;
    }

    // --- Native String (hidden): file_path ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeString.as_ptr(), FILE_PATH_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeFilePath).as_ptr()).ofx_ok()?;
        pi(pp, kOfxParamPropSecret.as_ptr(), 0, 1).ofx_ok()?;
        pi(pp, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
        ps(pp, kOfxParamPropDefault.as_ptr(), 0, c"".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    }

    // --- Native Choice: font_override_choice ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeChoice.as_ptr(), FONT_OVERRIDE_CHOICE_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeFontOverride).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeFontOverrideHint).as_ptr()).ofx_ok()?;
        // Option 0: "Use font from ASS file"
        ps(pp, kOfxParamPropChoiceOption.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeFontOverrideChoice).as_ptr()).ofx_ok()?;
        // Options 1..N: installed font names (cached globally, built once)
        let font_names = cached_font_names();
        let name_cstrs: Vec<CString> = font_names.iter().filter_map(|n| CString::new(n.as_str()).ok()).collect();
        for (i, name_cstr) in name_cstrs.iter().enumerate() {
            ps(pp, kOfxParamPropChoiceOption.as_ptr(), (i + 1) as i32, name_cstr.as_ptr()).ofx_ok()?;
        }
        pi(pp, kOfxParamPropDefault.as_ptr(), 0, 0).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    }

    // --- Native String (hidden): font_override_name ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeString.as_ptr(), FONT_OVERRIDE_STRING_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::ParamAssFontOverrideString).as_ptr()).ofx_ok()?;
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
        ass_script: None,
        file_path: String::new(),
        font_cache: FontCache::new(),
        font_override: None,
        available_fonts: cached_font_names().clone(),
        render_cache: RenderCache::new(),
        dst_buf: Vec::new(),
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
    let paramGetValueAtTime = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    let mut target_type: *mut c_char = ptr::null_mut();
    if propGetString(inArgs, kOfxPropType.as_ptr(), 0, &mut target_type).ofx_ok().is_err() {
        return Ok(()); // not an error we need to handle
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
                .add_filter("ASS Subtitles", &["ass", "ssa"])
                .pick_file()
            else { return Ok(()); };

            let path_str = path.to_string_lossy().to_string();

            // Read file bytes (handle UTF-8 / UTF-8 BOM / UTF-16 LE)
            let file_bytes = std::fs::read(&path).map_err(|_| OfxStat::kOfxStatFailed)?;
            let content = decode_ass_file(&file_bytes).map_err(|_| OfxStat::kOfxStatFailed)?;
            let ass_script = parse_ass_file(&content).map_err(|_| OfxStat::kOfxStatFailed)?;

            let mut ep: OfxPropertySetHandle = ptr::null_mut();
            (su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?)(effect, &mut ep).ofx_ok()?;
            let mut data_ptr: *mut c_void = ptr::null_mut();
            propGetPointer(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
            if !data_ptr.is_null() {
                let idata = &mut *(data_ptr as *mut InstanceData);
                idata.render_cache.invalidate_script_cache();
                idata.font_cache.invalidate();
                idata.ass_script = Some(ass_script);
                idata.file_path = path_str.clone();
            }

            let mut p: OfxParamHandle = ptr::null_mut();
            paramGetHandle(param_set, FILE_PATH_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
            if let Ok(path_cstr) = CString::new(path_str) {
                paramSetValue(p, path_cstr.as_ptr() as *const c_void).ofx_ok()?;
            }

            return Ok(());
        }

        if FONT_OVERRIDE_CHOICE_PARAM == CStr::from_ptr(target_name) {
            // Read the selected choice index
            let mut p: OfxParamHandle = ptr::null_mut();
            paramGetHandle(param_set, FONT_OVERRIDE_CHOICE_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
            let mut choice_idx: c_int = 0;
            paramGetValueAtTime(p, 0.0, &mut choice_idx).ofx_ok()?;

            let font_name = if choice_idx == 0 {
                String::new()
            } else {
                let mut ep: OfxPropertySetHandle = ptr::null_mut();
                (su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?)(effect, &mut ep).ofx_ok()?;
                let mut data_ptr: *mut c_void = ptr::null_mut();
                propGetPointer(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
                if !data_ptr.is_null() {
                    let idata = &*(data_ptr as *const InstanceData);
                    idata.available_fonts.get((choice_idx - 1) as usize).cloned().unwrap_or_default()
                } else {
                    String::new()
                }
            };

            // Update instance data
            let mut ep: OfxPropertySetHandle = ptr::null_mut();
            (su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?)(effect, &mut ep).ofx_ok()?;
            let mut data_ptr: *mut c_void = ptr::null_mut();
            propGetPointer(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
            if !data_ptr.is_null() {
                let idata = &mut *(data_ptr as *mut InstanceData);
                idata.font_override = if font_name.is_empty() { None } else { Some(font_name.clone()) };
            }

            // Persist to hidden string param
            let mut sp: OfxParamHandle = ptr::null_mut();
            paramGetHandle(param_set, FONT_OVERRIDE_STRING_PARAM.as_ptr(), &mut sp, ptr::null_mut()).ofx_ok()?;
            if let Ok(name_cstr) = CString::new(&*font_name) {
                paramSetValue(sp, name_cstr.as_ptr() as *const c_void).ofx_ok()?;
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

    let mut settings = ZzzAssSubtitleFullSettings::default();
    apply_params(param_set, time, &mut settings)?;

    // Retrieve instance data
    let gp = su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?;
    let gph = su.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    let mut ep: OfxPropertySetHandle = ptr::null_mut();
    gp(effect, &mut ep).ofx_ok()?;
    let mut data_ptr: *mut c_void = ptr::null_mut();
    gph(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
    let mut idata = if data_ptr.is_null() { None } else { Some(&mut *(data_ptr as *mut InstanceData)) };

    // Try to recover from hidden String param if instance data has no ASS file loaded
    if matches!(&idata, None) || idata.as_deref().is_some_and(|i| i.ass_script.is_none()) {
        let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
        let pgvt = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, FILE_PATH_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut path_ptr: *mut c_char = ptr::null_mut();
        if pgvt(p, 0.0, &mut path_ptr).ofx_ok().is_ok() && !path_ptr.is_null() {
            let path = CStr::from_ptr(path_ptr).to_string_lossy();
            if !path.is_empty() {
                if let Ok(file_bytes) = std::fs::read(std::path::Path::new(path.as_ref())) {
                    if let Ok(content) = decode_ass_file(&file_bytes) {
                        if let Ok(ass_script) = parse_ass_file(&content) {
                            if let Some(idata) = &mut idata {
                                idata.render_cache.invalidate_script_cache();
                                idata.font_cache.invalidate();
                                idata.ass_script = Some(ass_script);
                                idata.file_path = path.into_owned();
                            }
                        }
                    }
                }
            }
        }
    }

    // Try to recover font_override from hidden string param
    if let Some(ref mut idata_inner) = idata {
        if idata_inner.font_override.is_none() {
            let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
            let pgvt = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
            let mut p: OfxParamHandle = ptr::null_mut();
            pgh(param_set, FONT_OVERRIDE_STRING_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
            let mut name_ptr: *mut c_char = ptr::null_mut();
            if pgvt(p, 0.0, &mut name_ptr).ofx_ok().is_ok() && !name_ptr.is_null() {
                let name = CStr::from_ptr(name_ptr).to_string_lossy();
                if !name.is_empty() {
                    idata_inner.font_override = Some(name.into_owned());
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

    // Read project frame rate for time normalization.
    // Some hosts (e.g. VEGAS Pro) give generator plugins a fixed 1000 fps
    // instead of the real project frame rate. Using time / rate normalises
    // the timeline, matching the approach in SpriteSheet.
    let mut frame_rate: f64 = 30.0;
    let _ = pgd(ep, kOfxImageEffectPropFrameRate.as_ptr(), 0, &mut frame_rate);
    let rate = if frame_rate > 0.0 { frame_rate } else { 30.0 };

    let ss: ZzzAssSubtitle = (&settings).into();
    // Normalize time: time may be frame numbers (VEGAS) or seconds.
    // Dividing by frame_rate converts to seconds regardless of what the host
    // passes, because if time=frameN and rate=1000 (fake), the ratio still
    // gives seconds-like units. The user can compensate with time_offset_s.
    let time_ms = (time / rate * 1000.0) as i64 + (ss.time_offset_s * 1000.0) as i64;

    // Phase A: Render into reusable buffer
    {
        if let Some(ref mut idata_inner) = idata {
            let buf_size = width * height * 4;
            if idata_inner.dst_buf.len() != buf_size {
                idata_inner.dst_buf.resize(buf_size, 0);
            }
            // Shrink if significantly over-allocated after a resolution decrease
            if idata_inner.dst_buf.capacity() > buf_size * 2 {
                idata_inner.dst_buf.shrink_to(buf_size);
            }
            idata_inner.dst_buf.fill(0);

            if let Some(ref ass_script) = idata_inner.ass_script {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    render_ass_subtitle_frame(
                        ass_script,
                        time_ms,
                        &mut idata_inner.font_cache,
                        ss.scale,
                        ss.position_x,
                        ss.position_y,
                        ss.font_scale_x,
                        ss.font_scale_y,
                        ss.blend_mode,
                        idata_inner.font_override.as_deref(),
                        ss.use_native_size,
                        &mut idata_inner.dst_buf,
                        width,
                        height,
                        &mut idata_inner.render_cache,
                    );
                }));
            }
        }
    }

    // Phase B: Get dst slice for post-processing
    let mut fallback_dst;
    let dst_buf: &mut [u8] = if let Some(ref mut idata_inner) = idata {
        &mut idata_inner.dst_buf[..]
    } else {
        fallback_dst = vec![0u8; width * height * 4];
        &mut fallback_dst[..]
    };

    // Premultiply alpha (matching sprite_sheet.rs convention)
    if num_components == 4 {
        for pixel in dst_buf.chunks_exact_mut(4) {
            let a = pixel[3];
            if a == 0 {
                pixel[0] = 0;
                pixel[1] = 0;
                pixel[2] = 0;
            } else if a != 255 {
                let a32 = a as u32;
                pixel[0] = ((pixel[0] as u32 * a32 + 127) / 255) as u8;
                pixel[1] = ((pixel[1] as u32 * a32 + 127) / 255) as u8;
                pixel[2] = ((pixel[2] as u32 * a32 + 127) / 255) as u8;
            }
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
// Parameter reading
// ---------------------------------------------------------------------------

unsafe fn apply_params(
    param_set: OfxParamSetHandle,
    time: f64,
    dst: &mut ZzzAssSubtitleFullSettings,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;

    let find_id = |name: &str| -> OfxResult<SettingID<ZzzAssSubtitleFullSettings>> {
        d.settings_list.setting_descriptors.iter()
            .find(|desc| desc.id.name == name)
            .map(|desc| desc.id.clone())
            .ok_or(OfxStat::kOfxStatFailed)
    };

    // Read all generic params, skipping those handled by native Double2D controls
    for desc in d.settings_list.setting_descriptors.iter() {
        if matches!(desc.id.name, "position_x" | "position_y" | "font_scale_x" | "font_scale_y") {
            continue;
        }
        if let SettingKind::Group { .. } = &desc.kind {
            let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
            let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
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

    // --- Native Double2D: Position ---
    {
        let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
        let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, c"position".as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut x: f64 = 0.5;
        let mut y: f64 = 0.5;
        pgv(p, time, &mut x, &mut y).ofx_ok()?;
        dst.set_field::<f32>(&find_id("position_x")?, x.clamp(0.0, 1.0) as f32).unwrap();
        dst.set_field::<f32>(&find_id("position_y")?, y.clamp(0.0, 1.0) as f32).unwrap();
    }

    // --- Native Double2D: Font Scale ---
    {
        let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
        let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, c"font_scale".as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut x: f64 = 1.0;
        let mut y: f64 = 1.0;
        pgv(p, time, &mut x, &mut y).ofx_ok()?;
        dst.set_field::<f32>(&find_id("font_scale_x")?, (x as f32).clamp(0.01, 5.0)).unwrap();
        dst.set_field::<f32>(&find_id("font_scale_y")?, (y as f32).clamp(0.01, 5.0)).unwrap();
    }

    Ok(())
}
