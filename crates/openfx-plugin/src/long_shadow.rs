use std::{
    ffi::{CStr, c_char, c_int, c_void},
    ptr,
    sync::OnceLock,
};

use zzzfx::settings::TrKey;
use zzzfx::{
    LongShadow, LongShadowFullSettings,
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
// RAII guard for host image handles
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
// Native OFX parameter names
// ---------------------------------------------------------------------------

const SHADOW_COLOR_PARAM: &CStr = c"shadow_color";
const SHADOW_OFFSET_PARAM: &CStr = c"shadow_offset";

fn is_native_grouped_name(name: &str) -> bool {
    matches!(
        name,
        "shadow_color_r" | "shadow_color_g" | "shadow_color_b" | "shadow_color_a"
            | "offset_x" | "offset_y"
    )
}

// ---------------------------------------------------------------------------
// Per-effect globals
// ---------------------------------------------------------------------------

static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();

struct EffectData {
    suites: SuiteCache,
    settings_list: SettingsList<LongShadowFullSettings>,
    strings: StringCache<LongShadowFullSettings>,
    menu_item_strings: MenuItemCache<LongShadowFullSettings>,
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
        pluginIdentifier: c"io.github.zzzEffect:LongShadow".as_ptr(),
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
    let settings_list = SettingsList::<LongShadowFullSettings>::new();
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
        i18n::tr_cstr(TrKey::EffectLongShadowName).as_ptr(),
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
        i18n::tr_cstr(TrKey::EffectLongShadowDesc).as_ptr(),
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
    let pd = su.property_suite.propSetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let pdef = su.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let defaults = LongShadowFullSettings::default();

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

    // Iterate generic descriptors, inserting native params at specific points
    let mut passed_shadow_color = false;
    let mut passed_shadow_offset = false;

    for desc in d.settings_list.setting_descriptors.iter() {
        if is_native_grouped_name(desc.id.name) {
            continue;
        }

        // Insert Shadow Color native param after "length"
        if !passed_shadow_color && desc.id.name == "softness" {
            passed_shadow_color = true;
            let mut pp: OfxPropertySetHandle = ptr::null_mut();
            pdef(
                param_set,
                kOfxParamTypeRGBA.as_ptr(),
                SHADOW_COLOR_PARAM.as_ptr(),
                &mut pp,
            )
            .ofx_ok()?;
            ps(
                pp,
                kOfxPropLabel.as_ptr(),
                0,
                i18n::tr_cstr(TrKey::NativeShadowColor).as_ptr(),
            )
            .ofx_ok()?;
            ps(
                pp,
                kOfxParamPropHint.as_ptr(),
                0,
                i18n::tr_cstr(TrKey::NativeShadowColorHint).as_ptr(),
            )
            .ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 0, 0.0).ofx_ok()?; // R
            pd(pp, kOfxParamPropDefault.as_ptr(), 1, 0.0).ofx_ok()?; // G
            pd(pp, kOfxParamPropDefault.as_ptr(), 2, 0.0).ofx_ok()?; // B
            pd(pp, kOfxParamPropDefault.as_ptr(), 3, 0.8).ofx_ok()?; // A
        }

        // Insert Shadow Offset native param after "fade"
        if !passed_shadow_offset && desc.id.name == "opacity" {
            passed_shadow_offset = true;
            let mut pp: OfxPropertySetHandle = ptr::null_mut();
            pdef(
                param_set,
                kOfxParamTypeDouble2D.as_ptr(),
                SHADOW_OFFSET_PARAM.as_ptr(),
                &mut pp,
            )
            .ofx_ok()?;
            ps(
                pp,
                kOfxPropLabel.as_ptr(),
                0,
                i18n::tr_cstr(TrKey::NativeShadowOffset).as_ptr(),
            )
            .ofx_ok()?;
            ps(
                pp,
                kOfxParamPropHint.as_ptr(),
                0,
                i18n::tr_cstr(TrKey::NativeShadowOffsetHint).as_ptr(),
            )
            .ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 0, 0.5).ofx_ok()?; // X
            pd(pp, kOfxParamPropDefault.as_ptr(), 1, 0.5).ofx_ok()?; // Y
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
    action_get_clip_preferences_common(&data()?.suites, outArgs, 0, kOfxImageComponentRGBA)
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

    let mut settings = LongShadowFullSettings::default();
    apply_params(param_set, time, &mut settings)?;
    let shadow: LongShadow = (&settings).into();

    if shadow.is_identity() {
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

    let mut settings = LongShadowFullSettings::default();
    apply_params(param_set, time, &mut settings)?;
    let shadow: LongShadow = (&settings).into();

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

    // ── Use DESTINATION bounds for working region ──────────────
    // The shadow may extend beyond the source RoD into the output
    // (General context), so buffers must be sized to the destination.
    let mut dst_l: c_int = 0; let mut dst_b: c_int = 0;
    let mut dst_r: c_int = 0; let mut dst_t: c_int = 0;
    pgi(di, kOfxImagePropBounds.as_ptr(), 0, &mut dst_l).ofx_ok()?;
    pgi(di, kOfxImagePropBounds.as_ptr(), 1, &mut dst_b).ofx_ok()?;
    pgi(di, kOfxImagePropBounds.as_ptr(), 2, &mut dst_r).ofx_ok()?;
    pgi(di, kOfxImagePropBounds.as_ptr(), 3, &mut dst_t).ofx_ok()?;

    let width = (dst_r - dst_l).max(0) as usize;
    let height = (dst_t - dst_b).max(0) as usize;
    if width == 0 || height == 0 {
        return Err(OfxStat::kOfxStatFailed);
    }
    if width > 16384 || height > 16384 {
        return Err(OfxStat::kOfxStatErrFormat);
    }

    // Source bounds for offset within the destination region
    let mut src_l: c_int = 0; let mut src_b: c_int = 0;
    let mut src_r: c_int = 0; let mut src_t: c_int = 0;
    pgi(si, kOfxImagePropBounds.as_ptr(), 0, &mut src_l).ofx_ok()?;
    pgi(si, kOfxImagePropBounds.as_ptr(), 1, &mut src_b).ofx_ok()?;
    pgi(si, kOfxImagePropBounds.as_ptr(), 2, &mut src_r).ofx_ok()?;
    pgi(si, kOfxImagePropBounds.as_ptr(), 3, &mut src_t).ofx_ok()?;

    let src_w = (src_r - src_l).max(0) as usize;
    let src_h = (src_t - src_b).max(0) as usize;
    let off_x = ((src_l - dst_l).max(0)) as usize;
    let off_y = ((src_b - dst_b).max(0)) as usize;

    let s_stride = srb.max(0) as usize;
    let d_stride = drb.max(0) as usize;

    let depth = detect_pixel_depth(su, si).ok_or(OfxStat::kOfxStatErrFormat)?;
    let row_bytes_u8 = width * 4;
    let total_u8 = row_bytes_u8 * height;

    // Check if source and destination regions are the same size —
    // if so, use the fast direct-copy path.
    let same_region =
        src_l == dst_l && src_b == dst_b && src_r == dst_r && src_t == dst_t;

    thread_local! {
        static RENDER_BUFS: std::cell::RefCell<(Vec<u8>, Vec<u8>, Vec<u8>)> =
            std::cell::RefCell::new((Vec::new(), Vec::new(), Vec::new()));
    }
    RENDER_BUFS.with(|cell| {
        let do_render = |src_buf: &mut Vec<u8>, dst_buf: &mut Vec<u8>, tmp: &mut Vec<u8>| {
            src_buf.resize(total_u8, 0);
            dst_buf.resize(total_u8, 0);

            if same_region {
                copy_source_to_u8(sp, s_stride, src_buf, width, height, row_bytes_u8, depth);
            } else {
                // Source is smaller than destination — copy source into
                // a sub-rectangle; the rest stays transparent (zero).
                src_buf.fill(0);
                tmp.resize(src_w * src_h * 4, 0);
                copy_source_to_u8(sp, s_stride, tmp, src_w, src_h, src_w * 4, depth);
                let copy_w = (src_w * 4).min(row_bytes_u8.saturating_sub(off_x * 4));
                for row in 0..src_h.min(height.saturating_sub(off_y)) {
                    let tmp_row = row * src_w * 4;
                    let dst_row = (off_y + row) * row_bytes_u8 + off_x * 4;
                    ptr::copy_nonoverlapping(
                        tmp.as_ptr().add(tmp_row),
                        src_buf.as_mut_ptr().add(dst_row),
                        copy_w,
                    );
                }
            }

            shadow.apply_effect(src_buf, dst_buf, width, height);
            copy_u8_to_output(dst_buf, dp, d_stride, width, height, row_bytes_u8, depth);
        };

        if let Ok(mut bufs) = cell.try_borrow_mut() {
            let (src_buf, dst_buf, tmp) = &mut *bufs;
            do_render(src_buf, dst_buf, tmp);
        } else {
            let mut src_buf = vec![0u8; total_u8];
            let mut dst_buf = vec![0u8; total_u8];
            let mut tmp = vec![0u8; 0];
            do_render(&mut src_buf, &mut dst_buf, &mut tmp);
        }
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Parameter reading
// ---------------------------------------------------------------------------

unsafe fn apply_params(
    param_set: OfxParamSetHandle,
    time: f64,
    dst: &mut LongShadowFullSettings,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    // Read generic params (skip native-mapped fields)
    for desc in d.settings_list.all_descriptors() {
        if is_native_grouped_name(desc.id.name) {
            continue;
        }
        read_generic_param(su, param_set, time, desc, dst, &d.strings)?;
    }

    // Native RGBA: Shadow Color
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(
            param_set,
            SHADOW_COLOR_PARAM.as_ptr(),
            &mut p,
            ptr::null_mut(),
        )
        .ofx_ok()?;
        let mut r: f64 = 0.0;
        let mut g: f64 = 0.0;
        let mut b: f64 = 0.0;
        let mut a: f64 = 0.0;
        pgv(p, time, &mut r, &mut g, &mut b, &mut a).ofx_ok()?;
        dst.shadow_color_r = r as f32;
        dst.shadow_color_g = g as f32;
        dst.shadow_color_b = b as f32;
        dst.shadow_color_a = a as f32;
    }

    // Native Double2D: Shadow Offset
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(
            param_set,
            SHADOW_OFFSET_PARAM.as_ptr(),
            &mut p,
            ptr::null_mut(),
        )
        .ofx_ok()?;
        let mut x: f64 = 0.0;
        let mut y: f64 = 0.0;
        pgv(p, time, &mut x, &mut y).ofx_ok()?;
        dst.offset_x = x as f32;
        dst.offset_y = y as f32;
    }

    Ok(())
}
