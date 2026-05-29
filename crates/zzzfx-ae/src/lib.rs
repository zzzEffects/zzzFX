#![cfg(any(windows, target_os = "macos"))]

mod handle;
mod shared;

use std::sync::atomic::{AtomicU8, Ordering};

use after_effects::{self as ae, Error, Layer};
use zzzfx_core::{
    i18n,
    settings::{Settings, TrKey},
    CompositorLayer,
    ZzzAmbientLight, ZzzAmbientLightFullSettings,
    ZzzAsciiArt, ZzzAsciiArtFullSettings,
    ZzzLongShadow, ZzzLongShadowFullSettings,
    ZzzPixelArt, ZzzPixelArtFullSettings,
    ZzzRepeater, ZzzRepeaterFullSettings,
    ZzzSpriteSheetFullSettings,
    ZzzStroke, ZzzStrokeFullSettings,
    ascii_art_setting_id, AsciiColorMode,
    pixel_art_setting_id,
};
use shared::{
    IDExt, ParamID,
    apply_settings_list,
    copy_contiguous_to_layer, copy_layer_to_contiguous,
    global_setup_common, pre_render_common,
    map_params, update_controls_disabled,
};

// ---------------------------------------------------------------------------
// Effect type dispatch
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Clone, Copy)]
enum EffectType {
    Stroke = 0,
    Repeater = 1,
    SpriteSheet = 2,
    AsciiArt = 3,
    PixelArt = 4,
    AmbientLight = 5,
    LongShadow = 6,
}

static ACTIVE_EFFECT: AtomicU8 = AtomicU8::new(EffectType::Stroke as u8);

fn active_effect() -> EffectType {
    match ACTIVE_EFFECT.load(Ordering::Acquire) {
        1 => EffectType::Repeater,
        2 => EffectType::SpriteSheet,
        3 => EffectType::AsciiArt,
        4 => EffectType::PixelArt,
        5 => EffectType::AmbientLight,
        6 => EffectType::LongShadow,
        _ => EffectType::Stroke,
    }
}

// ── Helper: standard single-input apply_effect pattern ───────────────────
macro_rules! render_filter {
    ($stype:ty, $etype:ty, $desc:expr, $in:expr, $out:expr, $w:expr, $h:expr, $total:expr, $params:expr) => {{
        let mut s = <$stype>::default();
        apply_settings_list($desc, $params, &mut s)?;
        let e: $etype = (&s).into();
        let mut src = vec![0u8; $total];
        let mut dst = vec![0u8; $total];
        copy_layer_to_contiguous($in, &mut src, $w, $h);
        e.apply_effect(&src, &mut dst, $w, $h);
        copy_contiguous_to_layer(&dst, $out, $w, $h);
    }};
}

// ---------------------------------------------------------------------------
// Plugin struct
// ---------------------------------------------------------------------------

struct Plugin {
    stroke: zzzfx_core::settings::SettingsList<ZzzStrokeFullSettings>,
    repeater: zzzfx_core::settings::SettingsList<ZzzRepeaterFullSettings>,
    sprite_sheet: zzzfx_core::settings::SettingsList<ZzzSpriteSheetFullSettings>,
    ascii_art: zzzfx_core::settings::SettingsList<ZzzAsciiArtFullSettings>,
    pixel_art: zzzfx_core::settings::SettingsList<ZzzPixelArtFullSettings>,
    ambient_light: zzzfx_core::settings::SettingsList<ZzzAmbientLightFullSettings>,
    long_shadow: zzzfx_core::settings::SettingsList<ZzzLongShadowFullSettings>,
}

impl Default for Plugin {
    fn default() -> Self {
        Self {
            stroke: zzzfx_core::settings::SettingsList::<ZzzStrokeFullSettings>::new(),
            repeater: zzzfx_core::settings::SettingsList::<ZzzRepeaterFullSettings>::new(),
            sprite_sheet: zzzfx_core::settings::SettingsList::<ZzzSpriteSheetFullSettings>::new(),
            ascii_art: zzzfx_core::settings::SettingsList::<ZzzAsciiArtFullSettings>::new(),
            pixel_art: zzzfx_core::settings::SettingsList::<ZzzPixelArtFullSettings>::new(),
            ambient_light: zzzfx_core::settings::SettingsList::<ZzzAmbientLightFullSettings>::new(),
            long_shadow: zzzfx_core::settings::SettingsList::<ZzzLongShadowFullSettings>::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

ae::define_effect!(Plugin, (), ParamID);

macro_rules! effect_entry {
    ($fn:ident, $eff:expr) => {
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn $fn(
            cmd: ae::sys::PF_Cmd,
            in_data_ptr: *mut ae::sys::PF_InData,
            out_data_ptr: *mut ae::sys::PF_OutData,
            params: *mut *mut ae::sys::PF_ParamDef,
            output: *mut ae::sys::PF_LayerDef,
            extra: *mut std::ffi::c_void,
        ) -> ae::sys::PF_Err {
            ACTIVE_EFFECT.store($eff as u8, Ordering::Release);
            unsafe { EffectMain(cmd, in_data_ptr, out_data_ptr, params, output, extra) }
        }
    };
}

effect_entry!(EffectMainRepeater,      EffectType::Repeater);
effect_entry!(EffectMainSpriteSheet,   EffectType::SpriteSheet);
effect_entry!(EffectMainAsciiArt,      EffectType::AsciiArt);
effect_entry!(EffectMainPixelArt,      EffectType::PixelArt);
effect_entry!(EffectMainAmbientLight,  EffectType::AmbientLight);
effect_entry!(EffectMainLongShadow,    EffectType::LongShadow);

// ---------------------------------------------------------------------------
// AdobePluginGlobal
// ---------------------------------------------------------------------------

impl AdobePluginGlobal for Plugin {
    fn params_setup(&self, params: &mut Parameters<ParamID>, _in: InData, _out: OutData) -> Result<(), Error> {
        match active_effect() {
            EffectType::Stroke => {
                let d = ZzzStrokeFullSettings::default();
                let l = ZzzStrokeFullSettings::legacy_value();
                map_params(params, &self.stroke.setting_descriptors, &d, &l)
            }
            EffectType::Repeater => {
                let d = ZzzRepeaterFullSettings::default();
                let l = ZzzRepeaterFullSettings::legacy_value();
                map_params(params, &self.repeater.setting_descriptors, &d, &l)
            }
            EffectType::SpriteSheet => {
                let d = ZzzSpriteSheetFullSettings::default();
                let l = ZzzSpriteSheetFullSettings::legacy_value();
                map_params(params, &self.sprite_sheet.setting_descriptors, &d, &l)
            }
            EffectType::AsciiArt => {
                let d = ZzzAsciiArtFullSettings::default();
                let l = ZzzAsciiArtFullSettings::legacy_value();
                map_params(params, &self.ascii_art.setting_descriptors, &d, &l)
            }
            EffectType::PixelArt => {
                let d = ZzzPixelArtFullSettings::default();
                let l = ZzzPixelArtFullSettings::legacy_value();
                map_params(params, &self.pixel_art.setting_descriptors, &d, &l)
            }
            EffectType::AmbientLight => {
                let d = ZzzAmbientLightFullSettings::default();
                let l = ZzzAmbientLightFullSettings::legacy_value();
                map_params(params, &self.ambient_light.setting_descriptors, &d, &l)
            }
            EffectType::LongShadow => {
                let d = ZzzLongShadowFullSettings::default();
                let l = ZzzLongShadowFullSettings::legacy_value();
                map_params(params, &self.long_shadow.setting_descriptors, &d, &l)
            }
        }
    }

    fn handle_command(
        &mut self, command: Command, in_data: InData, out_data: OutData, params: &mut Parameters<ParamID>,
    ) -> Result<(), Error> {
        match command {
            Command::GlobalSetup => self.global_setup(in_data),
            Command::About => self.about(out_data),
            Command::Render { in_layer, out_layer } => {
                if !in_data.is_premiere() { return Err(Error::BadCallbackParameter); }
                if in_layer.width() != out_layer.width() || in_layer.height() != out_layer.height() {
                    return Err(Error::BadCallbackParameter);
                }
                self.do_render(in_layer, out_layer, params)
            }
            Command::SmartPreRender { extra } => pre_render_common(in_data, extra),
            Command::SmartRender { extra } => {
                let Some(input) = extra.callbacks().checkout_layer_pixels(0)? else { return Ok(()); };
                let Some(output) = extra.callbacks().checkout_output()? else { return Ok(()); };
                self.do_render(input, output, params)
            }
            Command::UpdateParamsUi => self.update_params_ui(params),
            Command::GetFlattenedSequenceData => Ok(()),
            _ => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin methods
// ---------------------------------------------------------------------------

impl Plugin {
    fn global_setup(&self, in_data: InData) -> Result<(), Error> {
        i18n::set_lang(resolve_language());
        global_setup_common(in_data)
    }

    fn about(&self, mut out: OutData) -> Result<(), Error> {
        let (name, desc) = match active_effect() {
            EffectType::Stroke => (TrKey::EffectStrokeName, TrKey::EffectStrokeDesc),
            EffectType::Repeater => (TrKey::EffectRepeaterName, TrKey::EffectRepeaterDesc),
            EffectType::SpriteSheet => (TrKey::EffectSpritesheetName, TrKey::EffectSpritesheetDesc),
            EffectType::AsciiArt => (TrKey::EffectAsciiArtName, TrKey::EffectAsciiArtDesc),
            EffectType::PixelArt => (TrKey::EffectPixelArtName, TrKey::EffectPixelArtDesc),
            EffectType::AmbientLight => (TrKey::EffectAmbientLightName, TrKey::EffectAmbientLightDesc),
            EffectType::LongShadow => (TrKey::EffectLongShadowName, TrKey::EffectLongShadowDesc),
        };
        out.set_return_msg(&format!(
            "{} {}.{}.{}\r\r{}",
            i18n::tr(name),
            env!("EFFECT_VERSION_MAJOR"), env!("EFFECT_VERSION_MINOR"), env!("EFFECT_VERSION_PATCH"),
            i18n::tr(desc),
        ));
        Ok(())
    }

    fn do_render(&self, in_layer: Layer, mut out_layer: Layer, params: &mut Parameters<ParamID>) -> Result<(), Error> {
        let w = in_layer.width().min(out_layer.width()) as usize;
        let h = in_layer.height().min(out_layer.height()) as usize;
        let total = w * h * 4;

        match active_effect() {
            EffectType::Stroke => {
                render_filter!(ZzzStrokeFullSettings, ZzzStroke, &self.stroke.setting_descriptors, &in_layer, &mut out_layer, w, h, total, params);
            }
            EffectType::Repeater => {
                let mut s = ZzzRepeaterFullSettings::default();
                apply_settings_list(&self.repeater.setting_descriptors, params, &mut s)?;
                let mut src = vec![0u8; total];
                copy_layer_to_contiguous(&in_layer, &mut src, w, h);
                let layers = [CompositorLayer { rgba: &src, position_x: s.position_x, position_y: s.position_y, rotation_deg: s.rotation, blend_mode: s.blend_mode }];
                let mut dst = vec![0u8; total];
                let r: ZzzRepeater = (&s).into();
                r.composite_layers(&layers, &mut dst, w, h);
                copy_contiguous_to_layer(&dst, &mut out_layer, w, h);
            }
            EffectType::SpriteSheet => return Err(Error::BadCallbackParameter),
            EffectType::AsciiArt => {
                render_filter!(ZzzAsciiArtFullSettings, ZzzAsciiArt, &self.ascii_art.setting_descriptors, &in_layer, &mut out_layer, w, h, total, params);
            }
            EffectType::PixelArt => {
                render_filter!(ZzzPixelArtFullSettings, ZzzPixelArt, &self.pixel_art.setting_descriptors, &in_layer, &mut out_layer, w, h, total, params);
            }
            EffectType::AmbientLight => {
                let mut s = ZzzAmbientLightFullSettings::default();
                apply_settings_list(&self.ambient_light.setting_descriptors, params, &mut s)?;
                let e: ZzzAmbientLight = (&s).into();
                let mut fg = vec![0u8; total];
                let mut bg = vec![0u8; total];
                let mut dst = vec![0u8; total];
                copy_layer_to_contiguous(&in_layer, &mut fg, w, h);
                bg.copy_from_slice(&fg);
                e.apply_effect(&fg, &bg, &mut dst, w, h);
                copy_contiguous_to_layer(&dst, &mut out_layer, w, h);
            }
            EffectType::LongShadow => {
                render_filter!(ZzzLongShadowFullSettings, ZzzLongShadow, &self.long_shadow.setting_descriptors, &in_layer, &mut out_layer, w, h, total, params);
            }
        }
        Ok(())
    }

    fn update_params_ui(&self, params: &mut Parameters<ParamID>) -> Result<(), Error> {
        match active_effect() {
            EffectType::Stroke => update_controls_disabled(params, &self.stroke.setting_descriptors, true),
            EffectType::Repeater => update_controls_disabled(params, &self.repeater.setting_descriptors, true),
            EffectType::SpriteSheet => update_controls_disabled(params, &self.sprite_sheet.setting_descriptors, true),
            EffectType::AsciiArt => {
                update_controls_disabled(params, &self.ascii_art.setting_descriptors, true)?;
                let mut tmp = ZzzAsciiArtFullSettings::default();
                apply_settings_list(&self.ascii_art.setting_descriptors, params, &mut tmp)?;
                let cm = tmp.color_mode as u32;
                let is_solid = cm == AsciiColorMode::Solid as u32 || cm == AsciiColorMode::SolidMapGrayscale as u32;
                for sid in [ascii_art_setting_id::FONT_COLOR_R, ascii_art_setting_id::FONT_COLOR_G, ascii_art_setting_id::FONT_COLOR_B, ascii_art_setting_id::FONT_COLOR_A] {
                    if let Ok(p) = params.get(ParamID::Param(sid.ae_id())) {
                        let was = p.ui_flags().contains(ae::ParamUIFlags::DISABLED);
                        if was == is_solid {
                            let mut p = p.clone();
                            p.set_ui_flag(ae::ParamUIFlags::DISABLED, !is_solid);
                            p.update_param_ui()?;
                        }
                    }
                }
                Ok(())
            }
            EffectType::PixelArt => {
                let square = params.get(ParamID::Param(pixel_art_setting_id::SQUARE.ae_id()))?.as_checkbox()?.value();
                update_controls_disabled(params, &self.pixel_art.setting_descriptors, true)?;
                if square {
                    if let Ok(p) = params.get(ParamID::Param(pixel_art_setting_id::PIXEL_SIZE_V.ae_id())) {
                        let mut p = p.clone();
                        p.set_ui_flag(ae::ParamUIFlags::DISABLED, true);
                        p.update_param_ui()?;
                    }
                }
                Ok(())
            }
            EffectType::AmbientLight => update_controls_disabled(params, &self.ambient_light.setting_descriptors, true),
            EffectType::LongShadow => update_controls_disabled(params, &self.long_shadow.setting_descriptors, true),
        }
    }
}

// ---------------------------------------------------------------------------
// Language resolution
// ---------------------------------------------------------------------------

fn resolve_language() -> i18n::Lang {
    if let Some(tag) = get_ae_language_tag() {
        return i18n::lang_from_locale_tag(&tag).unwrap_or(i18n::Lang::En);
    }
    i18n::detect_system_lang()
}

fn get_ae_language_tag() -> Option<String> {
    let app = ae::suites::App::new().ok()?;
    app.language().ok()
}
