use std::{
    cell::RefCell,
    ffi::{CStr, CString, c_char, c_int, c_void},
    ptr,
    sync::OnceLock,
};

use zzzfx::ass_subtitle::FontCache;
use zzzfx::latex_display::CachedLaTeX;
use zzzfx::{
    LaTeXDisplay, LaTeXDisplayFullSettings,
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

const FORMULA_PARAM: &CStr = c"formula";
const FONT_CHOICE_PARAM: &CStr = c"font_choice";
const FONT_NAME_PARAM: &CStr = c"font_name";
const TEXT_COLOR_PARAM: &CStr = c"text_color";
const BACKGROUND_COLOR_PARAM: &CStr = c"background_color";
const POSITION_PARAM: &CStr = c"position";
const PAGE_NAME: &CStr = c"Controls";

fn is_native_grouped_name(name: &str) -> bool {
    matches!(name, "position_x" | "position_y" | "font_choice" | "font_name")
}

// ---------------------------------------------------------------------------
// Instance data
// ---------------------------------------------------------------------------

const INSTANCE_MAGIC: u64 = 0x14E7_14E7_14E7_14E7;

struct InstanceData {
    magic: u64,
    formula: String,
    font_name: String,
    svg_bytes: Vec<u8>,
    cached: Option<CachedLaTeX>,
}

impl InstanceData {
    fn _assert_used(&self) {
        let _ = &self.magic;
        let _ = &self.formula;
        let _ = &self.font_name;
        let _ = &self.svg_bytes;
        let _ = &self.cached;
    }
}

thread_local! {
    static RENDER_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

// ---------------------------------------------------------------------------
// Cached font names (reuses ASS Subtitle's global font enumeration)
// ---------------------------------------------------------------------------

fn cached_font_names() -> &'static Vec<String> {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let cache = FontCache::new();
        cache.list_font_names()
    })
}

// ---------------------------------------------------------------------------
// Per-effect globals
// ---------------------------------------------------------------------------

static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();

struct EffectData {
    suites: SuiteCache,
    settings_list: SettingsList<LaTeXDisplayFullSettings>,
    strings: StringCache<LaTeXDisplayFullSettings>,
    menu_item_strings: MenuItemCache<LaTeXDisplayFullSettings>,
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
        pluginIdentifier: c"io.github.zzzEffect:LaTeXDisplay".as_ptr(),
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
    let settings_list = SettingsList::<LaTeXDisplayFullSettings>::new();
    i18n::set_lang(i18n::detect_system_lang());
    let (strings, menu_item_strings) = build_string_cache(&settings_list);

    let _ = cached_font_names(); // pre-populate font cache on load
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

    ps(ep, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(zzzfx::settings::TrKey::EffectLaTeXDisplayName).as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginPropGrouping.as_ptr(), 0, c"zzzFX".as_ptr()).ofx_ok()?;
    ps(ep, kOfxPropPluginDescription.as_ptr(), 0, i18n::tr_cstr(zzzfx::settings::TrKey::EffectLaTeXDisplayDesc).as_ptr()).ofx_ok()?;
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
    let defaults = LaTeXDisplayFullSettings::default();

    // --- Output clip only (Generator) ---
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
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(zzzfx::settings::TrKey::NativeControls).as_ptr()).ofx_ok()?;
    }

    // --- Native String: formula (multiline) ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeString.as_ptr(), FORMULA_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(zzzfx::settings::TrKey::NativeLaTeXFormula).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(zzzfx::settings::TrKey::NativeLaTeXFormulaHint).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropDefault.as_ptr(), 0, c"".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropStringMode.as_ptr(), 0, kOfxParamStringIsMultiLine.as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    }

    // --- Native Choice: font (before font_size generic param) ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeChoice.as_ptr(), FONT_CHOICE_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(zzzfx::settings::TrKey::NativeLaTeXFontChoice).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(zzzfx::settings::TrKey::NativeLaTeXFontChoiceHint).as_ptr()).ofx_ok()?;
        pi(pp, kOfxParamPropDefault.as_ptr(), 0, 0).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
        // Option 0: auto-detect
        ps(pp, kOfxParamPropChoiceOption.as_ptr(), 0, c"LatModernMath".as_ptr()).ofx_ok()?;
        // Options 1+: system fonts
        let font_names = cached_font_names();
        let name_cstrs: Vec<CString> = font_names
            .iter()
            .filter_map(|n| CString::new(n.as_str()).ok())
            .collect();
        for (i, name_cstr) in name_cstrs.iter().enumerate() {
            ps(pp, kOfxParamPropChoiceOption.as_ptr(), (i + 1) as c_int, name_cstr.as_ptr().cast()).ofx_ok()?;
        }
    }

    // --- Hidden String: font_name (persistence) ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeString.as_ptr(), FONT_NAME_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        pi(pp, kOfxParamPropSecret.as_ptr(), 0, 1).ofx_ok()?;
        pi(pp, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
        ps(pp, kOfxParamPropDefault.as_ptr(), 0, c"".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    }

    // --- Native RGBA: textColor (default white) ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeRGBA.as_ptr(), TEXT_COLOR_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(zzzfx::settings::TrKey::NativeLaTeXTextColor).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(zzzfx::settings::TrKey::NativeLaTeXTextColorHint).as_ptr()).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 1.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 1.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 2, 1.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 3, 1.0).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    }

    // --- Native RGBA: backgroundColor (default transparent) ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeRGBA.as_ptr(), BACKGROUND_COLOR_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(zzzfx::settings::TrKey::NativeLaTeXBackgroundColor).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(zzzfx::settings::TrKey::NativeLaTeXBackgroundColorHint).as_ptr()).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 0.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 0.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 2, 0.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 3, 0.0).ofx_ok()?;
        pi(pp, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    }

    // --- Interleave native Double2D: Position ---
    let mut defined_position = false;

    for desc in d.settings_list.setting_descriptors.iter() {
        if !defined_position && desc.id.name == "position_x" {
            define_native_double2d(
                su, param_set, POSITION_PARAM,
                i18n::tr_cstr(zzzfx::settings::TrKey::NativeLaTeXPosition),
                i18n::tr_cstr(zzzfx::settings::TrKey::NativeLaTeXPositionHint),
                defaults.position_x as f64, defaults.position_y as f64,
                0.0, 1.0,
            )?;
            defined_position = true;
        }
        if is_native_grouped_name(desc.id.name) { continue; }
        define_single_param(su, param_set, desc, &defaults, PAGE_NAME, &d.strings, &d.menu_item_strings)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Native Double2D helper
// ---------------------------------------------------------------------------

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
        formula: String::new(),
        font_name: String::new(),
        svg_bytes: Vec::new(),
        cached: None,
    });
    idata._assert_used();
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
// GetClipPreferences
// ---------------------------------------------------------------------------

unsafe fn action_get_clip_preferences(outArgs: OfxPropertySetHandle) -> OfxResult<()> {
    action_get_clip_preferences_common(&data()?.suites, outArgs, 0, kOfxImagePreMultiplied)
}

// ---------------------------------------------------------------------------
// InstanceChanged (formula parameter changed — invalidate cache)
// ---------------------------------------------------------------------------

unsafe fn action_instance_changed(
    effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;

    let propGetString = su.property_suite.propGetString.ok_or(OfxStat::kOfxStatFailed)?;
    let mut target_type: *mut c_char = ptr::null_mut();
    if propGetString(inArgs, kOfxPropType.as_ptr(), 0, &mut target_type).ofx_ok().is_err() {
        return Ok(());
    }
    if target_type.is_null() { return Ok(()); }

    if CStr::from_ptr(target_type) == kOfxTypeParameter {
        let mut target_name: *mut c_char = ptr::null_mut();
        if propGetString(inArgs, kOfxPropName.as_ptr(), 0, &mut target_name).ofx_ok().is_err() {
            return Ok(());
        }
        if target_name.is_null() { return Ok(()); }

        if FORMULA_PARAM == CStr::from_ptr(target_name) {
            // Invalidate cache on formula change (will be rebuilt on next render)
            invalidate_cache(su, effect)?;
        }

        if FONT_CHOICE_PARAM == CStr::from_ptr(target_name) {
            // Read choice index, resolve font name, persist to hidden param
            let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
            let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
            let psv = su.parameter_suite.paramSetValue.ok_or(OfxStat::kOfxStatFailed)?;
            let getParamSet = su.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;

            let mut param_set: OfxParamSetHandle = ptr::null_mut();
            getParamSet(effect, &mut param_set).ofx_ok()?;

            let mut p: OfxParamHandle = ptr::null_mut();
            pgh(param_set, FONT_CHOICE_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
            let mut idx: c_int = 0;
            pgv(p, 0.0, &mut idx).ofx_ok()?;

            let font_name = if idx > 0 {
                let font_names = cached_font_names();
                font_names.get((idx - 1) as usize).cloned().unwrap_or_default()
            } else {
                String::from("Latin Modern Math, STIX Two Math, XITS Math, Cambria Math, serif")
            };

            // Persist to hidden string param via paramSetValue
            let mut pp: OfxParamHandle = ptr::null_mut();
            pgh(param_set, FONT_NAME_PARAM.as_ptr(), &mut pp, ptr::null_mut()).ofx_ok()?;
            let name_cstr = CString::new(font_name.as_str()).unwrap_or_default();
            psv(pp, name_cstr.as_ptr() as *const c_void).ofx_ok()?;

            invalidate_cache(su, effect)?;
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

    let mut settings = LaTeXDisplayFullSettings::default();
    let (formula, font_name, bg_r, bg_g, bg_b, bg_a, text_r, text_g, text_b, text_a, pos_x, pos_y) =
        apply_params(param_set, time, &mut settings)?;
    let core_settings: LaTeXDisplay = (&settings).into();

    // Retrieve instance data
    let gp = su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?;
    let gph = su.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    let mut ep: OfxPropertySetHandle = ptr::null_mut();
    gp(effect, &mut ep).ofx_ok()?;
    let mut data_ptr: *mut c_void = ptr::null_mut();
    gph(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
    let idata = if data_ptr.is_null() {
        None
    } else {
        let idata_ref = unsafe { &*(data_ptr as *const InstanceData) };
        if idata_ref.magic != INSTANCE_MAGIC {
            return Err(OfxStat::kOfxStatFailed);
        }
        Some(unsafe { &mut *(data_ptr as *mut InstanceData) })
    };

    // Get output clip
    let mut dc: OfxImageClipHandle = ptr::null_mut();
    let mut _clip_props: OfxPropertySetHandle = ptr::null_mut();
    cgh(effect, c"Output".as_ptr(), &mut dc, &mut _clip_props).ofx_ok()?;

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

    // Use thread-local buffer to avoid per-frame allocation (C-2)
    let buf_size = width * height * 4;
    let bg = [bg_r, bg_g, bg_b, bg_a];
    let text_color = [text_r, text_g, text_b, text_a];

    RENDER_BUF.with_borrow_mut(|buf| {
        buf.resize(buf_size, 0u8);
        let dst_buf = buf.as_mut_slice();

        let rendered = if let Some(idata) = idata {
            if !formula.is_empty() {
                let new_cache = zzzfx::latex_display::render_latex(
                    &formula,
                    &font_name,
                    idata.cached.as_ref(),
                    &core_settings,
                    pos_x,
                    pos_y,
                    dst_buf,
                    width,
                    height,
                    bg,
                    text_color,
                );
                match new_cache {
                    Some(c) => {
                        idata.cached = Some(c);
                        true
                    }
                    None => false,
                }
            } else {
                fill_buf_bg(dst_buf, bg);
                true
            }
        } else {
            false
        };

        // C-3: Fall back to background color on render failure
        if !rendered {
            fill_buf_bg(dst_buf, bg);
        }

        // Pre-multiply alpha
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

        Ok::<_, OfxStatus>(())
    })?;

    cri(di).ofx_ok()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Background fill helper
// ---------------------------------------------------------------------------

fn fill_buf_bg(buf: &mut [u8], bg: [f32; 4]) {
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

// ---------------------------------------------------------------------------
// Cache invalidation helper
// ---------------------------------------------------------------------------

unsafe fn invalidate_cache(su: &SuiteCache, effect: OfxImageEffectHandle) -> OfxResult<()> {
    let gp = su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?;
    let propGetPointer = su.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    let mut ep: OfxPropertySetHandle = ptr::null_mut();
    gp(effect, &mut ep).ofx_ok()?;
    let mut data_ptr: *mut c_void = ptr::null_mut();
    propGetPointer(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
    if !data_ptr.is_null() {
        let idata = &mut *(data_ptr as *mut InstanceData);
        idata.cached = None;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Parameter reading
// ---------------------------------------------------------------------------

unsafe fn apply_params(
    param_set: OfxParamSetHandle,
    time: f64,
    dst: &mut LaTeXDisplayFullSettings,
) -> OfxResult<(String, String, f32, f32, f32, f32, f32, f32, f32, f32, f32, f32)> {
    let d = data()?;
    let su = &d.suites;
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    let find_id = |name: &str| -> OfxResult<SettingID<LaTeXDisplayFullSettings>> {
        d.settings_list.all_descriptors()
            .find(|d| d.id.name == name)
            .map(|d| d.id.clone())
            .ok_or(OfxStat::kOfxStatFailed)
    };

    // Read native Choice: font_choice → resolve font name
    let font_name = {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, FONT_CHOICE_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut idx: c_int = 0;
        pgv(p, time, &mut idx).ofx_ok()?;
        if idx > 0 {
            let font_names = cached_font_names();
            font_names.get((idx - 1) as usize).cloned().unwrap_or_else(|| {
                String::from("Latin Modern Math, STIX Two Math, XITS Math, Cambria Math, serif")
            })
        } else {
            String::from("Latin Modern Math, STIX Two Math, XITS Math, Cambria Math, serif")
        }
    };

    // Read native String: formula
    let formula = {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, FORMULA_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut s_ptr: *mut c_char = ptr::null_mut();
        pgv(p, time, &mut s_ptr).ofx_ok()?;
        if s_ptr.is_null() {
            String::new()
        } else {
            CStr::from_ptr(s_ptr).to_string_lossy().into_owned()
        }
    };

    // Read native RGBA: textColor
    let (text_r, text_g, text_b, text_a) = {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, TEXT_COLOR_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut r: f64 = 0.0; let mut g: f64 = 0.0;
        let mut b: f64 = 0.0; let mut a: f64 = 0.0;
        pgv(p, time, &mut r, &mut g, &mut b, &mut a).ofx_ok()?;
        (r as f32, g as f32, b as f32, a as f32)
    };

    // Read native RGBA: backgroundColor
    let (bg_r, bg_g, bg_b, bg_a) = {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, BACKGROUND_COLOR_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut r: f64 = 0.0; let mut g: f64 = 0.0;
        let mut b: f64 = 0.0; let mut a: f64 = 0.0;
        pgv(p, time, &mut r, &mut g, &mut b, &mut a).ofx_ok()?;
        (r as f32, g as f32, b as f32, a as f32)
    };

    // Read native Double2D: position (Y flipped)
    let (pos_x, pos_y) = {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, POSITION_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut x: f64 = 0.5; let mut y: f64 = 0.5;
        pgv(p, time, &mut x, &mut y).ofx_ok()?;
        let y_flipped = 1.0 - y;
        dst.set_field::<f32>(&find_id("position_x")?, x.clamp(0.0, 1.0) as f32).unwrap();
        dst.set_field::<f32>(&find_id("position_y")?, y_flipped.clamp(0.0, 1.0) as f32).unwrap();
        (x as f32, y_flipped as f32)
    };

    // Read generic params
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

    Ok((formula, font_name, bg_r, bg_g, bg_b, bg_a, text_r, text_g, text_b, text_a, pos_x, pos_y))
}
