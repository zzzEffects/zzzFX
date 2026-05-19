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
    ZzzRepeater, ZzzRepeaterFullSettings, ZzzStrokeBlendMode,
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
const POSITION_PARAM: &CStr = c"position";

/// Returns true if the parameter with the given name is handled by a native
/// OFX param type (Double2D for Position) rather than a generic single param.
fn is_native_grouped_name(name: &str) -> bool {
    matches!(name, "position_x" | "position_y")
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
    settings_list: SettingsList<ZzzRepeaterFullSettings>,
    supports_multiple_clip_depths: AtomicBool,
    strings: HashMap<SettingID<ZzzRepeaterFullSettings>, (CString, CString, Option<CString>, Option<CString>)>,
    menu_item_strings: HashMap<(SettingID<ZzzRepeaterFullSettings>, u32), (CString, Option<CString>)>,
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

        let settings_list = SettingsList::<ZzzRepeaterFullSettings>::new();
        let mut strings = HashMap::new();
        let mut menu_item_strings = HashMap::new();
        for descriptor in settings_list.all_descriptors() {
            let id = &descriptor.id;
            let id_str = CString::new(descriptor.id.name).unwrap();
            let label = CString::new(descriptor.label).unwrap();
            let description = descriptor.description.map(|d| CString::new(d).unwrap());
            let group_name = if let SettingKind::Group { .. } = descriptor.kind {
                Some(CString::new(format!("{}_group", descriptor.id.name)).unwrap())
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

    ps(ep, kOfxPropLabel.as_ptr(), 0, c"zzzRepeater".as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginPropGrouping.as_ptr(), 0, c"zzz".as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedContexts.as_ptr(), 0, kOfxImageEffectContextFilter.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedContexts.as_ptr(), 1, kOfxImageEffectContextGeneral.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 0, kOfxBitDepthFloat.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 1, kOfxBitDepthShort.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPropSupportedPixelDepths.as_ptr(), 2, kOfxBitDepthByte.as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginRenderThreadSafety.as_ptr(), 0, kOfxImageEffectRenderFullySafe.as_ptr()).ofx_ok()?;
    pi(ep, kOfxImageEffectPluginPropHostFrameThreading.as_ptr(), 0, 0).ofx_ok()?;
    pi(ep, kOfxImageEffectPropSupportsTiles.as_ptr(), 0, 0).ofx_ok()?;
    // Signal that this effect needs random temporal access to source clips.
    // Required by the OFX spec for kOfxImageEffectActionGetTimeDomain to be
    // called, and prevents host-side caching based solely on parameter values.
    pi(ep, kOfxImageEffectPropTemporalClipAccess.as_ptr(), 0, 1).ofx_ok()?;
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
    let defaults = ZzzRepeaterFullSettings::default();

    // --- Output / Source clips ---
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    cd(desc, c"Output".as_ptr(), &mut props).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 0, kOfxImageComponentRGBA.as_ptr()).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 1, kOfxImageComponentRGB.as_ptr()).ofx_ok()?;
    cd(desc, c"Source".as_ptr(), &mut props).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 0, kOfxImageComponentRGBA.as_ptr()).ofx_ok()?;
    ps(props, kOfxImageEffectPropSupportedComponents.as_ptr(), 1, kOfxImageComponentRGB.as_ptr()).ofx_ok()?;
    // Tell the host we need random temporal access to the Source clip.
    // This prevents hosts (e.g. VEGAS Pro) from caching rendered frames
    // based solely on parameter values, since our output depends on the
    // time position of keyframes and source content at varying times.
    pi(props, kOfxImageEffectPropTemporalClipAccess.as_ptr(), 0, 1).ofx_ok()?;

    // --- Parameter set ---
    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gp(desc, &mut param_set).ofx_ok()?;

    // --- Block A: Params before Position ---
    for desc in d.settings_list.setting_descriptors.iter() {
        if desc.id.name == "position_x" {
            break;
        }
        if is_native_grouped_name(desc.id.name) {
            continue;
        }
        define_single_param(d, param_set, desc, &defaults, c"")?;
    }

    // --- Native Double2D: Position ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypeDouble2D.as_ptr(), POSITION_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, c"Position".as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, c"Position of the repeat layer (0-1 normalized).".as_ptr()).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 0, 0.5).ofx_ok()?;
        pd(pp, kOfxParamPropDefault.as_ptr(), 1, 0.5).ofx_ok()?;
    }

    // --- Block C: Remaining params (rotation onwards) ---
    let mut after_position = false;
    for desc in d.settings_list.setting_descriptors.iter() {
        if desc.id.name == "position_x" {
            after_position = true;
            continue;
        }
        if !after_position || desc.id.name == "position_y" {
            continue;
        }
        if is_native_grouped_name(desc.id.name) {
            continue;
        }
        define_single_param(d, param_set, desc, &defaults, c"")?;
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

unsafe fn action_get_time_domain(
    inArgs: OfxPropertySetHandle,
    outArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let d = shared();
    let pg = d.property_suite.propGetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let psn = d.property_suite.propSetDoubleN.ok_or(OfxStat::kOfxStatFailed)?;

    let mut t: OfxTime = 0.0;
    pg(inArgs, kOfxPropTime.as_ptr(), 0, &mut t).ofx_ok()?;

    // Declare that we may access Source frames from time 0 up to the
    // current render time. This is conservative but correct: the original
    // layer accesses max(0, t - timeOffset), and repeat layers access
    // kv + t - kt which falls in [0, t] for typical kv=0 keyframes.
    //
    // Implementing this action in combination with
    // kOfxImageEffectPropTemporalClipAccess signals to the host (e.g.
    // VEGAS Pro) that our output depends on source content across a
    // time range, not just on current parameter values. This prevents
    // hosts from incorrectly caching rendered frames when parameter
    // values happen to be unchanged between keyframes.
    let mut range = [0.0, t];
    psn(
        outArgs,
        c"OfxImageClipPropFrameRange_Source".as_ptr(),
        2,
        range.as_mut_ptr() as *mut _,
    )
    .ofx_ok()?;

    Ok(())
}

unsafe fn action_get_clip_preferences(outArgs: OfxPropertySetHandle) -> OfxResult<()> {
    let d = shared();
    let pi = d.property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = d.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    pi(outArgs, kOfxImageEffectFrameVarying.as_ptr(), 0, 1).ofx_ok()?;
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
    let pgh = d.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgnk = d.parameter_suite.paramGetNumKeys.ok_or(OfxStat::kOfxStatFailed)?;

    let mut time: OfxTime = 0.0;
    pgd(inArgs, kOfxPropTime.as_ptr(), 0, &mut time).ofx_ok()?;

    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gps(effect, &mut param_set).ofx_ok()?;

    let mut settings = ZzzRepeaterFullSettings::default();
    apply_params(d, param_set, time, &mut settings)?;

    // Check keyframes on Time Offset parameter
    let ds = d.strings.iter().find(|(k, _)| k.name == "time_offset").unwrap();
    let id_cstr = ds.1.0.as_c_str();
    let mut p: OfxParamHandle = ptr::null_mut();
    pgh(param_set, id_cstr.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;

    let mut num_keys: u32 = 0;
    pgnk(p, &mut num_keys).ofx_ok()?;

    // Identity when no keyframes at t > 0 and time_offset == 0
    // If there are keyframes, check if all are at time 0 (the original layer keyframe)
    let has_active_keyframes = if num_keys == 0 {
        false
    } else if num_keys == 1 {
        let pgkt = d.parameter_suite.paramGetKeyTime.ok_or(OfxStat::kOfxStatFailed)?;
        let mut kt0: f64 = 0.0;
        pgkt(p, 0, &mut kt0).ofx_ok()?;
        kt0 > 0.0
    } else {
        true
    };

    // Identity only when ALL of these conditions are met:
    //   - No keyframes on Time Offset at t > 0 (no repeat layers)
    //   - time_offset == 0 (original layer shows current source frame)
    //   - position == (0.5, 0.5) (center, no offset)
    //   - rotation == 0 (no rotation)
    //   - blend_mode == Normal (passthrough blending)
    //   - layer_order == Above (only one layer, order irrelevant, but be explicit)
    //
    // This is the ONLY configuration where the output is guaranteed
    // identical to the source. A more lax check (e.g. ignoring position
    // or blend mode) would cause the host to incorrectly skip rendering,
    // producing visible "flash frame" artifacts in VEGAS Pro and other
    // hosts that cache based on parameter values.
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
    let d = shared();

    let cgh = d.image_effect_suite.clipGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let cgi = d.image_effect_suite.clipGetImage.ok_or(OfxStat::kOfxStatFailed)?;
    let cri = d.image_effect_suite.clipReleaseImage.ok_or(OfxStat::kOfxStatFailed)?;
    let pgp = d.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    let pgi = d.property_suite.propGetInt.ok_or(OfxStat::kOfxStatFailed)?;
    let pgd = d.property_suite.propGetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let pgs = d.property_suite.propGetString.ok_or(OfxStat::kOfxStatFailed)?;
    let gps = d.image_effect_suite.getParamSet.ok_or(OfxStat::kOfxStatFailed)?;
    let pgh = d.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = d.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;
    let pgnk = d.parameter_suite.paramGetNumKeys.ok_or(OfxStat::kOfxStatFailed)?;
    let pgkt = d.parameter_suite.paramGetKeyTime.ok_or(OfxStat::kOfxStatFailed)?;

    let mut time: OfxTime = 0.0;
    pgd(inArgs, kOfxPropTime.as_ptr(), 0, &mut time).ofx_ok()?;

    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    gps(effect, &mut param_set).ofx_ok()?;

    let mut settings = ZzzRepeaterFullSettings::default();
    apply_params(d, param_set, time, &mut settings)?;

    let mut sc: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"Source".as_ptr(), &mut sc, ptr::null_mut()).ofx_ok()?;
    let mut dc: OfxImageClipHandle = ptr::null_mut();
    cgh(effect, c"Output".as_ptr(), &mut dc, ptr::null_mut()).ofx_ok()?;

    // --- Build layer list ---

    // Get Time Offset param handle
    let to_ds = d.strings.iter().find(|(k, _)| k.name == "time_offset").unwrap();
    let to_id_cstr = to_ds.1.0.as_c_str();
    let mut to_p: OfxParamHandle = ptr::null_mut();
    pgh(param_set, to_id_cstr.as_ptr(), &mut to_p, ptr::null_mut()).ofx_ok()?;

    // Get Position param handle
    let pos_ds = d.strings.iter().find(|(k, _)| k.name == "position_x").unwrap();
    let _pos_id_cstr = pos_ds.1.0.as_c_str();

    // Get Rotation param handle name
    let rot_ds = d.strings.iter().find(|(k, _)| k.name == "rotation").unwrap();
    let rot_id_cstr = rot_ds.1.0.as_c_str();

    struct LayerInfo {
        source_time: f64,
        position_x: f32,
        position_y: f32,
        rotation: f32,
    }

    // Read position/rotation at a specific time via native Double2D and generic Double params
    let read_position_at_time = |param_set: OfxParamSetHandle, t: f64| -> OfxResult<(f32, f32)> {
        let mut pp: OfxParamHandle = ptr::null_mut();
        pgh(param_set, POSITION_PARAM.as_ptr(), &mut pp, ptr::null_mut()).ofx_ok()?;
        let mut x: f64 = 0.0;
        let mut y: f64 = 0.0;
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

    // Original layer (creation time = 0.0)
    let (opx, opy) = read_position_at_time(param_set, 0.0)?;
    let orot = read_rotation_at_time(param_set, 0.0)?;

    let mut layers = vec![LayerInfo {
        source_time: (time - settings.time_offset as f64).max(0.0),
        position_x: opx,
        position_y: opy,
        rotation: orot,
    }];

    // Enumerate keyframes on Time Offset
    let mut num_keys: u32 = 0;
    pgnk(to_p, &mut num_keys).ofx_ok()?;

    for i in 0..num_keys {
        let mut kt: f64 = 0.0;
        pgkt(to_p, i, &mut kt).ofx_ok()?;

        // Keyframe at time 0 is the original layer's trigger; skip it for repeats
        if kt <= 0.0 || kt > time {
            continue;
        }

        let mut kv: f64 = 0.0;
        pgv(to_p, kt, &mut kv).ofx_ok()?;

        let (lpx, lpy) = read_position_at_time(param_set, kt)?;
        let lrot = read_rotation_at_time(param_set, kt)?;

        layers.push(LayerInfo {
            source_time: (kv + time - kt).max(0.0),
            position_x: lpx,
            position_y: lpy,
            rotation: lrot,
        });
    }

    // Trim by maxLayers
    let max_layers = settings.max_layers as usize;
    if max_layers > 0 && layers.len() > max_layers {
        let skip = layers.len() - max_layers;
        layers.drain(0..skip);
    }

    // --- Fetch source images for all layers ---

    // Determine pixel format from first source image fetch
    let mut si0: OfxPropertySetHandle = ptr::null_mut();
    cgi(sc, layers[0].source_time, ptr::null(), &mut si0).ofx_ok()?;

    let mut l: c_int = 0; let mut b: c_int = 0;
    let mut r: c_int = 0; let mut t: c_int = 0;
    pgi(si0, kOfxImagePropBounds.as_ptr(), 0, &mut l).ofx_ok()?;
    pgi(si0, kOfxImagePropBounds.as_ptr(), 1, &mut b).ofx_ok()?;
    pgi(si0, kOfxImagePropBounds.as_ptr(), 2, &mut r).ofx_ok()?;
    pgi(si0, kOfxImagePropBounds.as_ptr(), 3, &mut t).ofx_ok()?;

    let width = (r - l) as usize;
    let height = (t - b) as usize;
    let row_bytes_u8 = width * 4;
    let total_u8 = row_bytes_u8 * height;

    // Determine pixel depth
    let mut depth_ptr: *mut c_char = ptr::null_mut();
    let depth = (|| {
        pgs(si0, kOfxImageEffectPropPixelDepth.as_ptr(), 0, &mut depth_ptr)
            .ofx_ok()
            .ok()?;
        let s = CStr::from_ptr(depth_ptr);
        if s == kOfxBitDepthFloat {
            Some(16usize)
        } else if s == kOfxBitDepthShort {
            Some(8usize)
        } else if s == kOfxBitDepthByte {
            Some(4usize)
        } else {
            None
        }
    })()
    .unwrap_or(4);

    let mut srb0: c_int = 0;
    pgi(si0, kOfxImagePropRowBytes.as_ptr(), 0, &mut srb0).ofx_ok()?;
    let s_stride0 = srb0.max(0) as usize;

    // Copy first layer's pixels
    let copy_source_to_u8 = |si: OfxPropertySetHandle, s_stride: usize| -> Vec<u8> {
        let mut sp: *mut c_void = ptr::null_mut();
        let _ = pgp(si, kOfxImagePropData.as_ptr(), 0, &mut sp);
        if sp.is_null() {
            return vec![0u8; total_u8];
        }
        let mut buf = vec![0u8; total_u8];
        match depth {
            4 => {
                for y in 0..height {
                    ptr::copy_nonoverlapping(
                        (sp as *const u8).add(y * s_stride),
                        buf.as_mut_ptr().add(y * row_bytes_u8),
                        row_bytes_u8,
                    );
                }
            }
            8 => {
                for y in 0..height {
                    let host_row = (sp as *const u8).add(y * s_stride) as *const u16;
                    let u8_row = buf.as_mut_ptr().add(y * row_bytes_u8);
                    for x in 0..(width * 4) {
                        let v = *host_row.add(x) as u32;
                        *u8_row.add(x) = ((v * 255 + 32767) / 65535) as u8;
                    }
                }
            }
            _ => {
                for y in 0..height {
                    let host_row = (sp as *const u8).add(y * s_stride) as *const f32;
                    let u8_row = buf.as_mut_ptr().add(y * row_bytes_u8);
                    for x in 0..(width * 4) {
                        let v = *host_row.add(x);
                        *u8_row.add(x) = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                }
            }
        }
        buf
    };

    let mut layer_bufs: Vec<Vec<u8>> = Vec::with_capacity(layers.len());
    layer_bufs.push(copy_source_to_u8(si0, s_stride0));
    cri(si0).ofx_ok()?;

    // Fetch remaining layers
    for i in 1..layers.len() {
        let mut si: OfxPropertySetHandle = ptr::null_mut();
        cgi(sc, layers[i].source_time, ptr::null(), &mut si).ofx_ok()?;

        let mut srb: c_int = 0;
        pgi(si, kOfxImagePropRowBytes.as_ptr(), 0, &mut srb).ofx_ok()?;
        let s_stride = srb.max(0) as usize;

        layer_bufs.push(copy_source_to_u8(si, s_stride));
        cri(si).ofx_ok()?;
    }

    // --- Composite ---

    let repeater: ZzzRepeater = (&settings).into();
    let bmode = settings.blend_mode;

    let compositor_layers: Vec<example_effect::CompositorLayer> = layers
        .iter()
        .zip(layer_bufs.iter())
        .map(|(info, buf)| example_effect::CompositorLayer {
            rgba: buf.as_slice(),
            position_x: info.position_x,
            position_y: info.position_y,
            rotation_deg: info.rotation,
            blend_mode: bmode,
        })
        .collect();

    let mut dst_buf = vec![0u8; total_u8];
    repeater.composite_layers(&compositor_layers, &mut dst_buf, width, height);

    // --- Write output ---

    let mut di: OfxPropertySetHandle = ptr::null_mut();
    cgi(dc, time, ptr::null(), &mut di).ofx_ok()?;

    let mut dp: *mut c_void = ptr::null_mut();
    pgp(di, kOfxImagePropData.as_ptr(), 0, &mut dp).ofx_ok()?;
    let mut drb: c_int = 0;
    pgi(di, kOfxImagePropRowBytes.as_ptr(), 0, &mut drb).ofx_ok()?;
    let d_stride = drb.max(0) as usize;

    match depth {
        4 => {
            for y in 0..height {
                ptr::copy_nonoverlapping(
                    dst_buf.as_ptr().add(y * row_bytes_u8),
                    (dp as *mut u8).add(y * d_stride),
                    row_bytes_u8,
                );
            }
        }
        8 => {
            for y in 0..height {
                let u8_row = dst_buf.as_ptr().add(y * row_bytes_u8);
                let host_row = (dp as *mut u8).add(y * d_stride) as *mut u16;
                for x in 0..(width * 4) {
                    let v = *u8_row.add(x) as u16;
                    *host_row.add(x) = (v << 8) | v;
                }
            }
        }
        _ => {
            for y in 0..height {
                let u8_row = dst_buf.as_ptr().add(y * row_bytes_u8);
                let host_row = (dp as *mut u8).add(y * d_stride) as *mut f32;
                for x in 0..(width * 4) {
                    *host_row.add(x) = *u8_row.add(x) as f32 / 255.0;
                }
            }
        }
    }

    cri(di).ofx_ok()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Parameter creation helpers
// ---------------------------------------------------------------------------

unsafe fn define_single_param(
    data: &'static SharedData,
    param_set: OfxParamSetHandle,
    descriptor: &SettingDescriptor<ZzzRepeaterFullSettings>,
    default_settings: &ZzzRepeaterFullSettings,
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
    dst: &mut ZzzRepeaterFullSettings,
) -> OfxResult<()> {
    let pgh = data.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = data.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    // --- Native Double2D: Position ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, POSITION_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut x: f64 = 0.0;
        let mut y: f64 = 0.0;
        pgv(p, time, &mut x, &mut y).ofx_ok()?;

        let find_id = |name: &str| -> SettingID<ZzzRepeaterFullSettings> {
            data.settings_list.setting_descriptors.iter()
                .find(|d| d.id.name == name)
                .unwrap()
                .id.clone()
        };

        dst.set_field::<f32>(&find_id("position_x"), x.clamp(0.0, 1.0) as f32).unwrap();
        dst.set_field::<f32>(&find_id("position_y"), y.clamp(0.0, 1.0) as f32).unwrap();
    }

    // --- Read remaining generic params (skip native grouped) ---
    for desc in data.settings_list.setting_descriptors.iter() {
        if is_native_grouped_name(desc.id.name) {
            continue;
        }
        if let SettingKind::Group { .. } = &desc.kind {
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
    desc: &SettingDescriptor<ZzzRepeaterFullSettings>,
    dst: &mut ZzzRepeaterFullSettings,
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
        SettingKind::Percentage { .. } => {
            let mut v: f64 = 0.0;
            pgv(p, time, &mut v).ofx_ok()?;
            dst.set_field::<f32>(&desc.id, v.clamp(0.0, 1.0) as f32).unwrap();
        }
        SettingKind::FloatRange { range, .. } => {
            let mut v: f64 = 0.0;
            pgv(p, time, &mut v).ofx_ok()?;
            let lo = *range.start() as f64;
            let hi = *range.end() as f64;
            dst.set_field::<f32>(&desc.id, v.clamp(lo, hi) as f32).unwrap();
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
