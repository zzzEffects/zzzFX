use std::{
    ffi::{CStr, c_char, c_int, c_void},
    ptr,
    sync::OnceLock,
};

use zzzfx::settings::TrKey;
use zzzfx::{
    PixelArt, PixelArtFullSettings,
    settings::SettingsList,
};

use crate::bindings::*;
use crate::i18n;
use crate::shared::{
    HostInfo, SuiteCache, StringCache, MenuItemCache, build_string_cache,
    define_single_param, read_generic_param, copy_source_to_u8, copy_u8_to_output,
    detect_pixel_depth, action_load_common, action_get_clip_preferences_common,
    action_get_regions_of_interest_common,
};

// ---------------------------------------------------------------------------
// RAII guard for host image handles — ensures clipReleaseImage is called
// even if a panic unwinds past the render function
// ---------------------------------------------------------------------------

struct ClipImageGuard {
    img: OfxPropertySetHandle,
    release_fn: unsafe extern "C" fn(OfxPropertySetHandle) -> OfxStatus,
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

// ---------------------------------------------------------------------------
// Native OFX parameter names (for pixel_size_v enable/disable via square)
// ---------------------------------------------------------------------------

const SQUARE_PARAM: &CStr = c"square";
const PIXEL_SIZE_V_PARAM: &CStr = c"pixel_size_v";
const GRID_COLOR_PARAM: &CStr = c"grid_color";
const GRID_POSITION_PARAM: &CStr = c"grid_position";

fn is_native_grouped_name(name: &str) -> bool {
    matches!(
        name,
        "grid_color_r" | "grid_color_g" | "grid_color_b" | "grid_color_a"
            | "grid_position_x" | "grid_position_y"
    )
}

// ---------------------------------------------------------------------------
// Per-effect globals
// ---------------------------------------------------------------------------

static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();

struct EffectData {
    suites: SuiteCache,
    settings_list: SettingsList<PixelArtFullSettings>,
    strings: StringCache<PixelArtFullSettings>,
    menu_item_strings: MenuItemCache<PixelArtFullSettings>,
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
        pluginIdentifier: c"io.github.zzzEffects:PixelArt".as_ptr(),
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
    let settings_list = SettingsList::<PixelArtFullSettings>::new();
    i18n::set_lang(i18n::detect_system_lang());
    let (strings, menu_item_strings) = build_string_cache(&settings_list);

    EFFECT_DATA.get_or_init(|| EffectData {
        suites,
        settings_list,
        strings,
        menu_item_strings,
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
        i18n::tr_cstr(TrKey::EffectPixelArtName).as_ptr(),
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
        i18n::tr_cstr(TrKey::EffectPixelArtDesc).as_ptr(),
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
    let defaults = PixelArtFullSettings::default();

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

    let pd = su.property_suite.propSetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let pdef = su.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;

    // --- Block A: generic params before grid_color ---
    for desc in d.settings_list.setting_descriptors.iter() {
        if desc.id.name == "grid_color_r" {
            break;
        }
        if is_native_grouped_name(desc.id.name) {
            continue;
        }
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

    // --- Native RGBA: Grid Color ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(
            param_set,
            kOfxParamTypeRGBA.as_ptr(),
            GRID_COLOR_PARAM.as_ptr(),
            &mut pp,
        )
        .ofx_ok()?;
        ps(
            pp,
            kOfxPropLabel.as_ptr(),
            0,
            i18n::tr_cstr(TrKey::NativeGridColor).as_ptr(),
        )
        .ofx_ok()?;
        ps(
            pp,
            kOfxParamPropHint.as_ptr(),
            0,
            i18n::tr_cstr(TrKey::NativeGridColorHint).as_ptr(),
        )
        .ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 0.0).ofx_ok()?; // R
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 0.0).ofx_ok()?; // G
        pd(pp, kOfxParamPropDefault.as_ptr(), 2, 0.0).ofx_ok()?; // B
        pd(pp, kOfxParamPropDefault.as_ptr(), 3, 0.5).ofx_ok()?; // A
    }

    // --- Native Double2D: Grid Position ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(
            param_set,
            kOfxParamTypeDouble2D.as_ptr(),
            GRID_POSITION_PARAM.as_ptr(),
            &mut pp,
        )
        .ofx_ok()?;
        ps(
            pp,
            kOfxPropLabel.as_ptr(),
            0,
            i18n::tr_cstr(TrKey::NativeGridPosition).as_ptr(),
        )
        .ofx_ok()?;
        ps(
            pp,
            kOfxParamPropHint.as_ptr(),
            0,
            i18n::tr_cstr(TrKey::NativeGridPositionHint).as_ptr(),
        )
        .ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 0.5).ofx_ok()?; // X
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 0.5).ofx_ok()?; // Y
    }

    // --- Block B: generic params after grid_color (skip the 4 color fields) ---
    let mut after_grid_color = false;
    for desc in d.settings_list.setting_descriptors.iter() {
        if desc.id.name == "grid_color_r" {
            after_grid_color = true;
            continue;
        }
        if !after_grid_color
            || is_native_grouped_name(desc.id.name)
        {
            continue;
        }
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
    let psi = su.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let gps = su.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
    let psv = su.parameter_suite.paramSetValue.ok_or(OfxStat::kOfxStatFailed)?;

    let mut target_name: *mut c_char = ptr::null_mut();
    if pgs(inArgs, kOfxPropName.as_ptr(), 0, &mut target_name).ofx_ok().is_err()
        || target_name.is_null()
    {
        return Ok(());
    }

    if SQUARE_PARAM == CStr::from_ptr(target_name) {
        let mut param_set: OfxParamSetHandle = ptr::null_mut();
        gps(effect, &mut param_set).ofx_ok()?;

        // Read the square value
        let mut sp: OfxParamHandle = ptr::null_mut();
        pgh(
            param_set,
            SQUARE_PARAM.as_ptr(),
            &mut sp,
            ptr::null_mut(),
        )
        .ofx_ok()?;
        let mut square_val: c_int = 0;
        pgv(sp, 0.0, &mut square_val).ofx_ok()?;
        let square_on = square_val != 0;

        // Get pixel_size_v param handle + its property set
        let mut vp_handle: OfxParamHandle = ptr::null_mut();
        let mut vp_props: OfxPropertySetHandle = ptr::null_mut();
        pgh(
            param_set,
            PIXEL_SIZE_V_PARAM.as_ptr(),
            &mut vp_handle,
            &mut vp_props,
        )
        .ofx_ok()?;

        // Enable/disable pixel_size_v
        if !vp_props.is_null() {
            psi(
                vp_props,
                kOfxParamPropEnabled.as_ptr(),
                0,
                if square_on { 0 } else { 1 },
            )
            .ofx_ok()?;
        }

        // When square is turned ON, sync pixel_size_v from pixel_size_h
        if square_on {
            // Read pixel_size_h value
            let mut hp: OfxParamHandle = ptr::null_mut();
            pgh(
                param_set,
                c"pixel_size_h".as_ptr(),
                &mut hp,
                ptr::null_mut(),
            )
            .ofx_ok()?;
            let mut h_val: f64 = 0.0;
            pgv(hp, 0.0, &mut h_val).ofx_ok()?;
            // Set pixel_size_v to the same value
            psv(vp_handle, &h_val as *const f64 as *const c_void).ofx_ok()?;
        }
    }

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
    let gps = su.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;
    let pgd = su.property_suite.propGetDouble.ok_or(OfxStat::kOfxStatFailed)?;

    let mut time: OfxTime = 0.0;
    pgd(inArgs, kOfxPropTime.as_ptr(), 0, &mut time).ofx_ok()?;

    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gps(effect, &mut param_set).ofx_ok()?;

    let mut settings = PixelArtFullSettings::default();
    apply_params(param_set, time, &mut settings)?;
    let pixel_effect: PixelArt = (&settings).into();

    if pixel_effect.is_identity() {
        let cgh = su.image_effect_suite.clipGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
        let ps = su.property_suite.propSetPointer.ok_or(OfxStat::kOfxStatFailed)?;
        let mut sc: OfxImageClipHandle = ptr::null_mut();
        cgh(
            effect,
            c"Source".as_ptr(),
            &mut sc,
            ptr::null_mut(),
        )
        .ofx_ok()?;
        ps(
            outArgs,
            kOfxPropEffectInstance.as_ptr(),
            0,
            sc as *mut c_void,
        )
        .ofx_ok()?;
        ps(
            outArgs,
            kOfxImageEffectOutputClipName.as_ptr(),
            0,
            c"Source".as_ptr() as *mut c_void,
        )
        .ofx_ok()?;
        return Ok(());
    }

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

    let mut settings = PixelArtFullSettings::default();
    apply_params(param_set, time, &mut settings)?;
    let pixel_art: PixelArt = (&settings).into();

    let mut sc: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"Source".as_ptr(), &mut sc, ptr::null_mut()).ofx_ok()?;
    let mut dc: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"Output".as_ptr(), &mut dc, ptr::null_mut()).ofx_ok()?;

    let mut si: OfxPropertySetHandle = ptr::null_mut();
    cgi(sc, time, ptr::null(), &mut si).ofx_ok()?;
    let _si_guard = ClipImageGuard { img: si, release_fn: cri };
    let mut di: OfxPropertySetHandle = ptr::null_mut();
    cgi(dc, time, ptr::null(), &mut di).ofx_ok()?;
    let _di_guard = ClipImageGuard { img: di, release_fn: cri };

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

    let depth = detect_pixel_depth(su, si).ok_or(OfxStat::kOfxStatErrFormat)?;
    let row_bytes_u8 = width * 4;
    let total_u8 = row_bytes_u8 * height;

    thread_local! {
        static RENDER_BUFS: std::cell::RefCell<(Vec<u8>, Vec<u8>)> =
            std::cell::RefCell::new((Vec::new(), Vec::new()));
    }
    RENDER_BUFS.with(|cell| {
        if let Ok(mut bufs) = cell.try_borrow_mut() {
            let (src_buf, dst_buf) = &mut *bufs;
            src_buf.resize(total_u8, 0);
            dst_buf.resize(total_u8, 0);
            copy_source_to_u8(sp, s_stride, src_buf, width, height, row_bytes_u8, depth);
            pixel_art.apply_effect(src_buf, dst_buf, width, height);
            copy_u8_to_output(dst_buf, dp, d_stride, width, height, row_bytes_u8, depth);
        } else {
            // Re-entrant render — allocate fresh buffers
            let mut src_buf = vec![0u8; total_u8];
            let mut dst_buf = vec![0u8; total_u8];
            copy_source_to_u8(sp, s_stride, &mut src_buf, width, height, row_bytes_u8, depth);
            pixel_art.apply_effect(&src_buf, &mut dst_buf, width, height);
            copy_u8_to_output(&dst_buf, dp, d_stride, width, height, row_bytes_u8, depth);
        }
    });

    // Image handles released by ClipImageGuard drop
    Ok(())
}

// ---------------------------------------------------------------------------
// Parameter reading
// ---------------------------------------------------------------------------

unsafe fn apply_params(
    param_set: OfxParamSetHandle,
    time: f64,
    dst: &mut PixelArtFullSettings,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    // Read generic params (skip native RGBA fields)
    for desc in d.settings_list.all_descriptors() {
        if is_native_grouped_name(desc.id.name) {
            continue;
        }
        read_generic_param(su, param_set, time, desc, dst, &d.strings)?;
    }

    // Native RGBA: Grid Color
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(
            param_set,
            GRID_COLOR_PARAM.as_ptr(),
            &mut p,
            ptr::null_mut(),
        )
        .ofx_ok()?;
        let mut r: f64 = 0.0;
        let mut g: f64 = 0.0;
        let mut b: f64 = 0.0;
        let mut a: f64 = 0.0;
        pgv(p, time, &mut r, &mut g, &mut b, &mut a).ofx_ok()?;
        dst.grid_color_r = r as f32;
        dst.grid_color_g = g as f32;
        dst.grid_color_b = b as f32;
        dst.grid_color_a = a as f32;
    }

    // Native Double2D: Grid Position
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(
            param_set,
            GRID_POSITION_PARAM.as_ptr(),
            &mut p,
            ptr::null_mut(),
        )
        .ofx_ok()?;
        let mut x: f64 = 0.0;
        let mut y: f64 = 0.0;
        pgv(p, time, &mut x, &mut y).ofx_ok()?;
        dst.grid_position_x = x as f32;
        dst.grid_position_y = y as f32;
    }

    Ok(())
}
