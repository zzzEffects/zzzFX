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
    settings::{
        EnumValue, SettingDescriptor, SettingID, SettingKind, Settings, SettingsList,
    },
    SolidColorBlend, SolidColorBlendFullSettings,
};

use bindings::*;

// SAFETY: The host promises not to mess with the raw string pointers in this struct
unsafe impl Send for OfxPlugin {}
unsafe impl Sync for OfxPlugin {}

// ---------------------------------------------------------------------------
// Globals
// ---------------------------------------------------------------------------

static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();
static SHARED_DATA: OnceLock<SharedData> = OnceLock::new();

// ---------------------------------------------------------------------------
// OFX parameter name constants
// ---------------------------------------------------------------------------

const RGBA_PARAM_NAME: &CStr = c"color";
const BLEND_MODE_PARAM_NAME: &CStr = c"blend_mode";

// ---------------------------------------------------------------------------
// HostInfo — stored during set_host
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct HostInfo {
    host: &'static OfxPropertySetStruct,
    fetch_suite: unsafe extern "C" fn(
        host: OfxPropertySetHandle,
        suiteName: *const c_char,
        suiteVersion: c_int,
    ) -> *const c_void,
}

// ---------------------------------------------------------------------------
// SharedData — initialized once on set_host
// ---------------------------------------------------------------------------

struct SharedData {
    #[allow(dead_code)]
    host_info: HostInfo,
    property_suite: &'static OfxPropertySuiteV1,
    image_effect_suite: &'static OfxImageEffectSuiteV1,
    memory_suite: &'static OfxMemorySuiteV1,
    parameter_suite: &'static OfxParameterSuiteV1,
    settings_list: SettingsList<SolidColorBlendFullSettings>,
    supports_multiple_clip_depths: AtomicBool,
    strings: HashMap<
        SettingID<SolidColorBlendFullSettings>,
        (CString, CString, Option<CString>, Option<CString>),
    >,
    menu_item_strings:
        HashMap<(SettingID<SolidColorBlendFullSettings>, u32), (CString, Option<CString>)>,
}

type OfxResult<T> = Result<T, OfxStatus>;

impl SharedData {
    pub unsafe fn new(host_info: HostInfo) -> OfxResult<Self> {
        let property_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _,
            kOfxPropertySuite.as_ptr(),
            1,
        ) as *const OfxPropertySuiteV1;
        let image_effect_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _,
            kOfxImageEffectSuite.as_ptr(),
            1,
        ) as *const OfxImageEffectSuiteV1;
        let memory_suite =
            (host_info.fetch_suite)(host_info.host as *const _ as _, kOfxMemorySuite.as_ptr(), 1)
                as *const OfxMemorySuiteV1;
        let parameter_suite = (host_info.fetch_suite)(
            host_info.host as *const _ as _,
            kOfxParameterSuite.as_ptr(),
            1,
        ) as *const OfxParameterSuiteV1;

        let settings_list = SettingsList::<SolidColorBlendFullSettings>::new();
        let mut strings = HashMap::new();
        let mut menu_item_strings = HashMap::new();
        for descriptor in settings_list.all_descriptors() {
            let id = &descriptor.id;
            let id_str = CString::new(descriptor.id.id.to_string()).unwrap();
            let label = CString::new(descriptor.label).unwrap();
            let description = descriptor
                .description
                .map(|desc| CString::new(desc).unwrap());
            let group_name = if let SettingKind::Group { .. } = descriptor.kind {
                Some(CString::new(format!("{}_group", descriptor.id.id)).unwrap())
            } else {
                None
            };
            strings.insert(id.clone(), (id_str, label, description, group_name));

            if let SettingKind::Enumeration { options } = &descriptor.kind {
                for menu_item in options {
                    let item_label = CString::new(menu_item.label).unwrap();
                    menu_item_strings.insert(
                        (id.clone(), menu_item.index),
                        (
                            item_label,
                            menu_item
                                .description
                                .map(|desc| CString::new(desc).unwrap()),
                        ),
                    );
                }
            }
        }

        Ok(SharedData {
            host_info,
            property_suite: property_suite
                .as_ref()
                .ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            image_effect_suite: image_effect_suite
                .as_ref()
                .ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            memory_suite: memory_suite
                .as_ref()
                .ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            parameter_suite: parameter_suite
                .as_ref()
                .ok_or(OfxStat::kOfxStatErrMissingHostFeature)?,
            settings_list,
            supports_multiple_clip_depths: AtomicBool::new(false),
            strings,
            menu_item_strings,
        })
    }
}

fn shared() -> &'static SharedData {
    SHARED_DATA.get().expect("SharedData not initialized; set_host must be called first")
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn OfxGetNumberOfPlugins() -> c_int {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn OfxGetPlugin(nth: c_int) -> *const OfxPlugin {
    if nth != 0 {
        return ptr::null();
    }

    // Prevent panics from being silently swallowed by the host
    std::panic::set_hook(Box::new(|info| {
        println!("{info:?}");
    }));

    let plugin_info = PLUGIN_INFO.get_or_init(|| OfxPlugin {
        pluginApi: kOfxImageEffectPluginApi.as_ptr(),
        apiVersion: 1,
        pluginIdentifier: c"com.example:ExampleEffect".as_ptr(),
        pluginVersionMajor: 0,
        pluginVersionMinor: 1,
        setHost: Some(set_host_info),
        mainEntry: Some(main_entry),
    });
    plugin_info as *const _
}

// ---------------------------------------------------------------------------
// set_host_info
// ---------------------------------------------------------------------------

unsafe fn set_host_info_inner(host: *mut OfxHost) -> OfxResult<()> {
    if let Some(host_struct) = host.as_ref() {
        let host = host_struct.host.as_ref().ok_or(OfxStat::kOfxStatFailed)?;
        let fetch_suite = host_struct.fetchSuite.ok_or(OfxStat::kOfxStatFailed)?;
        let new_shared_data = SharedData::new(HostInfo { host, fetch_suite })?;
        SHARED_DATA.get_or_init(|| new_shared_data);
        Ok(())
    } else {
        Err(OfxStat::kOfxStatFailed)
    }
}

unsafe extern "C" fn set_host_info(host: *mut OfxHost) {
    set_host_info_inner(host).unwrap();
}

// ---------------------------------------------------------------------------
// main_entry — action dispatcher
// ---------------------------------------------------------------------------

unsafe extern "C" fn main_entry(
    action: *const c_char,
    handle: *const c_void,
    inArgs: OfxPropertySetHandle,
    outArgs: OfxPropertySetHandle,
) -> OfxStatus {
    let effect = handle as OfxImageEffectHandle;
    let action = CStr::from_ptr(action);

    let return_status: OfxResult<()> = if action == kOfxActionLoad {
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
        // DaVinci Resolve requires these even as no-ops
        Ok(())
    } else if action == kOfxActionInstanceChanged {
        action_instance_changed(effect, inArgs)
    } else if action == kOfxImageEffectActionRender {
        action_render(effect, inArgs)
    } else {
        OfxResult::Err(OfxStat::kOfxStatReplyDefault)
    };

    match return_status {
        Ok(()) => OfxStat::kOfxStatOK,
        Err(e) => e,
    }
}

// ---------------------------------------------------------------------------
// Action handlers
// ---------------------------------------------------------------------------

unsafe fn action_load() -> OfxResult<()> {
    let data = shared();
    let propGetInt = data
        .property_suite
        .propGetInt
        .ok_or(OfxStat::kOfxStatFailed)?;
    let mut supports_multiple_clip_depths: c_int = 0;
    propGetInt(
        data.host_info.host as *const _ as _,
        kOfxImageEffectPropSupportsMultipleClipDepths.as_ptr(),
        0,
        &mut supports_multiple_clip_depths,
    )
    .ofx_ok()?;
    data.supports_multiple_clip_depths
        .store(supports_multiple_clip_depths != 0, Ordering::Release);
    Ok(())
}

unsafe fn action_describe(descriptor: OfxImageEffectHandle) -> OfxResult<()> {
    let data = shared();
    let mut effectProps: OfxPropertySetHandle = ptr::null_mut();
    (data
        .image_effect_suite
        .getPropertySet
        .ok_or(OfxStat::kOfxStatFailed)?)(descriptor, &mut effectProps)
    .ofx_ok()?;

    let propSetString = data
        .property_suite
        .propSetString
        .ok_or(OfxStat::kOfxStatFailed)?;
    let propSetInt = data
        .property_suite
        .propSetInt
        .ok_or(OfxStat::kOfxStatFailed)?;

    propSetString(effectProps, kOfxPropLabel.as_ptr(), 0, c"Example Effect".as_ptr()).ofx_ok()?;

    propSetString(
        effectProps,
        kOfxImageEffectPluginPropGrouping.as_ptr(),
        0,
        c"Example".as_ptr(),
    )
    .ofx_ok()?;

    propSetString(
        effectProps,
        kOfxImageEffectPropSupportedContexts.as_ptr(),
        0,
        kOfxImageEffectContextFilter.as_ptr(),
    )
    .ofx_ok()?;
    propSetString(
        effectProps,
        kOfxImageEffectPropSupportedContexts.as_ptr(),
        1,
        kOfxImageEffectContextGeneral.as_ptr(),
    )
    .ofx_ok()?;

    propSetString(
        effectProps,
        kOfxImageEffectPropSupportedPixelDepths.as_ptr(),
        0,
        kOfxBitDepthFloat.as_ptr(),
    )
    .ofx_ok()?;
    propSetString(
        effectProps,
        kOfxImageEffectPropSupportedPixelDepths.as_ptr(),
        1,
        kOfxBitDepthShort.as_ptr(),
    )
    .ofx_ok()?;
    propSetString(
        effectProps,
        kOfxImageEffectPropSupportedPixelDepths.as_ptr(),
        2,
        kOfxBitDepthByte.as_ptr(),
    )
    .ofx_ok()?;

    propSetString(
        effectProps,
        kOfxImageEffectPluginRenderThreadSafety.as_ptr(),
        0,
        kOfxImageEffectRenderFullySafe.as_ptr(),
    )
    .ofx_ok()?;
    propSetInt(
        effectProps,
        kOfxImageEffectPluginPropHostFrameThreading.as_ptr(),
        0,
        0,
    )
    .ofx_ok()?;
    propSetInt(effectProps, kOfxImageEffectPropSupportsTiles.as_ptr(), 0, 0).ofx_ok()?;

    Ok(())
}

unsafe fn action_describe_in_context(descriptor: OfxImageEffectHandle) -> OfxResult<()> {
    let data = shared();
    let clipDefine = data
        .image_effect_suite
        .clipDefine
        .ok_or(OfxStat::kOfxStatFailed)?;
    let getParamSet = data
        .image_effect_suite
        .getParamSet
        .ok_or(OfxStat::kOfxStatFailed)?;
    let property_suite = data.property_suite;
    let param_suite = data.parameter_suite;

    let propSetString = property_suite
        .propSetString
        .ok_or(OfxStat::kOfxStatFailed)?;

    // Output clip
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    clipDefine(descriptor, c"Output".as_ptr(), &mut props).ofx_ok()?;
    if props.is_null() {
        return Err(OfxStat::kOfxStatFailed);
    }
    propSetString(
        props,
        kOfxImageEffectPropSupportedComponents.as_ptr(),
        0,
        kOfxImageComponentRGBA.as_ptr(),
    )
    .ofx_ok()?;
    propSetString(
        props,
        kOfxImageEffectPropSupportedComponents.as_ptr(),
        1,
        kOfxImageComponentRGB.as_ptr(),
    )
    .ofx_ok()?;

    // Source clip
    clipDefine(descriptor, c"Source".as_ptr(), &mut props).ofx_ok()?;
    if props.is_null() {
        return Err(OfxStat::kOfxStatFailed);
    }
    propSetString(
        props,
        kOfxImageEffectPropSupportedComponents.as_ptr(),
        0,
        kOfxImageComponentRGBA.as_ptr(),
    )
    .ofx_ok()?;
    propSetString(
        props,
        kOfxImageEffectPropSupportedComponents.as_ptr(),
        1,
        kOfxImageComponentRGB.as_ptr(),
    )
    .ofx_ok()?;

    // Parameter set
    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    getParamSet(descriptor, &mut param_set).ofx_ok()?;

    let paramDefine = param_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let propSetDouble = property_suite.propSetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let propSetInt = property_suite.propSetInt.ok_or(OfxStat::kOfxStatFailed)?;

    // --- RGBA color parameter ---
    // R, G, B = solid color; A = blend amount
    let mut rgbaProps: OfxPropertySetHandle = ptr::null_mut();
    paramDefine(
        param_set,
        kOfxParamTypeRGBA.as_ptr(),
        RGBA_PARAM_NAME.as_ptr(),
        &mut rgbaProps,
    )
    .ofx_ok()?;
    propSetString(rgbaProps, kOfxPropLabel.as_ptr(), 0, c"Color".as_ptr()).ofx_ok()?;
    // Default: black with no blend (passthrough)
    propSetDouble(rgbaProps, kOfxParamPropDefault.as_ptr(), 0, 0.0).ofx_ok()?; // R
    propSetDouble(rgbaProps, kOfxParamPropDefault.as_ptr(), 1, 0.0).ofx_ok()?; // G
    propSetDouble(rgbaProps, kOfxParamPropDefault.as_ptr(), 2, 0.0).ofx_ok()?; // B
    propSetDouble(rgbaProps, kOfxParamPropDefault.as_ptr(), 3, 0.0).ofx_ok()?; // A

    // --- Blend mode choice parameter ---
    let defaults = SolidColorBlendFullSettings::default();
    let default_mode = defaults.get_field::<EnumValue>(&data.settings_list.setting_descriptors[4].id)
        .map_err(|_| OfxStat::kOfxStatFailed)?.0;
    let mut modeProps: OfxPropertySetHandle = ptr::null_mut();
    paramDefine(
        param_set,
        kOfxParamTypeChoice.as_ptr(),
        BLEND_MODE_PARAM_NAME.as_ptr(),
        &mut modeProps,
    )
    .ofx_ok()?;
    propSetString(modeProps, kOfxPropLabel.as_ptr(), 0, c"Blend Mode".as_ptr()).ofx_ok()?;
    propSetString(modeProps, kOfxParamPropChoiceOption.as_ptr(), 0, c"Normal".as_ptr()).ofx_ok()?;
    propSetString(modeProps, kOfxParamPropChoiceOption.as_ptr(), 1, c"Multiply".as_ptr()).ofx_ok()?;
    propSetString(modeProps, kOfxParamPropChoiceOption.as_ptr(), 2, c"Screen".as_ptr()).ofx_ok()?;
    propSetString(modeProps, kOfxParamPropChoiceOption.as_ptr(), 3, c"Overlay".as_ptr()).ofx_ok()?;
    propSetInt(modeProps, kOfxParamPropDefault.as_ptr(), 0, default_mode as c_int).ofx_ok()?;
    propSetString(modeProps, kOfxParamPropHint.as_ptr(), 0, c"How the solid color is blended with the image.".as_ptr()).ofx_ok()?;

    Ok(())
}

unsafe fn action_get_regions_of_interest(
    effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
    outArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let data = shared();
    let propGetDouble = data
        .property_suite
        .propGetDouble
        .ok_or(OfxStat::kOfxStatFailed)?;
    let propSetDoubleN = data
        .property_suite
        .propSetDoubleN
        .ok_or(OfxStat::kOfxStatFailed)?;
    let clipGetHandle = data
        .image_effect_suite
        .clipGetHandle
        .ok_or(OfxStat::kOfxStatFailed)?;
    let clipGetRegionOfDefinition = data
        .image_effect_suite
        .clipGetRegionOfDefinition
        .ok_or(OfxStat::kOfxStatFailed)?;

    let mut sourceClip: OfxImageClipHandle = ptr::null_mut();
    clipGetHandle(
        effect,
        c"Source".as_ptr(),
        &mut sourceClip,
        ptr::null_mut(),
    )
    .ofx_ok()?;
    let mut sourceRoD = OfxRectD {
        x1: 0.0,
        x2: 0.0,
        y1: 0.0,
        y2: 0.0,
    };
    let mut time: OfxTime = 0.0;
    propGetDouble(inArgs, kOfxPropTime.as_ptr(), 0, &mut time).ofx_ok()?;
    clipGetRegionOfDefinition(sourceClip, time, &mut sourceRoD).ofx_ok()?;

    propSetDoubleN(
        outArgs,
        c"OfxImageClipPropRoI_Source".as_ptr(),
        4,
        ptr::addr_of_mut!(sourceRoD) as *mut _,
    )
    .ofx_ok()?;

    Ok(())
}

unsafe fn action_get_clip_preferences(outArgs: OfxPropertySetHandle) -> OfxResult<()> {
    let data = shared();
    let propSetInt = data
        .property_suite
        .propSetInt
        .ok_or(OfxStat::kOfxStatFailed)?;
    let propSetString = data
        .property_suite
        .propSetString
        .ok_or(OfxStat::kOfxStatFailed)?;

    propSetInt(outArgs, kOfxImageEffectFrameVarying.as_ptr(), 0, 1).ofx_ok()?;
    propSetString(
        outArgs,
        kOfxImageEffectPropPreMultiplication.as_ptr(),
        0,
        kOfxImageOpaque.as_ptr(),
    )
    .ofx_ok()?;

    Ok(())
}

unsafe fn action_instance_changed(
    _effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let data = shared();
    let propGetInt = data
        .property_suite
        .propGetInt
        .ok_or(OfxStat::kOfxStatFailed)?;

    let mut reason: c_int = 0;
    propGetInt(inArgs, kOfxPropChangeReason.as_ptr(), 0, &mut reason).ofx_ok()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Render — solid color blend
// ---------------------------------------------------------------------------

unsafe fn action_render(
    effect: OfxImageEffectHandle,
    inArgs: OfxPropertySetHandle,
) -> OfxResult<()> {
    let data = shared();

    let clipGetHandle = data
        .image_effect_suite
        .clipGetHandle
        .ok_or(OfxStat::kOfxStatFailed)?;
    let clipGetImage = data
        .image_effect_suite
        .clipGetImage
        .ok_or(OfxStat::kOfxStatFailed)?;
    let clipReleaseImage = data
        .image_effect_suite
        .clipReleaseImage
        .ok_or(OfxStat::kOfxStatFailed)?;
    let propGetPointer = data
        .property_suite
        .propGetPointer
        .ok_or(OfxStat::kOfxStatFailed)?;
    let propGetInt = data
        .property_suite
        .propGetInt
        .ok_or(OfxStat::kOfxStatFailed)?;
    let propGetDouble = data
        .property_suite
        .propGetDouble
        .ok_or(OfxStat::kOfxStatFailed)?;
    let propGetString = data
        .property_suite
        .propGetString
        .ok_or(OfxStat::kOfxStatFailed)?;

    let getParamSet = data
        .image_effect_suite
        .getParamSet
        .ok_or(OfxStat::kOfxStatFailed)?;

    // Read time
    let mut time: OfxTime = 0.0;
    propGetDouble(inArgs, kOfxPropTime.as_ptr(), 0, &mut time).ofx_ok()?;

    // Read effect parameters
    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    getParamSet(effect, &mut param_set).ofx_ok()?;

    let mut settings = SolidColorBlendFullSettings::default();
    apply_params(data, param_set, time, &data.settings_list.setting_descriptors, &mut settings)?;
    let blend: SolidColorBlend = (&settings).into();

    // Get clip handles
    let mut srcClip: OfxImageClipHandle = ptr::null_mut();
    clipGetHandle(effect, c"Source".as_ptr(), &mut srcClip, ptr::null_mut()).ofx_ok()?;
    let mut dstClip: OfxImageClipHandle = ptr::null_mut();
    clipGetHandle(effect, c"Output".as_ptr(), &mut dstClip, ptr::null_mut()).ofx_ok()?;

    // Get images
    let mut srcImg: OfxPropertySetHandle = ptr::null_mut();
    clipGetImage(srcClip, time, ptr::null(), &mut srcImg).ofx_ok()?;
    let mut dstImg: OfxPropertySetHandle = ptr::null_mut();
    clipGetImage(dstClip, time, ptr::null(), &mut dstImg).ofx_ok()?;

    // Get image data pointers
    let mut srcPtr: *mut c_void = ptr::null_mut();
    propGetPointer(srcImg, kOfxImagePropData.as_ptr(), 0, &mut srcPtr).ofx_ok()?;
    let mut dstPtr: *mut c_void = ptr::null_mut();
    propGetPointer(dstImg, kOfxImagePropData.as_ptr(), 0, &mut dstPtr).ofx_ok()?;

    // Get row bytes and bounds
    let mut srcRowBytes: c_int = 0; let mut dstRowBytes: c_int = 0;
    propGetInt(srcImg, kOfxImagePropRowBytes.as_ptr(), 0, &mut srcRowBytes).ofx_ok()?;
    propGetInt(dstImg, kOfxImagePropRowBytes.as_ptr(), 0, &mut dstRowBytes).ofx_ok()?;

    let mut left: c_int = 0; let mut bottom: c_int = 0;
    let mut right: c_int = 0; let mut top: c_int = 0;
    propGetInt(srcImg, kOfxImagePropBounds.as_ptr(), 0, &mut left).ofx_ok()?;
    propGetInt(srcImg, kOfxImagePropBounds.as_ptr(), 1, &mut bottom).ofx_ok()?;
    propGetInt(srcImg, kOfxImagePropBounds.as_ptr(), 2, &mut right).ofx_ok()?;
    propGetInt(srcImg, kOfxImagePropBounds.as_ptr(), 3, &mut top).ofx_ok()?;

    let width = (right - left) as usize;
    let height = (top - bottom) as usize;
    let src_stride = srcRowBytes.max(0) as usize;
    let dst_stride = dstRowBytes.max(0) as usize;

    // Determine pixel depth from the source image
    let mut depth_ptr: *mut c_char = ptr::null_mut();
    let depth = (|| {
        propGetString(
            srcImg,
            kOfxImageEffectPropPixelDepth.as_ptr(),
            0,
            &mut depth_ptr,
        )
        .ofx_ok()
        .ok()?;
        let s = CStr::from_ptr(depth_ptr);
        if s == kOfxBitDepthFloat {
            Some(16usize) // 4 bytes/component × 4 components
        } else if s == kOfxBitDepthShort {
            Some(8usize) // 2 bytes/component × 4 components
        } else if s == kOfxBitDepthByte {
            Some(4usize) // 1 byte/component × 4 components
        } else {
            None
        }
    })()
    .unwrap_or(4); // default to Byte on unrecognized depth

    let row_bytes_u8 = width * 4;
    let total_u8 = row_bytes_u8 * height;
    let mut src_buf = vec![0u8; total_u8];
    let mut dst_buf = vec![0u8; total_u8];

    match depth {
        4 => {
            // Byte: direct copy — fast path
            for y in 0..height {
                ptr::copy_nonoverlapping(
                    (srcPtr as *const u8).add(y * src_stride),
                    src_buf.as_mut_ptr().add(y * row_bytes_u8),
                    row_bytes_u8,
                );
            }
        }
        8 => {
            // Short (u16): convert to u8 via (v * 255 + 32767) / 65535
            for y in 0..height {
                let host_row = (srcPtr as *const u8).add(y * src_stride) as *const u16;
                let u8_row = src_buf.as_mut_ptr().add(y * row_bytes_u8);
                for x in 0..(width * 4) {
                    let v = *host_row.add(x) as u32;
                    *u8_row.add(x) = ((v * 255 + 32767) / 65535) as u8;
                }
            }
        }
        _ => {
            // Float (f32): convert to u8 via clamp(0,1) * 255 + round
            for y in 0..height {
                let host_row = (srcPtr as *const u8).add(y * src_stride) as *const f32;
                let u8_row = src_buf.as_mut_ptr().add(y * row_bytes_u8);
                for x in 0..(width * 4) {
                    let v = *host_row.add(x);
                    *u8_row.add(x) = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                }
            }
        }
    }

    blend.apply_effect(&src_buf, &mut dst_buf, width, height);

    match depth {
        4 => {
            // Byte: direct copy back — fast path
            for y in 0..height {
                ptr::copy_nonoverlapping(
                    dst_buf.as_ptr().add(y * row_bytes_u8),
                    (dstPtr as *mut u8).add(y * dst_stride),
                    row_bytes_u8,
                );
            }
        }
        8 => {
            // u8 to Short (u16): v * 257 (i.e. (v << 8) | v)
            for y in 0..height {
                let u8_row = dst_buf.as_ptr().add(y * row_bytes_u8);
                let host_row = (dstPtr as *mut u8).add(y * dst_stride) as *mut u16;
                for x in 0..(width * 4) {
                    let v = *u8_row.add(x) as u16;
                    *host_row.add(x) = (v << 8) | v;
                }
            }
        }
        _ => {
            // u8 to Float (f32): v / 255.0
            for y in 0..height {
                let u8_row = dst_buf.as_ptr().add(y * row_bytes_u8);
                let host_row = (dstPtr as *mut u8).add(y * dst_stride) as *mut f32;
                for x in 0..(width * 4) {
                    *host_row.add(x) = *u8_row.add(x) as f32 / 255.0;
                }
            }
        }
    }

    clipReleaseImage(srcImg).ofx_ok()?;
    clipReleaseImage(dstImg).ofx_ok()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Parameter mapping helpers
// ---------------------------------------------------------------------------

unsafe fn map_params(
    data: &'static SharedData,
    param_set: OfxParamSetHandle,
    setting_descriptors: &[SettingDescriptor<SolidColorBlendFullSettings>],
    default_settings: &SolidColorBlendFullSettings,
    parent: &CStr,
) -> OfxResult<()> {
    let paramDefine = data
        .parameter_suite
        .paramDefine
        .ok_or(OfxStat::kOfxStatFailed)?;
    let propSetDouble = data
        .property_suite
        .propSetDouble
        .ok_or(OfxStat::kOfxStatFailed)?;
    let propSetInt = data
        .property_suite
        .propSetInt
        .ok_or(OfxStat::kOfxStatFailed)?;
    let propSetString = data
        .property_suite
        .propSetString
        .ok_or(OfxStat::kOfxStatFailed)?;

    for descriptor in setting_descriptors {
        let mut paramProps: OfxPropertySetHandle = ptr::null_mut();
        let descriptor_strings: &'static _ = data.strings.get(&descriptor.id).unwrap();
        let descriptor_id_cstr = descriptor_strings.0.as_c_str();

        match &descriptor.kind {
            SettingKind::Enumeration { options } => {
                paramDefine(
                    param_set,
                    kOfxParamTypeChoice.as_ptr(),
                    descriptor_id_cstr.as_ptr(),
                    &mut paramProps,
                )
                .ofx_ok()?;
                let default_value = default_settings
                    .get_field::<EnumValue>(&descriptor.id)
                    .map_err(|_| OfxStat::kOfxStatFailed)?
                    .0;
                let mut default_idx: usize = 0;
                for (i, menu_item) in options.iter().enumerate() {
                    let item_strings = data
                        .menu_item_strings
                        .get(&(descriptor.id.clone(), menu_item.index))
                        .unwrap();
                    let item_label_cstr: &'static CStr = item_strings.0.as_c_str();
                    propSetString(
                        paramProps,
                        kOfxParamPropChoiceOption.as_ptr(),
                        i as i32,
                        item_label_cstr.as_ptr(),
                    )
                    .ofx_ok()?;
                    if menu_item.index == default_value {
                        default_idx = i;
                    }
                }
                propSetInt(
                    paramProps,
                    kOfxParamPropDefault.as_ptr(),
                    0,
                    default_idx as i32,
                )
                .ofx_ok()?;
            }
            SettingKind::Percentage { .. } => {
                let default_value = default_settings
                    .get_field::<f32>(&descriptor.id)
                    .map_err(|_| OfxStat::kOfxStatFailed)?;
                paramDefine(
                    param_set,
                    kOfxParamTypeDouble.as_ptr(),
                    descriptor_id_cstr.as_ptr(),
                    &mut paramProps,
                )
                .ofx_ok()?;
                propSetString(
                    paramProps,
                    kOfxParamPropDoubleType.as_ptr(),
                    0,
                    kOfxParamDoubleTypeScale.as_ptr(),
                )
                .ofx_ok()?;
                propSetDouble(paramProps, kOfxParamPropDefault.as_ptr(), 0, default_value as f64)
                    .ofx_ok()?;
                propSetDouble(paramProps, kOfxParamPropMin.as_ptr(), 0, 0.0).ofx_ok()?;
                propSetDouble(paramProps, kOfxParamPropDisplayMin.as_ptr(), 0, 0.0).ofx_ok()?;
                propSetDouble(paramProps, kOfxParamPropMax.as_ptr(), 0, 1.0).ofx_ok()?;
                propSetDouble(paramProps, kOfxParamPropDisplayMax.as_ptr(), 0, 1.0).ofx_ok()?;
            }
            SettingKind::IntRange { range } => {
                let default_value = default_settings
                    .get_field::<i32>(&descriptor.id)
                    .map_err(|_| OfxStat::kOfxStatFailed)?;
                paramDefine(
                    param_set,
                    kOfxParamTypeInteger.as_ptr(),
                    descriptor_id_cstr.as_ptr(),
                    &mut paramProps,
                )
                .ofx_ok()?;
                propSetInt(paramProps, kOfxParamPropDefault.as_ptr(), 0, default_value).ofx_ok()?;
                propSetInt(paramProps, kOfxParamPropMin.as_ptr(), 0, *range.start()).ofx_ok()?;
                propSetInt(paramProps, kOfxParamPropDisplayMin.as_ptr(), 0, *range.start())
                    .ofx_ok()?;
                propSetInt(paramProps, kOfxParamPropMax.as_ptr(), 0, *range.end()).ofx_ok()?;
                propSetInt(paramProps, kOfxParamPropDisplayMax.as_ptr(), 0, *range.end())
                    .ofx_ok()?;
            }
            SettingKind::FloatRange { range, .. } => {
                let default_value = default_settings
                    .get_field::<f32>(&descriptor.id)
                    .map_err(|_| OfxStat::kOfxStatFailed)?;
                paramDefine(
                    param_set,
                    kOfxParamTypeDouble.as_ptr(),
                    descriptor_id_cstr.as_ptr(),
                    &mut paramProps,
                )
                .ofx_ok()?;
                propSetDouble(paramProps, kOfxParamPropDefault.as_ptr(), 0, default_value as f64)
                    .ofx_ok()?;
                propSetDouble(
                    paramProps,
                    kOfxParamPropMin.as_ptr(),
                    0,
                    *range.start() as f64,
                )
                .ofx_ok()?;
                propSetDouble(
                    paramProps,
                    kOfxParamPropDisplayMin.as_ptr(),
                    0,
                    *range.start() as f64,
                )
                .ofx_ok()?;
                propSetDouble(
                    paramProps,
                    kOfxParamPropMax.as_ptr(),
                    0,
                    *range.end() as f64,
                )
                .ofx_ok()?;
                propSetDouble(
                    paramProps,
                    kOfxParamPropDisplayMax.as_ptr(),
                    0,
                    *range.end() as f64,
                )
                .ofx_ok()?;
            }
            SettingKind::Boolean => {
                let default_value = default_settings
                    .get_field::<bool>(&descriptor.id)
                    .map_err(|_| OfxStat::kOfxStatFailed)?;
                paramDefine(
                    param_set,
                    kOfxParamTypeBoolean.as_ptr(),
                    descriptor_id_cstr.as_ptr(),
                    &mut paramProps,
                )
                .ofx_ok()?;
                propSetInt(paramProps, kOfxParamPropDefault.as_ptr(), 0, default_value as i32)
                    .ofx_ok()?;
            }
            SettingKind::Group { children } => {
                let default_value = default_settings
                    .get_field::<bool>(&descriptor.id)
                    .map_err(|_| OfxStat::kOfxStatFailed)?;
                let group_name_cstr: &'static CStr = descriptor_strings
                    .3
                    .as_ref()
                    .expect("Group name is None")
                    .as_c_str();
                paramDefine(
                    param_set,
                    kOfxParamTypeGroup.as_ptr(),
                    group_name_cstr.as_ptr(),
                    &mut paramProps,
                )
                .ofx_ok()?;

                let mut checkboxProps: OfxPropertySetHandle = ptr::null_mut();
                paramDefine(
                    param_set,
                    kOfxParamTypeBoolean.as_ptr(),
                    descriptor_id_cstr.as_ptr(),
                    &mut checkboxProps,
                )
                .ofx_ok()?;
                propSetString(
                    checkboxProps,
                    kOfxPropLabel.as_ptr(),
                    0,
                    c"Enabled".as_ptr(),
                )
                .ofx_ok()?;
                propSetInt(
                    checkboxProps,
                    kOfxParamPropDefault.as_ptr(),
                    0,
                    default_value as i32,
                )
                .ofx_ok()?;
                propSetString(
                    checkboxProps,
                    kOfxParamPropParent.as_ptr(),
                    0,
                    group_name_cstr.as_ptr(),
                )
                .ofx_ok()?;
                propSetInt(checkboxProps, kOfxParamPropAnimates.as_ptr(), 0, 0).ofx_ok()?;

                map_params(data, param_set, children, default_settings, group_name_cstr)?;
            }
        }

        if !paramProps.is_null() {
            let descriptor_strings = data.strings.get(&descriptor.id).unwrap();
            let descriptor_label_cstr: &'static CStr = descriptor_strings.1.as_c_str();
            propSetString(
                paramProps,
                kOfxPropLabel.as_ptr(),
                0,
                descriptor_label_cstr.as_ptr(),
            )
            .ofx_ok()?;
            if let Some(description) = descriptor_strings.2.as_deref() {
                propSetString(
                    paramProps,
                    kOfxParamPropHint.as_ptr(),
                    0,
                    description.as_ptr(),
                )
                .ofx_ok()?;
            }
            propSetString(paramProps, kOfxParamPropParent.as_ptr(), 0, parent.as_ptr()).ofx_ok()?;
        }
    }

    Ok(())
}

unsafe fn apply_params(
    data: &'static SharedData,
    param_set: OfxParamSetHandle,
    time: f64,
    _setting_descriptors: &[SettingDescriptor<SolidColorBlendFullSettings>],
    dst: &mut SolidColorBlendFullSettings,
) -> OfxResult<()> {
    let paramGetHandle = data
        .parameter_suite
        .paramGetHandle
        .ok_or(OfxStat::kOfxStatFailed)?;
    let paramGetValueAtTime = data
        .parameter_suite
        .paramGetValueAtTime
        .ok_or(OfxStat::kOfxStatFailed)?;

    // --- Read RGBA param ---
    let mut rgba_param: OfxParamHandle = ptr::null_mut();
    paramGetHandle(
        param_set,
        RGBA_PARAM_NAME.as_ptr(),
        &mut rgba_param,
        ptr::null_mut(),
    )
    .ofx_ok()?;

    let mut r: f64 = 0.0; let mut g: f64 = 0.0;
    let mut b: f64 = 0.0; let mut a: f64 = 0.0;
    paramGetValueAtTime(rgba_param, time, &mut r, &mut g, &mut b, &mut a).ofx_ok()?;
    dst.set_field::<f32>(&data.settings_list.setting_descriptors[0].id, r as f32).unwrap();
    dst.set_field::<f32>(&data.settings_list.setting_descriptors[1].id, g as f32).unwrap();
    dst.set_field::<f32>(&data.settings_list.setting_descriptors[2].id, b as f32).unwrap();
    dst.set_field::<f32>(&data.settings_list.setting_descriptors[3].id, a as f32).unwrap();

    // --- Read blend_mode choice param ---
    let mut mode_param: OfxParamHandle = ptr::null_mut();
    paramGetHandle(
        param_set,
        BLEND_MODE_PARAM_NAME.as_ptr(),
        &mut mode_param,
        ptr::null_mut(),
    )
    .ofx_ok()?;

    let mut selected_idx: c_int = 0;
    paramGetValueAtTime(mode_param, time, &mut selected_idx).ofx_ok()?;
    let blend_mode_id = &data.settings_list.setting_descriptors[4]; // blend_mode is 5th descriptor
    if let SettingKind::Enumeration { options } = &blend_mode_id.kind {
        dst.set_field::<EnumValue>(
            &blend_mode_id.id,
            EnumValue(options[selected_idx as usize].index),
        ).unwrap();
    }

    Ok(())
}
