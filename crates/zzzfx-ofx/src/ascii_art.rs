use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    ptr,
    sync::OnceLock,
};

use zzzfx_core::settings::{SettingKind, TrKey};
use zzzfx_core::{
    ZzzAsciiArt, ZzzAsciiArtFullSettings,
    settings::{Settings, SettingsList},
};
use zzzfx_core::ass_subtitle::FontCache;

use crate::bindings::*;
use crate::i18n;
use crate::shared::{
    HostInfo, SuiteCache, StringCache, MenuItemCache, build_string_cache,
    define_single_param, read_generic_param, copy_source_to_u8, copy_u8_to_output,
    detect_pixel_depth, action_load_common, action_get_clip_preferences_common,
    action_get_regions_of_interest_common,
};

// ---------------------------------------------------------------------------
// Native OFX parameter names
// ---------------------------------------------------------------------------

const FONT_CHOICE_PARAM: &CStr = c"font_choice";
const FONT_NAME_PARAM: &CStr = c"font_name";
const CUSTOM_CHARS_PARAM: &CStr = c"custom_chars";

fn is_native_grouped_name(name: &str) -> bool {
    matches!(name, "font_choice" | "font_name" | "custom_chars")
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
    settings_list: SettingsList<ZzzAsciiArtFullSettings>,
    strings: StringCache<ZzzAsciiArtFullSettings>,
    menu_item_strings: MenuItemCache<ZzzAsciiArtFullSettings>,
    available_fonts: Vec<String>,
}

static EFFECT_DATA: OnceLock<EffectData> = OnceLock::new();

fn data() -> OfxResult<&'static EffectData> {
    EFFECT_DATA.get().ok_or(OfxStat::kOfxStatFailed)
}

// ---------------------------------------------------------------------------
// Plugin info accessor
// ---------------------------------------------------------------------------

pub fn get_plugin() -> *const OfxPlugin {
    std::panic::set_hook(Box::new(|info| {
        println!("{info:?}");
    }));
    let pi = PLUGIN_INFO.get_or_init(|| OfxPlugin {
        pluginApi: kOfxImageEffectPluginApi.as_ptr(),
        apiVersion: 1,
        pluginIdentifier: c"com.example:zzzAsciiArt".as_ptr(),
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

    let host_info = HostInfo {
        host: h,
        fetch_suite: fs,
    };
    let suites = SuiteCache::new(host_info)?;
    let settings_list = SettingsList::<ZzzAsciiArtFullSettings>::new();
    i18n::set_lang(i18n::detect_system_lang());
    let (strings, menu_item_strings) = build_string_cache(&settings_list);

    let available_fonts = cached_font_names().clone();
    EFFECT_DATA.get_or_init(|| EffectData {
        suites,
        settings_list,
        strings,
        menu_item_strings,
        available_fonts,
    });
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
    if action.is_null() {
        return OfxStat::kOfxStatFailed;
    }
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
    (su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?)(
        desc, &mut ep,
    )
    .ofx_ok()?;
    let ps = su.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pi = su.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;

    ps(
        ep,
        kOfxPropLabel.as_ptr(),
        0,
        i18n::tr_cstr(TrKey::EffectAsciiArtName).as_ptr(),
    )
    .ofx_ok()?;
    ps(
        ep,
        kOfxImageEffectPluginPropGrouping.as_ptr(),
        0,
        c"zzzFX".as_ptr(),
    )
    .ofx_ok()?;
    ps(
        ep,
        kOfxPropPluginDescription.as_ptr(),
        0,
        i18n::tr_cstr(TrKey::EffectAsciiArtDesc).as_ptr(),
    )
    .ofx_ok()?;
    ps(
        ep,
        kOfxImageEffectPropSupportedContexts.as_ptr(),
        0,
        kOfxImageEffectContextFilter.as_ptr(),
    )
    .ofx_ok()?;
    ps(
        ep,
        kOfxImageEffectPropSupportedContexts.as_ptr(),
        1,
        kOfxImageEffectContextGeneral.as_ptr(),
    )
    .ofx_ok()?;
    ps(
        ep,
        kOfxImageEffectPropSupportedPixelDepths.as_ptr(),
        0,
        kOfxBitDepthFloat.as_ptr(),
    )
    .ofx_ok()?;
    ps(
        ep,
        kOfxImageEffectPropSupportedPixelDepths.as_ptr(),
        1,
        kOfxBitDepthShort.as_ptr(),
    )
    .ofx_ok()?;
    ps(
        ep,
        kOfxImageEffectPropSupportedPixelDepths.as_ptr(),
        2,
        kOfxBitDepthByte.as_ptr(),
    )
    .ofx_ok()?;
    ps(
        ep,
        kOfxImageEffectPluginRenderThreadSafety.as_ptr(),
        0,
        kOfxImageEffectRenderFullySafe.as_ptr(),
    )
    .ofx_ok()?;
    pi(
        ep,
        kOfxImageEffectPluginPropHostFrameThreading.as_ptr(),
        0,
        0,
    )
    .ofx_ok()?;
    pi(
        ep,
        kOfxImageEffectPropSupportsTiles.as_ptr(),
        0,
        0,
    )
    .ofx_ok()?;
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
    let defaults = ZzzAsciiArtFullSettings::default();

    // --- Output / Source clips ---
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    cd(desc, c"Output".as_ptr(), &mut props).ofx_ok()?;
    ps(
        props,
        kOfxImageEffectPropSupportedComponents.as_ptr(),
        0,
        kOfxImageComponentRGBA.as_ptr(),
    )
    .ofx_ok()?;
    ps(
        props,
        kOfxImageEffectPropSupportedComponents.as_ptr(),
        1,
        kOfxImageComponentRGB.as_ptr(),
    )
    .ofx_ok()?;
    cd(desc, c"Source".as_ptr(), &mut props).ofx_ok()?;
    ps(
        props,
        kOfxImageEffectPropSupportedComponents.as_ptr(),
        0,
        kOfxImageComponentRGBA.as_ptr(),
    )
    .ofx_ok()?;
    ps(
        props,
        kOfxImageEffectPropSupportedComponents.as_ptr(),
        1,
        kOfxImageComponentRGB.as_ptr(),
    )
    .ofx_ok()?;

    // --- Parameter set ---
    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gp(desc, &mut param_set).ofx_ok()?;

    let pen = CString::new(i18n::tr(TrKey::CommonEnabled))
        .unwrap_or_else(|_| CString::new("Enabled").unwrap());

    // --- Generic params (skip native params; handle Group specially) ---
    for desc in d.settings_list.setting_descriptors.iter() {
        if is_native_grouped_name(desc.id.name) {
            continue;
        }
        if let SettingKind::Group { children } = &desc.kind {
            let ds = d.strings.get(&desc.id).unwrap();
            let id_cstr = ds.0.as_c_str();
            let gnc = ds.3.as_ref().expect("group name").as_c_str();
            let dv = defaults
                .get_field::<bool>(&desc.id)
                .map_err(|_| OfxStat::kOfxStatFailed)?;
            // Group container
            let mut gp: OfxPropertySetHandle = ptr::null_mut();
            pdef(
                param_set,
                kOfxParamTypeGroup.as_ptr(),
                gnc.as_ptr(),
                &mut gp,
            )
            .ofx_ok()?;
            ps(gp, kOfxPropLabel.as_ptr(), 0, ds.1.as_ptr()).ofx_ok()?;
            if let Some(desc_text) = ds.2.as_deref() {
                ps(gp, kOfxParamPropHint.as_ptr(), 0, desc_text.as_ptr()).ofx_ok()?;
            }
            // Enabled checkbox (hidden)
            let mut cb: OfxPropertySetHandle = ptr::null_mut();
            pdef(
                param_set,
                kOfxParamTypeBoolean.as_ptr(),
                id_cstr.as_ptr(),
                &mut cb,
            )
            .ofx_ok()?;
            ps(cb, kOfxPropLabel.as_ptr(), 0, pen.as_ptr()).ofx_ok()?;
            pi(cb, kOfxParamPropDefault.as_ptr(), 0, dv as i32).ofx_ok()?;
            ps(cb, kOfxParamPropParent.as_ptr(), 0, gnc.as_ptr()).ofx_ok()?;
            pi(cb, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
            pi(cb, kOfxParamPropSecret.as_ptr(), 0, 1).ofx_ok()?;
            // Children inside group
            for child in children {
                define_single_param(
                    su,
                    param_set,
                    child,
                    &defaults,
                    gnc,
                    &d.strings,
                    &d.menu_item_strings,
                )?;
            }
            // Native String (hidden): custom_chars, inside the group
            {
                let mut pp: OfxPropertySetHandle = ptr::null_mut();
                pdef(
                    param_set,
                    kOfxParamTypeString.as_ptr(),
                    CUSTOM_CHARS_PARAM.as_ptr(),
                    &mut pp,
                )
                .ofx_ok()?;
                pi(pp, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
                ps(
                    pp,
                    kOfxPropLabel.as_ptr(),
                    0,
                    i18n::tr_cstr(TrKey::NativeAsciiCustomChars).as_ptr(),
                )
                .ofx_ok()?;
                ps(
                    pp,
                    kOfxParamPropHint.as_ptr(),
                    0,
                    i18n::tr_cstr(TrKey::NativeAsciiCustomCharsHint).as_ptr(),
                )
                .ofx_ok()?;
                let empty = CString::new("").unwrap_or_else(|_| CString::new("").unwrap());
                ps(pp, kOfxParamPropDefault.as_ptr(), 0, empty.as_ptr()).ofx_ok()?;
                ps(pp, kOfxParamPropParent.as_ptr(), 0, gnc.as_ptr()).ofx_ok()?;
            }
        } else {
            define_single_param(
                su,
                param_set,
                desc,
                &defaults,
                c"",
                &d.strings,
                &d.menu_item_strings,
            )?;
        }
    }

    // --- Native Choice: Font selection ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(
            param_set,
            kOfxParamTypeChoice.as_ptr(),
            FONT_CHOICE_PARAM.as_ptr(),
            &mut pp,
        )
        .ofx_ok()?;
        ps(
            pp,
            kOfxPropLabel.as_ptr(),
            0,
            i18n::tr_cstr(TrKey::NativeAsciiFontChoice).as_ptr(),
        )
        .ofx_ok()?;
        ps(
            pp,
            kOfxParamPropHint.as_ptr(),
            0,
            i18n::tr_cstr(TrKey::NativeAsciiFontChoiceHint).as_ptr(),
        )
        .ofx_ok()?;
        // Option 0: Auto-detect
        ps(
            pp,
            kOfxParamPropChoiceOption.as_ptr(),
            0,
            i18n::tr_cstr(TrKey::NativeAsciiFontAutoDetect).as_ptr(),
        )
        .ofx_ok()?;
        // Options 1..N: installed font names
        let font_names = cached_font_names();
        let name_cstrs: Vec<CString> = font_names
            .iter()
            .filter_map(|n| CString::new(n.as_str()).ok())
            .collect();
        for (i, name_cstr) in name_cstrs.iter().enumerate() {
            ps(
                pp,
                kOfxParamPropChoiceOption.as_ptr(),
                (i + 1) as i32,
                name_cstr.as_ptr(),
            )
            .ofx_ok()?;
        }
        let pi2 = d.suites.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
        pi2(pp, kOfxParamPropDefault.as_ptr(), 0, 0).ofx_ok()?;
    }

    // --- Native String (hidden): font_name persistence ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(
            param_set,
            kOfxParamTypeString.as_ptr(),
            FONT_NAME_PARAM.as_ptr(),
            &mut pp,
        )
        .ofx_ok()?;
        let pi2 = d.suites.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
        pi2(pp, kOfxParamPropSecret.as_ptr(), 0, 1).ofx_ok()?;
        pi2(pp, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
        let empty = CString::new("").unwrap_or_else(|_| CString::new("").unwrap());
        ps(pp, kOfxParamPropDefault.as_ptr(), 0, empty.as_ptr()).ofx_ok()?;
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

unsafe fn action_instance_changed(
    effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let pgs = su.property_suite.propGetString.ok_or(OfxStat::kOfxStatFailed)?;
    let gps = su.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
    let psv = su.parameter_suite.paramSetValue.ok_or(OfxStat::kOfxStatFailed)?;

    // Get the name of the changed parameter
    let mut target_name: *mut c_char = ptr::null_mut();
    if pgs(inArgs, kOfxPropName.as_ptr(), 0, &mut target_name).ofx_ok().is_err()
        || target_name.is_null()
    {
        return Ok(());
    }

    if FONT_CHOICE_PARAM == CStr::from_ptr(target_name) {
        let mut param_set: OfxParamSetHandle = ptr::null_mut();
        gps(effect, &mut param_set).ofx_ok()?;

        // Read the selected choice index
        let mut cp: OfxParamHandle = ptr::null_mut();
        pgh(
            param_set,
            FONT_CHOICE_PARAM.as_ptr(),
            &mut cp,
            ptr::null_mut(),
        )
        .ofx_ok()?;
        let mut choice_idx: c_int = 0;
        pgv(cp, 0.0, &mut choice_idx).ofx_ok()?;

        let font_name = if choice_idx == 0 {
            String::new()
        } else {
            d.available_fonts
                .get((choice_idx - 1) as usize)
                .cloned()
                .unwrap_or_default()
        };

        // Persist to hidden string param
        let mut sp: OfxParamHandle = ptr::null_mut();
        pgh(
            param_set,
            FONT_NAME_PARAM.as_ptr(),
            &mut sp,
            ptr::null_mut(),
        )
        .ofx_ok()?;
        if let Ok(name_cstr) = CString::new(&*font_name) {
            psv(sp, name_cstr.as_ptr() as *const c_void).ofx_ok()?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// IsIdentity
// ---------------------------------------------------------------------------

unsafe fn action_is_identity(
    _effect: OfxImageEffectHandle,
    _inArgs: OfxPropertySetHandle,
    _outArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    Err(OfxStat::kOfxStatReplyDefault)
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

    let mut settings = ZzzAsciiArtFullSettings::default();
    apply_params(param_set, time, &mut settings)?;
    let ascii_art: ZzzAsciiArt = (&settings).into();

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

    let mut srb: c_int = 0;
    let mut drb: c_int = 0;
    pgi(si, kOfxImagePropRowBytes.as_ptr(), 0, &mut srb).ofx_ok()?;
    pgi(di, kOfxImagePropRowBytes.as_ptr(), 0, &mut drb).ofx_ok()?;

    let mut l: c_int = 0;
    let mut b: c_int = 0;
    let mut r: c_int = 0;
    let mut t: c_int = 0;
    pgi(si, kOfxImagePropBounds.as_ptr(), 0, &mut l).ofx_ok()?;
    pgi(si, kOfxImagePropBounds.as_ptr(), 1, &mut b).ofx_ok()?;
    pgi(si, kOfxImagePropBounds.as_ptr(), 2, &mut r).ofx_ok()?;
    pgi(si, kOfxImagePropBounds.as_ptr(), 3, &mut t).ofx_ok()?;

    let width = (r - l).max(0) as usize;
    let height = (t - b).max(0) as usize;
    if width == 0 || height == 0 {
        return Err(OfxStat::kOfxStatFailed);
    }
    if width > 16384 || height > 16384 {
        return Err(OfxStat::kOfxStatErrFormat);
    }
    let s_stride = srb.max(0) as usize;
    let d_stride = drb.max(0) as usize;

    let depth = detect_pixel_depth(su, si).unwrap_or(4);
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
        ascii_art.apply_effect(src_buf, dst_buf, width, height);
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
    dst: &mut ZzzAsciiArtFullSettings,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    // --- Read generic params (skip native params) ---
    for desc in d.settings_list.all_descriptors() {
        if is_native_grouped_name(desc.id.name) {
            continue;
        }
        read_generic_param(su, param_set, time, desc, dst, &d.strings)?;
    }

    // --- Font name: read Choice param directly, fall back to hidden String ---
    {
        // Read the Choice param index first
        let mut cp: OfxParamHandle = ptr::null_mut();
        let has_choice = pgh(
            param_set,
            FONT_CHOICE_PARAM.as_ptr(),
            &mut cp,
            ptr::null_mut(),
        )
        .ofx_ok()
        .is_ok();
        if has_choice {
            let mut choice_idx: c_int = 0;
            if pgv(cp, time, &mut choice_idx).ofx_ok().is_ok() && choice_idx > 0 {
                dst.font_name = d
                    .available_fonts
                    .get((choice_idx - 1) as usize)
                    .cloned()
                    .unwrap_or_default();
            }
        }

        // Fall back to hidden String param (persisted value, or set by InstanceChanged)
        if dst.font_name.is_empty() {
            let mut sp: OfxParamHandle = ptr::null_mut();
            if pgh(
                param_set,
                FONT_NAME_PARAM.as_ptr(),
                &mut sp,
                ptr::null_mut(),
            )
            .ofx_ok()
            .is_ok()
            {
                let mut name_ptr: *mut c_char = ptr::null_mut();
                if pgv(sp, time, &mut name_ptr).ofx_ok().is_ok() && !name_ptr.is_null() {
                    let name = CStr::from_ptr(name_ptr).to_string_lossy();
                    if !name.is_empty() {
                        dst.font_name = name.into_owned();
                    }
                }
            }
        }
    }

    // --- Hidden String: custom_chars ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(
            param_set,
            CUSTOM_CHARS_PARAM.as_ptr(),
            &mut p,
            ptr::null_mut(),
        )
        .ofx_ok()?;
        let mut name_ptr: *mut c_char = ptr::null_mut();
        if pgv(p, time, &mut name_ptr).ofx_ok().is_ok() && !name_ptr.is_null() {
            let s = CStr::from_ptr(name_ptr).to_string_lossy();
            if !s.is_empty() {
                dst.custom_chars = s.into_owned();
            }
        }
    }

    Ok(())
}
