#![cfg(any(windows, target_os = "macos"))]

mod handle;
mod shared;

use after_effects::{self as ae, Error, Layer};
use zzzfx_core::{i18n, settings::{Settings, TrKey}};
use shared::{ParamID, apply_settings_list, global_setup_common, pre_render_common, map_params, update_controls_disabled, copy_layer_to_contiguous, copy_contiguous_to_layer};

// ---------------------------------------------------------------------------
// Effect-specific types selected by cargo feature
// ---------------------------------------------------------------------------

#[cfg(feature = "effect-stroke")]
use zzzfx_core::{ZzzStroke, ZzzStrokeFullSettings, settings::SettingsList as StrokeSettingsList};

#[cfg(feature = "effect-repeater")]
use zzzfx_core::{CompositorLayer, ZzzRepeater, ZzzRepeaterFullSettings, settings::SettingsList as RepeaterSettingsList};

#[cfg(feature = "effect-sprite-sheet")]
use zzzfx_core::{ZzzSpriteSheet, ZzzSpriteSheetFullSettings, settings::SettingsList as SpriteSheetSettingsList};

// ---------------------------------------------------------------------------
// Plugin struct
// ---------------------------------------------------------------------------

struct Plugin {
    #[cfg(feature = "effect-stroke")]
    settings: StrokeSettingsList<ZzzStrokeFullSettings>,
    #[cfg(feature = "effect-repeater")]
    settings: RepeaterSettingsList<ZzzRepeaterFullSettings>,
    #[cfg(feature = "effect-sprite-sheet")]
    settings: SpriteSheetSettingsList<ZzzSpriteSheetFullSettings>,
}

impl Default for Plugin {
    fn default() -> Self {
        Self {
            #[cfg(feature = "effect-stroke")]
            settings: StrokeSettingsList::<ZzzStrokeFullSettings>::new(),
            #[cfg(feature = "effect-repeater")]
            settings: RepeaterSettingsList::<ZzzRepeaterFullSettings>::new(),
            #[cfg(feature = "effect-sprite-sheet")]
            settings: SpriteSheetSettingsList::<ZzzSpriteSheetFullSettings>::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Effect entry point
// ---------------------------------------------------------------------------

ae::define_effect!(Plugin, (), ParamID);

// ---------------------------------------------------------------------------
// AdobePluginGlobal trait implementation
// ---------------------------------------------------------------------------

impl AdobePluginGlobal for Plugin {
    fn params_setup(
        &self,
        params: &mut Parameters<ParamID>,
        _in_data: InData,
        _out_data: OutData,
    ) -> Result<(), Error> {
        #[cfg(feature = "effect-stroke")]
        {
            let defaults = ZzzStrokeFullSettings::default();
            let legacy = ZzzStrokeFullSettings::legacy_value();
            map_params(params, &self.settings.setting_descriptors, &defaults, &legacy)?;
        }
        #[cfg(feature = "effect-repeater")]
        {
            let defaults = ZzzRepeaterFullSettings::default();
            let legacy = ZzzRepeaterFullSettings::legacy_value();
            map_params(params, &self.settings.setting_descriptors, &defaults, &legacy)?;
        }
        #[cfg(feature = "effect-sprite-sheet")]
        {
            let defaults = ZzzSpriteSheetFullSettings::default();
            let legacy = ZzzSpriteSheetFullSettings::legacy_value();
            map_params(params, &self.settings.setting_descriptors, &defaults, &legacy)?;
        }
        Ok(())
    }

    fn handle_command(
        &mut self,
        command: Command,
        in_data: InData,
        out_data: OutData,
        params: &mut Parameters<ParamID>,
    ) -> Result<(), Error> {
        match command {
            Command::GlobalSetup => self.global_setup(in_data, out_data, params)?,
            Command::About => self.about(in_data, out_data)?,
            Command::Render { in_layer, out_layer } => {
                self.legacy_render(in_data, out_data, in_layer, out_layer, params)?
            }
            Command::SmartPreRender { extra } => self.pre_render(in_data, out_data, extra)?,
            Command::SmartRender { extra } => self.smart_render(in_data, out_data, extra, params)?,
            Command::UpdateParamsUi => {
                #[cfg(feature = "effect-stroke")]
                update_controls_disabled(params, &self.settings.setting_descriptors, true)?;
                #[cfg(feature = "effect-repeater")]
                update_controls_disabled(params, &self.settings.setting_descriptors, true)?;
                #[cfg(feature = "effect-sprite-sheet")]
                update_controls_disabled(params, &self.settings.setting_descriptors, true)?;
            }
            Command::GetFlattenedSequenceData => {}
            _ => {}
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Plugin implementation — common methods
// ---------------------------------------------------------------------------

impl Plugin {
    fn global_setup(
        &self,
        in_data: InData,
        _out_data: OutData,
        _params: &mut Parameters<ParamID>,
    ) -> Result<(), Error> {
        i18n::set_lang(resolve_language());
        global_setup_common(in_data)
    }

    fn about(&self, _in_data: InData, mut out_data: OutData) -> Result<(), Error> {
        #[cfg(feature = "effect-stroke")]
        let (name, desc) = (
            i18n::tr(TrKey::EffectStrokeName),
            i18n::tr(TrKey::EffectStrokeDesc),
        );
        #[cfg(feature = "effect-repeater")]
        let (name, desc) = (
            i18n::tr(TrKey::EffectRepeaterName),
            i18n::tr(TrKey::EffectRepeaterDesc),
        );
        #[cfg(feature = "effect-sprite-sheet")]
        let (name, desc) = (
            i18n::tr(TrKey::EffectSpritesheetName),
            i18n::tr(TrKey::EffectSpritesheetDesc),
        );

        out_data.set_return_msg(
            format!(
                "{} {}.{}.{}\r\r{}",
                name,
                env!("EFFECT_VERSION_MAJOR"),
                env!("EFFECT_VERSION_MINOR"),
                env!("EFFECT_VERSION_PATCH"),
                desc,
            )
            .as_str(),
        );
        Ok(())
    }

    fn pre_render(
        &self,
        in_data: InData,
        _out_data: OutData,
        extra: PreRenderExtra,
    ) -> Result<(), Error> {
        pre_render_common(in_data, extra)
    }

    fn legacy_render(
        &self,
        in_data: InData,
        _out_data: OutData,
        in_layer: Layer,
        out_layer: Layer,
        params: &mut Parameters<ParamID>,
    ) -> Result<(), Error> {
        if !in_data.is_premiere() { return Err(Error::BadCallbackParameter); }
        if in_layer.width() != out_layer.width() || in_layer.height() != out_layer.height() {
            return Err(Error::BadCallbackParameter);
        }
        self.do_render(in_layer, out_layer, params)
    }

    fn smart_render(
        &self,
        _in_data: InData,
        _out_data: OutData,
        extra: SmartRenderExtra,
        params: &mut Parameters<ParamID>,
    ) -> Result<(), Error> {
        let Some(input_world) = extra.callbacks().checkout_layer_pixels(0)? else { return Ok(()); };
        let Some(output_world) = extra.callbacks().checkout_output()? else { return Ok(()); };
        self.do_render(input_world, output_world, params)
    }

    #[cfg(feature = "effect-stroke")]
    fn do_render(
        &self,
        in_layer: Layer,
        mut out_layer: Layer,
        params: &mut Parameters<ParamID>,
    ) -> Result<(), Error> {
        let mut full_settings = ZzzStrokeFullSettings::default();
        apply_settings_list(&self.settings.setting_descriptors, params, &mut full_settings)?;
        let effect: ZzzStroke = (&full_settings).into();

        let width = in_layer.width().min(out_layer.width()) as usize;
        let height = in_layer.height().min(out_layer.height()) as usize;
        let total = width * height * 4;

        let mut src_buf = vec![0u8; total];
        let mut dst_buf = vec![0u8; total];

        copy_layer_to_contiguous(&in_layer, &mut src_buf, width, height);
        effect.apply_effect(&src_buf, &mut dst_buf, width, height);
        copy_contiguous_to_layer(&dst_buf, &mut out_layer, width, height);
        Ok(())
    }

    #[cfg(feature = "effect-repeater")]
    fn do_render(
        &self,
        in_layer: Layer,
        mut out_layer: Layer,
        params: &mut Parameters<ParamID>,
    ) -> Result<(), Error> {
        let mut full_settings = ZzzRepeaterFullSettings::default();
        apply_settings_list(&self.settings.setting_descriptors, params, &mut full_settings)?;
        let repeater: ZzzRepeater = (&full_settings).into();

        let width = in_layer.width().min(out_layer.width()) as usize;
        let height = in_layer.height().min(out_layer.height()) as usize;
        let total = width * height * 4;

        let mut src_buf = vec![0u8; total];
        copy_layer_to_contiguous(&in_layer, &mut src_buf, width, height);

        let layers = [CompositorLayer {
            rgba: &src_buf,
            position_x: full_settings.position_x,
            position_y: full_settings.position_y,
            rotation_deg: full_settings.rotation,
            blend_mode: full_settings.blend_mode,
        }];

        let mut dst_buf = vec![0u8; total];
        repeater.composite_layers(&layers, &mut dst_buf, width, height);
        copy_contiguous_to_layer(&dst_buf, &mut out_layer, width, height);
        Ok(())
    }

    #[cfg(feature = "effect-sprite-sheet")]
    fn do_render(
        &self,
        _in_layer: Layer,
        mut _out_layer: Layer,
        _params: &mut Parameters<ParamID>,
    ) -> Result<(), Error> {
        Err(Error::BadCallbackParameter)
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
