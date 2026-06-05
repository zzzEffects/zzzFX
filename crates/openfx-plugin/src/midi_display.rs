use std::{
    ffi::{CStr, c_char, c_int, c_void},
    ptr,
    sync::OnceLock,
};

use zzzfx::{
    MidiDisplay, MidiDisplayFullSettings,
    midi_display::{MidiData, parse_midi_file},
    settings::{Settings, SettingsList, TrKey},
};
use zzzfx::settings::midi_display::setting_id;

use crate::bindings::*;
use crate::file_param;
use crate::i18n;
use crate::shared::{
    HostInfo, SuiteCache, StringCache, MenuItemCache,
    build_string_cache, define_single_param, read_generic_param,
    action_load_common, action_get_clip_preferences_common,
    action_get_region_of_definition_generator,
};

// ---------------------------------------------------------------------------
// Native OFX parameter names
// ---------------------------------------------------------------------------

const FILE_SELECT_PARAM: &CStr = c"file_select";
const FILE_PATH_PARAM: &CStr = c"file_path";
const NOTE_COLOR_PARAM: &CStr = c"note_color";
const NOTE_BORDER_COLOR_PARAM: &CStr = c"note_border_color";
const BACKGROUND_COLOR_PARAM: &CStr = c"background_color";
const PAGE_NAME: &CStr = c"Controls";

fn is_native_grouped_name(name: &str) -> bool {
    matches!(
        name,
        "note_color_r" | "note_color_g" | "note_color_b" | "note_color_a"
        | "note_border_color_r" | "note_border_color_g" | "note_border_color_b" | "note_border_color_a"
        | "background_color_r" | "background_color_g" | "background_color_b" | "background_color_a"
    )
}

// ---------------------------------------------------------------------------
// Instance data
// ---------------------------------------------------------------------------

const INSTANCE_MAGIC: u64 = 0x1D1E_1D1E_1D1E_1D1E;

struct InstanceData {
    magic: u64,
    midi_data: Option<MidiData>,
    file_path: String,
    cached_dst: Vec<u8>,
    cache_valid: bool,
    cached_output_w: usize,
    cached_output_h: usize,
    cached_time: f64,
}

impl InstanceData {
    fn _assert_used(&self) {
        let _ = &self.magic;
        let _ = &self.midi_data;
        let _ = &self.file_path;
        let _ = &self.cached_dst;
        let _ = &self.cache_valid;
        let _ = &self.cached_output_w;
        let _ = &self.cached_output_h;
        let _ = &self.cached_time;
    }
}

// ---------------------------------------------------------------------------
// Per-effect globals
// ---------------------------------------------------------------------------

static PLUGIN_INFO: OnceLock<OfxPlugin> = OnceLock::new();

struct EffectData {
    suites: SuiteCache,
    settings_list: SettingsList<MidiDisplayFullSettings>,
    strings: StringCache<MidiDisplayFullSettings>,
    menu_item_strings: MenuItemCache<MidiDisplayFullSettings>,
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
        pluginIdentifier: c"io.github.zzzEffect:MidiDisplay".as_ptr(),
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
    let settings_list = SettingsList::<MidiDisplayFullSettings>::new();
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
    } else if action == kOfxImageEffectActionGetRegionOfDefinition {
        match data() { Ok(d) => action_get_region_of_definition_generator(&d.suites, effect, inArgs, outArgs), Err(e) => Err(e) }
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

    ps(ep, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::EffectMidiDisplayName).as_ptr()).ofx_ok()?;
    ps(ep, kOfxImageEffectPluginPropGrouping.as_ptr(), 0, c"zzzFX".as_ptr()).ofx_ok()?;
    ps(ep, kOfxPropPluginDescription.as_ptr(), 0, i18n::tr_cstr(TrKey::EffectMidiDisplayDesc).as_ptr()).ofx_ok()?;
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
    let pdef = su.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let defaults = MidiDisplayFullSettings::default();

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
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeControls).as_ptr()).ofx_ok()?;
    }

    // --- Native PushButton: fileSelect ---
    {
        let mut pp: OfxPropertySetHandle = ptr::null_mut();
        pdef(param_set, kOfxParamTypePushButton.as_ptr(), FILE_SELECT_PARAM.as_ptr(), &mut pp).ofx_ok()?;
        ps(pp, kOfxPropLabel.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeSelectMidiFile).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropHint.as_ptr(), 0, i18n::tr_cstr(TrKey::NativeSelectMidiFileHint).as_ptr()).ofx_ok()?;
        ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    }

    // --- Reload File button (hidden by default) ---
    file_param::define_reload_button(su, param_set, PAGE_NAME)?;

    // --- Native RGBA params (interleaved before their component descriptors) ---
    let mut defined_note_color = false;
    let mut defined_note_border_color = false;
    let mut defined_background_color = false;

    for desc in d.settings_list.setting_descriptors.iter() {
        if !defined_note_color && desc.id.name == "note_color_r" {
            define_native_rgba(
                su, param_set, NOTE_COLOR_PARAM,
                i18n::tr_cstr(TrKey::NativeNoteColor),
                i18n::tr_cstr(TrKey::NativeNoteColorHint),
                defaults.note_color_r as f64, defaults.note_color_g as f64,
                defaults.note_color_b as f64, defaults.note_color_a as f64,
            )?;
            defined_note_color = true;
        }
        if !defined_note_border_color && desc.id.name == "note_border_color_r" {
            define_native_rgba(
                su, param_set, NOTE_BORDER_COLOR_PARAM,
                i18n::tr_cstr(TrKey::NativeNoteBorderColor),
                i18n::tr_cstr(TrKey::NativeNoteBorderColorHint),
                defaults.note_border_color_r as f64, defaults.note_border_color_g as f64,
                defaults.note_border_color_b as f64, defaults.note_border_color_a as f64,
            )?;
            defined_note_border_color = true;
        }
        if !defined_background_color && desc.id.name == "background_color_r" {
            define_native_rgba(
                su, param_set, BACKGROUND_COLOR_PARAM,
                i18n::tr_cstr(TrKey::NativeBackgroundColor),
                i18n::tr_cstr(TrKey::NativeBackgroundColorHint),
                defaults.background_color_r as f64, defaults.background_color_g as f64,
                defaults.background_color_b as f64, defaults.background_color_a as f64,
            )?;
            defined_background_color = true;
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

    // --- String param (hidden): fileData (persisted as base64) ---
    file_param::define_file_data_string_param(su, param_set, PAGE_NAME)?;

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

    {
        let gph = su.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
        let mut existing: *mut c_void = ptr::null_mut();
        let _ = gph(ep, kOfxPropInstanceData.as_ptr(), 0, &mut existing);
        if !existing.is_null() {
            let p = &*(existing as *const InstanceData);
            if p.magic == INSTANCE_MAGIC {
                let _ = Box::from_raw(existing as *mut InstanceData);
            }
        }
    }

    let idata = Box::new(InstanceData {
        magic: INSTANCE_MAGIC,
        midi_data: None,
        file_path: String::new(),
        cached_dst: Vec::new(),
        cache_valid: false,
        cached_output_w: 0,
        cached_output_h: 0,
        cached_time: f64::NAN,
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
                .add_filter("MIDI Files", &["mid", "midi", "smf"])
                .pick_file()
            else { return Ok(()); };

            let bytes = std::fs::read(&path).map_err(|_| OfxStat::kOfxStatFailed)?;
            let midi_data = parse_midi_file(&bytes).map_err(|_| OfxStat::kOfxStatFailed)?;
            let path_str = path.to_string_lossy().to_string();

            let mut ep: OfxPropertySetHandle = ptr::null_mut();
            (su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?)(effect, &mut ep).ofx_ok()?;
            let mut data_ptr: *mut c_void = ptr::null_mut();
            propGetPointer(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
            if !data_ptr.is_null() {
                let idata = &mut *(data_ptr as *mut InstanceData);
                idata.file_path = path_str.clone();
                idata.midi_data = Some(midi_data);
                idata.cache_valid = false;
            }

            file_param::write_file_data_base64(su, param_set, &bytes)?;
            file_param::write_string_param(su, param_set, FILE_PATH_PARAM, &path_str)?;
            file_param::reveal_param(su, param_set, file_param::RELOAD_FILE_PARAM)?;

            return Ok(());
        }

        if file_param::RELOAD_FILE_PARAM == CStr::from_ptr(target_name) {
            let path_str = file_param::read_string_param(su, param_set, FILE_PATH_PARAM)?;
            if path_str.is_empty() { return Ok(()); }

            let bytes = std::fs::read(&path_str).map_err(|_| OfxStat::kOfxStatFailed)?;
            let midi_data = parse_midi_file(&bytes).map_err(|_| OfxStat::kOfxStatFailed)?;

            file_param::write_file_data_base64(su, param_set, &bytes)?;

            let mut ep: OfxPropertySetHandle = ptr::null_mut();
            (su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?)(effect, &mut ep).ofx_ok()?;
            let mut data_ptr: *mut c_void = ptr::null_mut();
            propGetPointer(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
            if !data_ptr.is_null() {
                let idata = &mut *(data_ptr as *mut InstanceData);
                idata.file_path = path_str;
                idata.midi_data = Some(midi_data);
                idata.cache_valid = false;
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

    let mut settings = MidiDisplayFullSettings::default();
    apply_params(param_set, time, &mut settings)?;

    // Retrieve instance data
    let gp = su.image_effect_suite.getPropertySet.ok_or(OfxStat::kOfxStatFailed)?;
    let gph = su.property_suite.propGetPointer.ok_or(OfxStat::kOfxStatFailed)?;
    let mut ep: OfxPropertySetHandle = ptr::null_mut();
    gp(effect, &mut ep).ofx_ok()?;
    let mut data_ptr: *mut c_void = ptr::null_mut();
    gph(ep, kOfxPropInstanceData.as_ptr(), 0, &mut data_ptr).ofx_ok()?;
    let mut idata = if data_ptr.is_null() { None } else { Some(&mut *(data_ptr as *mut InstanceData)) };

    // Normalize time by frame rate — VEGAS Pro gives generators frame numbers at 1000fps
    let mut frame_rate: f64 = 30.0;
    let _ = pgd(ep, kOfxImageEffectPropFrameRate.as_ptr(), 0, &mut frame_rate);
    let rate = if frame_rate > 0.0 { frame_rate } else { 30.0 };
    let time_seconds = time / rate;

    // Get output clip and its property set
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

    let ss: MidiDisplay = (&settings).into();
    let mut dst_buf = vec![0u8; width * height * 4];

    // Lazy recovery: read file data from String param if InstanceData is empty (fresh instance)
    // Lazy recovery: if InstanceData is empty (fresh instance after project load or undo),
    // pull file bytes from persisted params. This is a one-time cost, not per-frame.
    if idata.as_ref().and_then(|i| i.midi_data.as_ref()).is_none() {
        if let Ok(bytes) = file_param::read_file_data_base64(su, param_set) {
            if !bytes.is_empty() {
                if let Ok(md) = parse_midi_file(&bytes) {
                    if let Some(ref mut idata_inner) = idata {
                        idata_inner.midi_data = Some(md);
                    }
                }
            }
        }
    }

    // Render — always produce visible output (background fill as fallback)
    match idata {
        Some(ref idata_inner) => {
            if let Some(ref midi_data) = idata_inner.midi_data {
                ss.render(midi_data, &mut dst_buf, width, height, time_seconds);
            }
        }
        None => {}
    }
    // Fill with background color when midi_data is None (no file loaded yet)
    if idata.as_ref().and_then(|i| i.midi_data.as_ref()).is_none() {
        let bg_a = (ss.background_color_a * ss.background_opacity).clamp(0.0, 1.0);
        let br = (ss.background_color_r * 255.0).round() as u8;
        let bbg = (ss.background_color_g * 255.0).round() as u8;
        let bb = (ss.background_color_b * 255.0).round() as u8;
        let ba = (bg_a * 255.0).round() as u8;
        for chunk in dst_buf.chunks_exact_mut(4) {
            chunk[0] = br;
            chunk[1] = bbg;
            chunk[2] = bb;
            chunk[3] = ba;
        }
    }

    // Premultiply alpha
    if num_components == 4 {
        for pixel in dst_buf.chunks_exact_mut(4) {
            let a = pixel[3] as f32 / 255.0;
            pixel[0] = (pixel[0] as f32 * a).round() as u8;
            pixel[1] = (pixel[1] as f32 * a).round() as u8;
            pixel[2] = (pixel[2] as f32 * a).round() as u8;
        }
    }

    // Write to output
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
// Native RGBA helper
// ---------------------------------------------------------------------------

unsafe fn define_native_rgba(
    suites: &SuiteCache,
    param_set: OfxParamSetHandle,
    name: &CStr,
    label: &CStr,
    hint: &CStr,
    default_r: f64,
    default_g: f64,
    default_b: f64,
    default_a: f64,
) -> OfxResult<()> {
    let pdef = suites.parameter_suite.paramDefine.ok_or(OfxStat::kOfxStatFailed)?;
    let ps = suites.property_suite.propSetString.ok_or(OfxStat::kOfxStatFailed)?;
    let pd = suites.property_suite.propSetDouble.ok_or(OfxStat::kOfxStatFailed)?;
    let mut pp: OfxPropertySetHandle = ptr::null_mut();
    pdef(param_set, kOfxParamTypeRGBA.as_ptr(), name.as_ptr(), &mut pp).ofx_ok()?;
    ps(pp, kOfxPropLabel.as_ptr(), 0, label.as_ptr()).ofx_ok()?;
    ps(pp, kOfxParamPropHint.as_ptr(), 0, hint.as_ptr()).ofx_ok()?;
    pd(pp, kOfxParamPropDefault.as_ptr(), 0, default_r).ofx_ok()?;
    pd(pp, kOfxParamPropDefault.as_ptr(), 1, default_g).ofx_ok()?;
    pd(pp, kOfxParamPropDefault.as_ptr(), 2, default_b).ofx_ok()?;
    pd(pp, kOfxParamPropDefault.as_ptr(), 3, default_a).ofx_ok()?;
    ps(pp, kOfxParamPropParent.as_ptr(), 0, PAGE_NAME.as_ptr()).ofx_ok()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Parameter reading
// ---------------------------------------------------------------------------

unsafe fn apply_params(
    param_set: OfxParamSetHandle,
    time: f64,
    dst: &mut MidiDisplayFullSettings,
) -> OfxResult<()> {
    let d = data()?;
    let su = &d.suites;
    let pgh = su.parameter_suite.paramGetHandle.ok_or(OfxStat::kOfxStatFailed)?;
    let pgv = su.parameter_suite.paramGetValueAtTime.ok_or(OfxStat::kOfxStatFailed)?;

    // --- Native RGBA: noteColor ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, NOTE_COLOR_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut r: f64 = 0.0; let mut g: f64 = 0.0;
        let mut b: f64 = 0.0; let mut a: f64 = 0.0;
        pgv(p, time, &mut r, &mut g, &mut b, &mut a).ofx_ok()?;
        dst.set_field::<f32>(&setting_id::NOTE_COLOR_R, r.clamp(0.0, 1.0) as f32).map_err(|_| OfxStat::kOfxStatFailed)?;
        dst.set_field::<f32>(&setting_id::NOTE_COLOR_G, g.clamp(0.0, 1.0) as f32).map_err(|_| OfxStat::kOfxStatFailed)?;
        dst.set_field::<f32>(&setting_id::NOTE_COLOR_B, b.clamp(0.0, 1.0) as f32).map_err(|_| OfxStat::kOfxStatFailed)?;
        dst.set_field::<f32>(&setting_id::NOTE_COLOR_A, a.clamp(0.0, 1.0) as f32).map_err(|_| OfxStat::kOfxStatFailed)?;
    }

    // --- Native RGBA: noteBorderColor ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, NOTE_BORDER_COLOR_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut r: f64 = 0.0; let mut g: f64 = 0.0;
        let mut b: f64 = 0.0; let mut a: f64 = 0.0;
        pgv(p, time, &mut r, &mut g, &mut b, &mut a).ofx_ok()?;
        dst.set_field::<f32>(&setting_id::NOTE_BORDER_COLOR_R, r.clamp(0.0, 1.0) as f32).map_err(|_| OfxStat::kOfxStatFailed)?;
        dst.set_field::<f32>(&setting_id::NOTE_BORDER_COLOR_G, g.clamp(0.0, 1.0) as f32).map_err(|_| OfxStat::kOfxStatFailed)?;
        dst.set_field::<f32>(&setting_id::NOTE_BORDER_COLOR_B, b.clamp(0.0, 1.0) as f32).map_err(|_| OfxStat::kOfxStatFailed)?;
        dst.set_field::<f32>(&setting_id::NOTE_BORDER_COLOR_A, a.clamp(0.0, 1.0) as f32).map_err(|_| OfxStat::kOfxStatFailed)?;
    }

    // --- Native RGBA: backgroundColor ---
    {
        let mut p: OfxParamHandle = ptr::null_mut();
        pgh(param_set, BACKGROUND_COLOR_PARAM.as_ptr(), &mut p, ptr::null_mut()).ofx_ok()?;
        let mut r: f64 = 0.0; let mut g: f64 = 0.0;
        let mut b: f64 = 0.0; let mut a: f64 = 0.0;
        pgv(p, time, &mut r, &mut g, &mut b, &mut a).ofx_ok()?;
        dst.set_field::<f32>(&setting_id::BACKGROUND_COLOR_R, r.clamp(0.0, 1.0) as f32).map_err(|_| OfxStat::kOfxStatFailed)?;
        dst.set_field::<f32>(&setting_id::BACKGROUND_COLOR_G, g.clamp(0.0, 1.0) as f32).map_err(|_| OfxStat::kOfxStatFailed)?;
        dst.set_field::<f32>(&setting_id::BACKGROUND_COLOR_B, b.clamp(0.0, 1.0) as f32).map_err(|_| OfxStat::kOfxStatFailed)?;
        dst.set_field::<f32>(&setting_id::BACKGROUND_COLOR_A, a.clamp(0.0, 1.0) as f32).map_err(|_| OfxStat::kOfxStatFailed)?;
    }

    // --- Read generic params ---
    for desc in d.settings_list.all_descriptors() {
        if is_native_grouped_name(desc.id.name) { continue; }
        read_generic_param(su, param_set, time, desc, dst, &d.strings)?;
    }

    Ok(())
}
