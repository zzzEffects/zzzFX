#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

mod bindings;

use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_int, c_void},
    ptr,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use example_effect::{
    ZzzStroke, ZzzStrokeFullSettings,
    settings::{
        EnumValue, SettingDescriptor, SettingID, SettingKind, Settings, SettingsList,
    },
};

use bindings::*;

// SAFETY
unsafe impl Send for OfxPlugin {}
unsafe impl Sync for OfxPlugin {}

// ---------------------------------------------------------------------------
// Globals
// ---------------------------------------------------------------------------

static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();
static SHARED_DATA: OnceLock<SharedData> = OnceLock::new();

// Native OFX parameter names for grouped params
const STROKE_COLOR_PARAM: &CStr = c"strokeColor";
const GRADIENT_START_POS_PARAM: &CStr = c"gradientStartPos";
const GRADIENT_START_COLOR_PARAM: &CStr = c"gradientStartColor";
const GRADIENT_END_POS_PARAM: &CStr = c"gradientEndPos";
const GRADIENT_END_COLOR_PARAM: &CStr = c"gradientEndColor";
const GRADIENT_GROUP_PARAM: &CStr = c"gradientGroup";

// Descriptor IDs handled by native params (not created as generic params)
fn is_native_grouped(id: u32) -> bool {
    matches!(
        id,
        203 | 204 | 205 | 206 |        // stroke_color → RGBA
        213 | 214 |                     // gradient_start → Double2D
        215 | 216 | 217 | 218 |         // gradient_start_color → RGBA
        219 | 220 |                     // gradient_end → Double2D
        221 | 222 | 223 | 224           // gradient_end_color → RGBA
    )
}

// ---------------------------------------------------------------------------
// HostInfo
// ---------------------------------------------------------------------------

struct HostInfo {
    host: &'static OfxPropertySetStruct,
    fetch_suite: unsafe extern "C" fn(
        host: OfxPropertySetHandle,
        suiteName: *const c_char,
        suiteVersion: c_int,
    ) -> *const c_void,
}

// ---------------------------------------------------------------------------
// SharedData
// ---------------------------------------------------------------------------

struct SharedData {
    #[allow(dead_code)]
    host_info: HostInfo,
    property_suite: &'static OfxPropertySuiteV1,
    image_effect_suite: &'static OfxImageEffectSuiteV1,
    #[allow(dead_code)]
    memory_suite: &'static OfxMemorySuiteV1,
    parameter_suite: &'static OfxParameterSuiteV1,
    settings_list: SettingsList<ZzzStrokeFullSettings>,
    supports_multiple_clip_depths: AtomicBool,
    strings: HashMap<SettingID<ZzzStrokeFullSettings>, (CString, CString, Option<CString>, Option<CString>)>,
    menu_item_strings: HashMap<(SettingID<ZzzStrokeFullSettings>, u32), (CString, Option<CString>)>,
}

type OfxResult<T> = Result<T, OfxStatus>;

impl SharedData {
    pub unsafe fn new(host_info: HostInfo) -> OfxResult<Self> {
        let property_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _,
            kOfxPropertySuite.as_ptr(), 1,
        ) as *const OfxPropertySuiteV1;
        let image_effect_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _,
            kOfxImageEffectSuite.as_ptr(), 1,
        ) as *const OfxImageEffectSuiteV1;
        let memory_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _, kOfxMemorySuite.as_ptr(), 1,
        ) as *const OfxMemorySuiteV1;
        let parameter_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _,
            kOfxParameterSuite.as_ptr(), 1,
        ) as *const OfxParameterSuiteV1;

        let settings_list = SettingsList::<ZzzStrokeFullSettings>::new();
        let mut strings = HashMap::new();
        let mut menu_item_strings = HashMap::new();
        for descriptor in settings_list.all_descriptors() {
            let id = &descriptor.id;
            let id_str = CString::new(descriptor.id.id.to_string()).unwrap();
            let label = CString::new(descriptor.label).unwrap();
            let description = descriptor.description.map(|d| CString::new(d).unwrap());
            let group_name = if let SettingKind::Group { .. } = descriptor.kind {
                Some(CString::new(format!("{}_group", descriptor.id.id)).unwrap())
            } else {
                None
            };
            strings.insert(id.clone(), (id_str, label, description, group_name));
            if let SettingKind::Enumeration { options } = &descriptor.kind {
                for item in options {
                    let lbl = CString::new(item.label).unwrap();
                    menu_item_strings.insert(
                        (id.clone(), item.index),
                        (lbl, item.description.map(|d| CString::new(d).unwrap())),
                    );
                }
            }
        }

        Ok(SharedData {
            host_info,
            property_suite: property_suite.as_ref().ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            image_effect_suite: image_effect_suite.as_ref().ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            memory_suite: memory_suite.as_ref().ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            parameter_suite: parameter_suite.as_ref().ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            settings_list,
            supports_multiple_clip_depths: AtomicBool::new(false),
            strings,
            menu_item_strings,
        })
    }
}

fn shared() -> &'static SharedData {
    SHARED_DATA.get().expect("SharedData not initialized")
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn OfxGetNumberOfPlugins() -> c_int { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn OfxGetPlugin(nth: c_int) -> *const OfxPlugin {
    if nth != 0 { return ptr::null(); }
    std::panic::set_hook(Box::new(|info| { println!("{info:?}"); }));
    let pi = PLUGIN_INFO.get_or_init(|| OfxPlugin {
        pluginApi: kOfxImageEffectPluginApi.as_ptr(),
        apiVersion: 1,
        pluginIdentifier: c"com.example:zzzStroke".as_ptr(),
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
    SHARED_DATA.get_or_init(|| SharedData::new(HostInfo { host: h, fetch_suite: fs }).unwrap());
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
    let d = shared();
    let pg = d.property_suite.propGetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let mut v: c_int = 0;
    pg(d.host_info.host as *const _ as _, kOfxImageEffectPropSupportsMultipleClipDepths.as_ptr(), 0, &mut v).ofx_ok()?;
    d.supports_multiple_clip_depths.store(v != 0, Ordering::Release);
    Ok(())
}

unsafe fn action_describe(desc: OfxImageEffectHandle) -> OfxResult<()> {
    let d = shared();
    let mut ep: OfxPropertySetHandle = ptr::null_mut();
    (d.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?)(desc, &mut ep).ofx_ok()?;
    let ps = d.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pi = d.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;

    ps(ep, kOfxPropLabel.as_ptr(), 0, c"zzzStroke".as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginPropGrouping.as_ptr(), 0, c"zzz".as_ptr()).ofx_ok()?;
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
    let d = shared();
    let cd = d.image_effect_suite.clipDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let gp = d.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = d.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pd = d.property_suite.propSetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let pi = d.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let pdef = d.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let defaults = ZzzStrokeFullSettings::default();

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

    // --- Block A: Params before Stroke Color (IDs 200-202) ---
    for desc in d.settings_list.setting_descriptors.iter() {
        if desc.id.id > 202 {
            break;
        }
        if is_native_grouped(desc.id.id) {
            continue;
        }
        define_single_param(d, param_set, desc, &defaults, c"")?;
    }

    // --- Native RGBA: Stroke Color (descriptors 203-206) ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeRGBA.as_ptr(), STROKE_COLOR_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, c"Stroke Color".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, c"Color of the stroke.".as_ptr()).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 1.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 1.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 2, 1.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 3, 1.0).ofx_ok()?;
    }

    // --- Block C: Params after Stroke Color (IDs 207+) ---
    for desc in d.settings_list.setting_descriptors.iter() {
        if desc.id.id < 207 {
            continue;
        }
        if is_native_grouped(desc.id.id) {
            continue;
        }
        if let SettingKind::Group { .. } = &desc.kind {
            // Create gradient group container with GRADIENT_GROUP_PARAM name
            let ds = d.strings.get(&desc.id).unwrap();
            let id_cstr = ds.0.as_c_str();
            let dv = defaults.get_field::<bool>(&desc.id).map_err(|_| OfxStat::kOfxStatFailed)?;
            let mut gp: OfxPropertySetHandle = ptr::null_mut();
            pdef(param_set, kOfxParamTypeGroup.as_ptr(), GRADIENT_GROUP_PARAM.as_ptr(), &mut gp).ofx_ok()?;
            ps(gp, kOfxPropLabel.as_ptr(), 0, ds.1.as_ptr()).ofx_ok()?;
            if let Some(desc_text) = ds.2.as_deref() {
                ps(gp, kOfxParamPropHint.as_ptr(), 0, desc_text.as_ptr()).ofx_ok()?;
            }
            // Checkbox
            let mut cb: OfxPropertySetHandle = ptr::null_mut();
            pdef(param_set, kOfxParamTypeBoolean.as_ptr(), id_cstr.as_ptr(), &mut cb).ofx_ok()?;
            ps(cb, kOfxPropLabel.as_ptr(), 0, c"Enabled".as_ptr()).ofx_ok()?;
            pi(cb, kOfxParamPropDefault.as_ptr(), 0, dv as i32).ofx_ok()?;
            ps(cb, kOfxParamPropParent.as_ptr(), 0, GRADIENT_GROUP_PARAM.as_ptr()).ofx_ok()?;
            pi(cb, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
        } else {
            define_single_param(d, param_set, desc, &defaults, c"")?;
        }
    }

    // --- Native Double2D: Gradient Start (descriptors 213-214) ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeDouble2D.as_ptr(), GRADIENT_START_POS_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, c"Gradient Start".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, c"Normalized start position (0-1).".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, GRADIENT_GROUP_PARAM.as_ptr()).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 0.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 0.0).ofx_ok()?;
    }

    // --- Native RGBA: Gradient Start Color (descriptors 215-218) ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeRGBA.as_ptr(), GRADIENT_START_COLOR_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, c"Gradient Start Color".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, c"Color at gradient start.".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, GRADIENT_GROUP_PARAM.as_ptr()).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 0.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 0.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 2, 0.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 3, 1.0).ofx_ok()?;
    }

    // --- Native Double2D: Gradient End (descriptors 219-220) ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeDouble2D.as_ptr(), GRADIENT_END_POS_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, c"Gradient End".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, c"Normalized end position (0-1).".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, GRADIENT_GROUP_PARAM.as_ptr()).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 1.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 1.0).ofx_ok()?;
    }

    // --- Native RGBA: Gradient End Color (descriptors 221-224) ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeRGBA.as_ptr(), GRADIENT_END_COLOR_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, c"Gradient End Color".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, c"Color at gradient end.".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, GRADIENT_GROUP_PARAM.as_ptr()).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 1.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 1.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 2, 1.0).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 3, 1.0).ofx_ok()?;
    }

    Ok(())
}

unsafe fn action_get_regions_of_interest(
    effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
    outArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let d = shared();
    let pg = d.property_suite.propGetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let psn = d.property_suite.propSetDoubleN.ok_or(OfxStat::kOfxStatFailed)?;
    let cgh = d.image_effect_suite.clipGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let crod = d.image_effect_suite.clipGetRegionOfDefinition.ok_or(OfxStat::kOfxStatFailed)?;

    let mut sc: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"Source".as_ptr(), &mut sc, ptr::null_mut()).ofx_ok()?;
    let mut rod = OfxRectD { x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 };
    let mut t: OfxTime = 0.0;
    pg(inArgs, kOfxPropTime.as_ptr(), 0, &mut t).ofx_ok()?;
    crod(sc, t, &mut rod).ofx_ok()?;

    psn(outArgs, c"OfxImageClipPropRoI_Source".as_ptr(), 4, ptr::addr_of_mut!(rod) as *mut _).ofx_ok()?;
    Ok(())
}

unsafe fn action_get_clip_preferences(outArgs: OfxPropertySetHandle) -> OfxResult<()> {
    let d = shared();
    let pi = d.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = d.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    pi(outArgs, kOfxImageEffectFrameVarying.as_ptr(), 0, 0).ofx_ok()?;
    ps(outArgs, kOfxImageEffectPropPreMultiplication.as_ptr(), 0, kOfxImageOpaque.as_ptr()).ofx_ok()?;
    Ok(())
}

unsafe fn action_instance_changed(
    _effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let d = shared();
    let pg = d.property_suite.propGetInt.ok_or(OfxStat::kOfxStatFailed)?;
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
    let d = shared();
    let pss = d.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pgd = d.property_suite.propGetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let gps = d.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;

    let mut time: OfxTime = 0.0;
    pgd(inArgs, kOfxPropTime.as_ptr(), 0, &mut time).ofx_ok()?;

    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gps(effect, &mut param_set).ofx_ok()?;

    let mut settings = ZzzStrokeFullSettings::default();
    apply_params(d, param_set, time, &mut settings)?;
    let stroke: ZzzStroke = (&settings).into();

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
    let d = shared();

    let cgh = d.image_effect_suite.clipGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let cgi = d.image_effect_suite.clipGetImage.ok_or(OfxStat::kOfxStatFailed)?;
    let cri = d.image_effect_suite.clipReleaseImage.ok_or(OfxStat::kOfxStatFailed)?;
    let pgp = d.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    let pgi = d.property_suite.propGetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let pgd = d.property_suite.propGetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let gps = d.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;

    let mut time: OfxTime = 0.0;
    pgd(inArgs, kOfxPropTime.as_ptr(), 0, &mut time).ofx_ok()?;

    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gps(effect, &mut param_set).ofx_ok()?;

    let mut settings = ZzzStrokeFullSettings::default();
    apply_params(d, param_set, time, &mut settings)?;
    let stroke: ZzzStroke = (&settings).into();

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

    let width = (r - l) as usize;
    let height = (t - b) as usize;
    let s_stride = srb.max(0) as usize;
    let d_stride = drb.max(0) as usize;
    let row_bytes = width * 4;
    let total = row_bytes * height;

    let mut src_buf = vec![0u8; total];
    let mut dst_buf = vec![0u8; total];
    for y in 0..height {
        ptr::copy_nonoverlapping(
            (sp as *const u8).add(y * s_stride),
            src_buf.as_mut_ptr().add(y * row_bytes),
            row_bytes,
        );
    }
    stroke.apply_effect(&src_buf, &mut dst_buf, width, height);
    for y in 0..height {
        ptr::copy_nonoverlapping(
            dst_buf.as_ptr().add(y * row_bytes),
            (dp as *mut u8).add(y * d_stride),
            row_bytes,
        );
    }

    let _ = cri(si);
    let _ = cri(di);
    Ok(())
}

// ---------------------------------------------------------------------------
// Parameter creation helpers
// ---------------------------------------------------------------------------

unsafe fn define_single_param(
    data: &'static SharedData,
    param_set: OfxParamSetHandle,
    descriptor: &SettingDescriptor<ZzzStrokeFullSettings>,
    default_settings: &ZzzStrokeFullSettings,
    parent: &CStr,
) -> OfxResult<()> {
    let pdef = data.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let pd = data.property_suite.propSetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let pi = data.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = data.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;

    let ds = data.strings.get(&descriptor.id).unwrap();
    let id_cstr = ds.0.as_c_str();
    let mut pp: OfxPropertySetHandle = ptr::null_mut();

    match &descriptor.kind {
        SettingKind::Enumeration { options } => {
            pdef(param_set, kOfxParamTypeChoice.as_ptr(), id_cstr.as_ptr(), &mut pp).ofx_ok()?;
            let dv = default_settings.get_field::<EnumValue>(&descriptor.id).map_err(|_| OfxStat::kOfxStatFailed)?.0;
            let mut di: usize = 0;
            for (i, mi) in options.iter().enumerate() {
                let is = data.menu_item_strings.get(&(descriptor.id.clone(), mi.index)).unwrap();
                ps(pp, kOfxParamPropChoiceOption.as_ptr(), i as i32, is.0.as_c_str().as_ptr()).ofx_ok()?;
                if mi.index == dv { di = i; }
            }
            pi(pp, kOfxParamPropDefault.as_ptr(), 0, di as i32).ofx_ok()?;
        }
        SettingKind::Percentage { .. } => {
            let dv = default_settings.get_field::<f32>(&descriptor.id).map_err(|_| OfxStat::kOfxStatFailed)?;
            pdef(param_set, kOfxParamTypeDouble.as_ptr(), id_cstr.as_ptr(), &mut pp).ofx_ok()?;
            ps(pp, kOfxParamPropDoubleType.as_ptr(), 0, kOfxParamDoubleTypeScale.as_ptr()).ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 0, dv as f64).ofx_ok()?;
            pd(pp, kOfxParamPropMin.as_ptr(), 0, 0.0).ofx_ok()?;
            pd(pp, kOfxParamPropDisplayMin.as_ptr(), 0, 0.0).ofx_ok()?;
            pd(pp, kOfxParamPropMax.as_ptr(), 0, 1.0).ofx_ok()?;
            pd(pp, kOfxParamPropDisplayMax.as_ptr(), 0, 1.0).ofx_ok()?;
        }
        SettingKind::FloatRange { range, .. } => {
            let dv = default_settings.get_field::<f32>(&descriptor.id).map_err(|_| OfxStat::kOfxStatFailed)?;
            pdef(param_set, kOfxParamTypeDouble.as_ptr(), id_cstr.as_ptr(), &mut pp).ofx_ok()?;
            pd(pp, kOfxParamPropDefault.as_ptr(), 0, dv as f64).ofx_ok()?;
            pd(pp, kOfxParamPropMin.as_ptr(), 0, *range.start() as f64).ofx_ok()?;
            pd(pp, kOfxParamPropDisplayMin.as_ptr(), 0, *range.start() as f64).ofx_ok()?;
            pd(pp, kOfxParamPropMax.as_ptr(), 0, *range.end() as f64).ofx_ok()?;
            pd(pp, kOfxParamPropDisplayMax.as_ptr(), 0, *range.end() as f64).ofx_ok()?;
        }
        SettingKind::IntRange { range } => {
            let dv = default_settings.get_field::<i32>(&descriptor.id).map_err(|_| OfxStat::kOfxStatFailed)?;
            pdef(param_set, kOfxParamTypeInteger.as_ptr(), id_cstr.as_ptr(), &mut pp).ofx_ok()?;
            pi(pp, kOfxParamPropDefault.as_ptr(), 0, dv).ofx_ok()?;
            pi(pp, kOfxParamPropMin.as_ptr(), 0, *range.start()).ofx_ok()?;
            pi(pp, kOfxParamPropDisplayMin.as_ptr(), 0, *range.start()).ofx_ok()?;
            pi(pp, kOfxParamPropMax.as_ptr(), 0, *range.end()).ofx_ok()?;
            pi(pp, kOfxParamPropDisplayMax.as_ptr(), 0, *range.end()).ofx_ok()?;
        }
        SettingKind::Boolean => {
            let dv = default_settings.get_field::<bool>(&descriptor.id).map_err(|_| OfxStat::kOfxStatFailed)?;
            pdef(param_set, kOfxParamTypeBoolean.as_ptr(), id_cstr.as_ptr(), &mut pp).ofx_ok()?;
            pi(pp, kOfxParamPropDefault.as_ptr(), 0, dv as i32).ofx_ok()?;
        }
        SettingKind::Group { children } => {
            let dv = default_settings.get_field::<bool>(&descriptor.id).map_err(|_| OfxStat::kOfxStatFailed)?;
            let gnc: &CStr = ds.3.as_ref().expect("group name").as_c_str();
            pdef(param_set, kOfxParamTypeGroup.as_ptr(), gnc.as_ptr(), &mut pp).ofx_ok()?;
            let mut cb: OfxPropertySetHandle = ptr::null_mut();
            pdef(param_set, kOfxParamTypeBoolean.as_ptr(), id_cstr.as_ptr(), &mut cb).ofx_ok()?;
            ps(cb, kOfxPropLabel.as_ptr(), 0, c"Enabled".as_ptr()).ofx_ok()?;
            pi(cb, kOfxParamPropDefault.as_ptr(), 0, dv as i32).ofx_ok()?;
            ps(cb, kOfxParamPropParent.as_ptr(), 0, gnc.as_ptr()).ofx_ok()?;
            pi(cb, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;
            for child in children {
                define_single_param(data, param_set, child, default_settings, gnc)?;
            }
        }
    }

    if !pp.is_null() {
        ps(pp, kOfxPropLabel.as_ptr(), 0, ds.1.as_ptr()).ofx_ok()?;
        if let Some(desc) = ds.2.as_deref() {
            ps(pp, kOfxParamPropHint.as_ptr(), 0, desc.as_ptr()).ofx_ok()?;
        }
        ps(pp, kOfxParamPropParent.as_ptr(), 0, parent.as_ptr()).ofx_ok()?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Parameter reading
// ---------------------------------------------------------------------------

unsafe fn apply_params(
    data: &'static SharedData,
    param_set: OfxParamSetHandle,
    time: f64,
    dst: &mut ZzzStrokeFullSettings,
) -> OfxResult<()> {
    let pgh = data.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = data.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    // Collect gradient children IDs from the group descriptor
    let grad_children = {
        let td = &data.settings_list.setting_descriptors;
        let group_desc = td.iter().find(|d| d.id.id == 211).unwrap();
        if let SettingKind::Group { children } = &group_desc.kind {
            children.clone()
        } else {
            unreachable!()
        }
    };

    // --- Native RGBA: Stroke Color (→ descriptors 203-206) ---
    {
        let td = &data.settings_list.setting_descriptors;
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, STROKE_COLOR_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut r: f64 = 0.0; let mut g: f64 = 0.0;
        let mut b: f64 = 0.0; let mut a: f64 = 0.0;
        pgv(p, time, &mut r, &mut g, &mut b, &mut a).ofx_ok()?;
        dst.set_field::<f32>(&td[3].id, r as f32).unwrap();
        dst.set_field::<f32>(&td[4].id, g as f32).unwrap();
        dst.set_field::<f32>(&td[5].id, b as f32).unwrap();
        dst.set_field::<f32>(&td[6].id, a as f32).unwrap();
    }

    // --- Native Double2D: Gradient Start (→ descriptors 213-214) ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, GRADIENT_START_POS_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut x: f64 = 0.0; let mut y: f64 = 0.0;
        pgv(p, time, &mut x, &mut y).ofx_ok()?;
        dst.set_field::<f32>(&grad_children[0].id, x as f32).unwrap();
        dst.set_field::<f32>(&grad_children[1].id, y as f32).unwrap();
    }

    // --- Native RGBA: Gradient Start Color (→ descriptors 215-218) ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, GRADIENT_START_COLOR_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut r: f64 = 0.0; let mut g: f64 = 0.0;
        let mut b: f64 = 0.0; let mut a: f64 = 0.0;
        pgv(p, time, &mut r, &mut g, &mut b, &mut a).ofx_ok()?;
        dst.set_field::<f32>(&grad_children[2].id, r as f32).unwrap();
        dst.set_field::<f32>(&grad_children[3].id, g as f32).unwrap();
        dst.set_field::<f32>(&grad_children[4].id, b as f32).unwrap();
        dst.set_field::<f32>(&grad_children[5].id, a as f32).unwrap();
    }

    // --- Native Double2D: Gradient End (→ descriptors 219-220) ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, GRADIENT_END_POS_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut x: f64 = 0.0; let mut y: f64 = 0.0;
        pgv(p, time, &mut x, &mut y).ofx_ok()?;
        dst.set_field::<f32>(&grad_children[6].id, x as f32).unwrap();
        dst.set_field::<f32>(&grad_children[7].id, y as f32).unwrap();
    }

    // --- Native RGBA: Gradient End Color (→ descriptors 221-224) ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, GRADIENT_END_COLOR_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut r: f64 = 0.0; let mut g: f64 = 0.0;
        let mut b: f64 = 0.0; let mut a: f64 = 0.0;
        pgv(p, time, &mut r, &mut g, &mut b, &mut a).ofx_ok()?;
        dst.set_field::<f32>(&grad_children[8].id, r as f32).unwrap();
        dst.set_field::<f32>(&grad_children[9].id, g as f32).unwrap();
        dst.set_field::<f32>(&grad_children[10].id, b as f32).unwrap();
        dst.set_field::<f32>(&grad_children[11].id, a as f32).unwrap();
    }

    // --- Read remaining generic params (skip grouped + group checkbox handled separately) ---
    for desc in data.settings_list.setting_descriptors.iter() {
        if is_native_grouped(desc.id.id) {
            continue;
        }
        if let SettingKind::Group { .. } = &desc.kind {
            // Read checkbox value only (native children already read above)
            let ds = data.strings.get(&desc.id).unwrap();
            let id_cstr = ds.0.as_c_str();
            let mut p: OfxParamHandle = ptr::null_mut();
            pgh(param_set, id_cstr.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
            let mut v: c_int = 0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<bool>(&desc.id, v != 0).unwrap();
        } else {
            read_generic_param(data, param_set, time, desc, dst)?;
        }
    }

    Ok(())
}

unsafe fn read_generic_param(
    data: &'static SharedData,
    param_set: OfxParamSetHandle,
    time: f64,
    desc: &SettingDescriptor<ZzzStrokeFullSettings>,
    dst: &mut ZzzStrokeFullSettings,
) -> OfxResult<()> {
    let pgh = data.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = data.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
    let ds = data.strings.get(&desc.id).unwrap();
    let id_cstr = ds.0.as_c_str();

    let mut p: OfxParamHandle = ptr::null_mut();
    pgh(param_set, id_cstr.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;

    match &desc.kind {
        SettingKind::Enumeration { options } => {
            let mut idx: c_int = 0;
            pgv(p, time, &mut idx).ofx_ok()?;
            if idx >= 0 && (idx as usize) < options.len() {
                dst.set_field::<EnumValue>(&desc.id, EnumValue(options[idx as usize].index)).unwrap();
            }
        }
        SettingKind::Percentage { .. } | SettingKind::FloatRange { .. } => {
            let mut v: f64 = 0.0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<f32>(&desc.id, v.clamp(0.0, 1.0) as f32).unwrap();
        }
        SettingKind::IntRange { range } => {
            let mut v: c_int = 0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<i32>(&desc.id, v.clamp(*range.start(), *range.end())).unwrap();
        }
        SettingKind::Boolean => {
            let mut v: c_int = 0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<bool>(&desc.id, v != 0).unwrap();
        }
        SettingKind::Group { .. } => {
            // Already handled in caller
        }
    }
    Ok(())
}
