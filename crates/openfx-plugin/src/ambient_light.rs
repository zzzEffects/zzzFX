use std::{
    cell::RefCell,
    ffi::{CStr, c_char, c_int, c_void},
    ptr,
    sync::OnceLock,
};

use zzzfx::settings::TrKey;
use zzzfx::{
    AmbientLight, AmbientLightFullSettings,
    settings::{SettingsList},
};

use crate::bindings::*;
use crate::i18n;
use crate::shared::{
    HostInfo, SuiteCache, StringCache, MenuItemCache,
    build_string_cache, define_single_param, read_generic_param,
    copy_source_to_u8, copy_u8_to_output, detect_pixel_depth,
    action_load_common,
};

// ---------------------------------------------------------------------------
// Per-effect globals
// ---------------------------------------------------------------------------

static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();

struct EffectData {
    suites: SuiteCache,
    settings_list: SettingsList<AmbientLightFullSettings>,
    strings: StringCache<AmbientLightFullSettings>,
    menu_item_strings: MenuItemCache<AmbientLightFullSettings>,
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
        pluginIdentifier: c"io.github.zzzEffects:AmbientLight".as_ptr(),
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
    let settings_list = SettingsList::<AmbientLightFullSettings>::new();
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

    ps(ep, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::EffectAmbientLightName).as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginPropGrouping.as_ptr(), 0, c"zzzFX".as_ptr()).ofx_ok()?;
    ps(ep, kOfxPropPluginDescription.as_ptr(), 0, i18n::tr_cstr(TrKey::EffectAmbientLightDesc).as_ptr()).ofx_ok()?;
    // Compositor: General context only
    ps(ep, kOfxImageEffectPropSupportedContexts.as_ptr(), 0, kOfxImageEffectContextGeneral.as_ptr()).ofx_ok()?;
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
    let defaults = AmbientLightFullSettings::default();

    // --- Output clip ---
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    cd(desc, c"Output".as_ptr(), &mut props).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 0, kOfxImageComponentRGBA.as_ptr()).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 1, kOfxImageComponentRGB.as_ptr()).ofx_ok()?;

    // --- SourceA (foreground) ---
    cd(desc, c"SourceA".as_ptr(), &mut props).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 0, kOfxImageComponentRGBA.as_ptr()).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 1, kOfxImageComponentRGB.as_ptr()).ofx_ok()?;

    // --- SourceB (background) ---
    cd(desc, c"SourceB".as_ptr(), &mut props).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 0, kOfxImageComponentRGBA.as_ptr()).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 1, kOfxImageComponentRGB.as_ptr()).ofx_ok()?;

    // --- Parameter set ---
    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gp(desc, &mut param_set).ofx_ok()?;

    // Define all params (no native grouped params — all are generic scalars)
    for desc in d.settings_list.setting_descriptors.iter() {
        define_single_param(su, param_set, desc, &defaults, c"", &d.strings, &d.menu_item_strings)?;
    }

    Ok(())
}

unsafe fn action_get_regions_of_interest(
    effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
    outArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    // Copy RoI from foreground (SourceB) to both clips
    let d = data()?;
    let su = &d.suites;
    let pg = su.property_suite.propGetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let psn = su.property_suite.propSetDoubleN.ok_or(OfxStat::kOfxStatFailed)?;
    let cgh = su.image_effect_suite.clipGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let crod = su.image_effect_suite.clipGetRegionOfDefinition.ok_or(OfxStat::kOfxStatFailed)?;

    let mut sc: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"SourceB".as_ptr(), &mut sc, ptr::null_mut()).ofx_ok()?;
    let mut rod = OfxRectD { x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 };
    let mut t: OfxTime = 0.0;
    pg(inArgs, kOfxPropTime.as_ptr(), 0, &mut t).ofx_ok()?;
    crod(sc, t, &mut rod).ofx_ok()?;

    psn(outArgs, c"OfxImageClipPropRoI_SourceA".as_ptr(), 4, ptr::addr_of_mut!(rod) as *mut _).ofx_ok()?;
    psn(outArgs, c"OfxImageClipPropRoI_SourceB".as_ptr(), 4, ptr::addr_of_mut!(rod) as *mut _).ofx_ok()?;
    Ok(())
}

unsafe fn action_get_clip_preferences(outArgs: OfxPropertySetHandle) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    // Request unpremultiplied alpha from both input clips to ensure
    // the straight-alpha OVER composite formula works correctly.
    let ps = su.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    ps(outArgs, kOfxImageEffectPropPreMultiplication.as_ptr(), 0, kOfxImageUnPreMultiplied.as_ptr()).ofx_ok()?;
    ps(outArgs, c"OfxImageClipPropPreMultiplication_SourceA".as_ptr(), 0, kOfxImageUnPreMultiplied.as_ptr()).ofx_ok()?;
    ps(outArgs, c"OfxImageClipPropPreMultiplication_SourceB".as_ptr(), 0, kOfxImageUnPreMultiplied.as_ptr()).ofx_ok()?;
    Ok(())
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
    _effect: OfxImageEffectHandle,
    _inArgs: OfxPropertySetHandle,
    _outArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    // Always render — the identity path is handled internally by apply_effect()
    // which does a proper fg-over-bg composite when is_identity() is true.
    Err(OfxStat::kOfxStatReplyDefault)
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

// Thread-local render buffers
struct RenderBufs {
    fg_buf: Vec<u8>,
    bg_buf: Vec<u8>,
    dst_buf: Vec<u8>,
}

impl Default for RenderBufs {
    fn default() -> Self {
        Self { fg_buf: Vec::new(), bg_buf: Vec::new(), dst_buf: Vec::new() }
    }
}

thread_local! {
    static RENDER_BUFS: RefCell<RenderBufs> = RefCell::new(RenderBufs::default());
}

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

    let mut settings = AmbientLightFullSettings::default();
    apply_params(param_set, time, &mut settings)?;

    let ambient: AmbientLight = (&settings).into();

    // Get clip handles: SourceA = background (lower track), SourceB = foreground (upper track)
    let mut bg_clip: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"SourceA".as_ptr(), &mut bg_clip, ptr::null_mut()).ofx_ok()?;
    let mut fg_clip: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"SourceB".as_ptr(), &mut fg_clip, ptr::null_mut()).ofx_ok()?;
    let mut out_clip: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"Output".as_ptr(), &mut out_clip, ptr::null_mut()).ofx_ok()?;

    // Fetch foreground image
    let mut fg_img: OfxPropertySetHandle = ptr::null_mut();
    cgi(fg_clip, time, ptr::null(), &mut fg_img).ofx_ok()?;

    let mut l: c_int = 0; let mut b: c_int = 0;
    let mut r: c_int = 0; let mut t: c_int = 0;
    pgi(fg_img, kOfxImagePropBounds.as_ptr(), 0, &mut l).ofx_ok()?;
    pgi(fg_img, kOfxImagePropBounds.as_ptr(), 1, &mut b).ofx_ok()?;
    pgi(fg_img, kOfxImagePropBounds.as_ptr(), 2, &mut r).ofx_ok()?;
    pgi(fg_img, kOfxImagePropBounds.as_ptr(), 3, &mut t).ofx_ok()?;

    let width = (r - l).max(0) as usize;
    let height = (t - b).max(0) as usize;
    if width == 0 || height == 0 { return Err(OfxStat::kOfxStatFailed); }
    if width > 16384 || height > 16384 { return Err(OfxStat::kOfxStatErrFormat); }
    let row_bytes_u8 = width * 4;
    let total_u8 = row_bytes_u8 * height;

    let depth = detect_pixel_depth(su, fg_img).ok_or(OfxStat::kOfxStatErrFormat)?;

    let mut fg_rb: c_int = 0;
    pgi(fg_img, kOfxImagePropRowBytes.as_ptr(), 0, &mut fg_rb).ofx_ok()?;

    // Fetch background image
    let mut bg_img: OfxPropertySetHandle = ptr::null_mut();
    cgi(bg_clip, time, ptr::null(), &mut bg_img).ofx_ok()?;
    let mut bg_rb: c_int = 0;
    pgi(bg_img, kOfxImagePropRowBytes.as_ptr(), 0, &mut bg_rb).ofx_ok()?;

    // Copy source images to contiguous u8 buffers
    let mut rb = RENDER_BUFS.with(|cell| cell.take());
    if rb.fg_buf.len() != total_u8 {
        rb.fg_buf.resize(total_u8, 0);
        rb.bg_buf.resize(total_u8, 0);
        rb.dst_buf.resize(total_u8, 0);
    }

    // Copy foreground
    {
        let mut sp: *mut c_void = ptr::null_mut();
        pgp(fg_img, kOfxImagePropData.as_ptr(), 0, &mut sp).ofx_ok()?;
        if !sp.is_null() {
            copy_source_to_u8(sp, fg_rb.max(0) as usize, &mut rb.fg_buf, width, height, row_bytes_u8, depth);
        } else {
            rb.fg_buf.fill(0);
        }
    }

    // Copy background
    {
        let mut sp: *mut c_void = ptr::null_mut();
        pgp(bg_img, kOfxImagePropData.as_ptr(), 0, &mut sp).ofx_ok()?;
        if !sp.is_null() {
            // Background may have different depth — re-detect
            let bg_depth = detect_pixel_depth(su, bg_img).unwrap_or(depth);
            copy_source_to_u8(sp, bg_rb.max(0) as usize, &mut rb.bg_buf, width, height, row_bytes_u8, bg_depth);
        } else {
            rb.bg_buf.fill(0);
        }
    }

    // Release source images early
    cri(fg_img).ofx_ok()?;
    cri(bg_img).ofx_ok()?;

    // Apply effect
    ambient.apply_effect(&rb.fg_buf, &rb.bg_buf, &mut rb.dst_buf, width, height);

    // Write output
    let mut out_img: OfxPropertySetHandle = ptr::null_mut();
    cgi(out_clip, time, ptr::null(), &mut out_img).ofx_ok()?;
    let mut dp: *mut c_void = ptr::null_mut();
    pgp(out_img, kOfxImagePropData.as_ptr(), 0, &mut dp).ofx_ok()?;
    let mut drb: c_int = 0;
    pgi(out_img, kOfxImagePropRowBytes.as_ptr(), 0, &mut drb).ofx_ok()?;
    copy_u8_to_output(&rb.dst_buf, dp, drb.max(0) as usize, width, height, row_bytes_u8, depth);
    cri(out_img).ofx_ok()?;

    RENDER_BUFS.with(|cell| cell.replace(rb));
    Ok(())
}

// ---------------------------------------------------------------------------
// Parameter reading
// ---------------------------------------------------------------------------

unsafe fn apply_params(
    param_set: OfxParamSetHandle,
    time: f64,
    dst: &mut AmbientLightFullSettings,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;

    // All params are generic scalars — no native grouped params
    for desc in d.settings_list.setting_descriptors.iter() {
        read_generic_param(su, param_set, time, desc, dst, &d.strings)?;
    }

    Ok(())
}
