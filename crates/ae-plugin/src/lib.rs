#![cfg(any(windows, target_os = "macos"))]

mod shared;

use std::sync::atomic::{AtomicU8, Ordering};

use after_effects::{self as ae, Error, Layer};
use zzzfx::{
    i18n,
    settings::{Settings, TrKey},
    CompositorLayer,
    AmbientLight, AmbientLightFullSettings,
    AsciiArt, AsciiArtFullSettings,
    AssSubtitleFullSettings,
    CastShadow, CastShadowFullSettings,
    ChromaKey, ChromaKeyFullSettings,
    LaTeXDisplayFullSettings,
    LongShadow, LongShadowFullSettings,
    MidiDisplayFullSettings,
    PixelArt, PixelArtFullSettings,
    QrCodeFullSettings,
    Repeater, RepeaterFullSettings,
    SpriteSheetFullSettings,
    Stroke, StrokeFullSettings,
    SvgDisplayFullSettings,
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
    CastShadow = 7,
    ChromaKey = 8,
    MidiDisplay = 9,
    SvgDisplay = 10,
    LaTeXDisplay = 11,
    QrCode = 12,
    AssSubtitle = 13,
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
        7 => EffectType::CastShadow,
        8 => EffectType::ChromaKey,
        9 => EffectType::MidiDisplay,
        10 => EffectType::SvgDisplay,
        11 => EffectType::LaTeXDisplay,
        12 => EffectType::QrCode,
        13 => EffectType::AssSubtitle,
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
        // AE uses ARGB; effect uses RGBA. Swap before and after.
        argb_to_rgba(&mut src);
        e.apply_effect(&src, &mut dst, $w, $h);
        rgba_to_argb(&mut dst);
        copy_contiguous_to_layer(&dst, $out, $w, $h);
    }};
}

// ── ARGB ↔ RGBA byte-swap helpers (AE pixel format conversion) ──────────

fn argb_to_rgba(buf: &mut [u8]) {
    for px in buf.chunks_exact_mut(4) {
        let a = px[0]; let r = px[1]; let g = px[2]; let b = px[3];
        px[0] = r; px[1] = g; px[2] = b; px[3] = a;
    }
}
fn rgba_to_argb(buf: &mut [u8]) {
    for px in buf.chunks_exact_mut(4) {
        let r = px[0]; let g = px[1]; let b = px[2]; let a = px[3];
        px[0] = a; px[1] = r; px[2] = g; px[3] = b;
    }
}

// ---------------------------------------------------------------------------
// Plugin struct
// ---------------------------------------------------------------------------

struct Plugin {
    stroke: zzzfx::settings::SettingsList<StrokeFullSettings>,
    repeater: zzzfx::settings::SettingsList<RepeaterFullSettings>,
    sprite_sheet: zzzfx::settings::SettingsList<SpriteSheetFullSettings>,
    ascii_art: zzzfx::settings::SettingsList<AsciiArtFullSettings>,
    pixel_art: zzzfx::settings::SettingsList<PixelArtFullSettings>,
    ambient_light: zzzfx::settings::SettingsList<AmbientLightFullSettings>,
    long_shadow: zzzfx::settings::SettingsList<LongShadowFullSettings>,
    cast_shadow: zzzfx::settings::SettingsList<CastShadowFullSettings>,
    chroma_key: zzzfx::settings::SettingsList<ChromaKeyFullSettings>,
    midi_display: zzzfx::settings::SettingsList<MidiDisplayFullSettings>,
    svg_display: zzzfx::settings::SettingsList<SvgDisplayFullSettings>,
    latex_display: zzzfx::settings::SettingsList<LaTeXDisplayFullSettings>,
    qr_code: zzzfx::settings::SettingsList<QrCodeFullSettings>,
    ass_subtitle: zzzfx::settings::SettingsList<AssSubtitleFullSettings>,
}

impl Default for Plugin {
    fn default() -> Self {
        Self {
            stroke: zzzfx::settings::SettingsList::<StrokeFullSettings>::new(),
            repeater: zzzfx::settings::SettingsList::<RepeaterFullSettings>::new(),
            sprite_sheet: zzzfx::settings::SettingsList::<SpriteSheetFullSettings>::new(),
            ascii_art: zzzfx::settings::SettingsList::<AsciiArtFullSettings>::new(),
            pixel_art: zzzfx::settings::SettingsList::<PixelArtFullSettings>::new(),
            ambient_light: zzzfx::settings::SettingsList::<AmbientLightFullSettings>::new(),
            long_shadow: zzzfx::settings::SettingsList::<LongShadowFullSettings>::new(),
            cast_shadow: zzzfx::settings::SettingsList::<CastShadowFullSettings>::new(),
            chroma_key: zzzfx::settings::SettingsList::<ChromaKeyFullSettings>::new(),
            midi_display: zzzfx::settings::SettingsList::<MidiDisplayFullSettings>::new(),
            svg_display: zzzfx::settings::SettingsList::<SvgDisplayFullSettings>::new(),
            latex_display: zzzfx::settings::SettingsList::<LaTeXDisplayFullSettings>::new(),
            qr_code: zzzfx::settings::SettingsList::<QrCodeFullSettings>::new(),
            ass_subtitle: zzzfx::settings::SettingsList::<AssSubtitleFullSettings>::new(),
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

effect_entry!(EffectMainStroke,       EffectType::Stroke);
effect_entry!(EffectMainRepeater,      EffectType::Repeater);
effect_entry!(EffectMainSpriteSheet,   EffectType::SpriteSheet);
effect_entry!(EffectMainAsciiArt,      EffectType::AsciiArt);
effect_entry!(EffectMainPixelArt,      EffectType::PixelArt);
effect_entry!(EffectMainAmbientLight,  EffectType::AmbientLight);
effect_entry!(EffectMainLongShadow,    EffectType::LongShadow);
effect_entry!(EffectMainCastShadow,    EffectType::CastShadow);
effect_entry!(EffectMainChromaKey,     EffectType::ChromaKey);
effect_entry!(EffectMainMidiDisplay,   EffectType::MidiDisplay);
effect_entry!(EffectMainSvgDisplay,    EffectType::SvgDisplay);
effect_entry!(EffectMainLaTeXDisplay,  EffectType::LaTeXDisplay);
effect_entry!(EffectMainQrCode,        EffectType::QrCode);
effect_entry!(EffectMainAssSubtitle,   EffectType::AssSubtitle);

// ---------------------------------------------------------------------------
// AdobePluginGlobal
// ---------------------------------------------------------------------------

impl AdobePluginGlobal for Plugin {
    fn params_setup(&self, params: &mut Parameters<ParamID>, _in: InData, _out: OutData) -> Result<(), Error> {
        match active_effect() {
            EffectType::Stroke => {
                let d = StrokeFullSettings::default();
                let l = StrokeFullSettings::legacy_value();
                map_params(params, &self.stroke.setting_descriptors, &d, &l)
            }
            EffectType::Repeater => {
                let d = RepeaterFullSettings::default();
                let l = RepeaterFullSettings::legacy_value();
                map_params(params, &self.repeater.setting_descriptors, &d, &l)
            }
            EffectType::SpriteSheet => {
                let d = SpriteSheetFullSettings::default();
                let l = SpriteSheetFullSettings::legacy_value();
                map_params(params, &self.sprite_sheet.setting_descriptors, &d, &l)
            }
            EffectType::AsciiArt => {
                let d = AsciiArtFullSettings::default();
                let l = AsciiArtFullSettings::legacy_value();
                map_params(params, &self.ascii_art.setting_descriptors, &d, &l)
            }
            EffectType::PixelArt => {
                let d = PixelArtFullSettings::default();
                let l = PixelArtFullSettings::legacy_value();
                map_params(params, &self.pixel_art.setting_descriptors, &d, &l)
            }
            EffectType::AmbientLight => {
                let d = AmbientLightFullSettings::default();
                let l = AmbientLightFullSettings::legacy_value();
                map_params(params, &self.ambient_light.setting_descriptors, &d, &l)
            }
            EffectType::LongShadow => {
                let d = LongShadowFullSettings::default();
                let l = LongShadowFullSettings::legacy_value();
                map_params(params, &self.long_shadow.setting_descriptors, &d, &l)
            }
            EffectType::CastShadow => {
                let d = CastShadowFullSettings::default();
                let l = CastShadowFullSettings::legacy_value();
                map_params(params, &self.cast_shadow.setting_descriptors, &d, &l)
            }
            EffectType::ChromaKey => {
                let d = ChromaKeyFullSettings::default();
                let l = ChromaKeyFullSettings::legacy_value();
                map_params(params, &self.chroma_key.setting_descriptors, &d, &l)
            }
            EffectType::MidiDisplay => {
                let d = MidiDisplayFullSettings::default();
                let l = MidiDisplayFullSettings::legacy_value();
                map_params(params, &self.midi_display.setting_descriptors, &d, &l)
            }
            EffectType::SvgDisplay => {
                let d = SvgDisplayFullSettings::default();
                let l = SvgDisplayFullSettings::legacy_value();
                map_params(params, &self.svg_display.setting_descriptors, &d, &l)
            }
            EffectType::LaTeXDisplay => {
                let d = LaTeXDisplayFullSettings::default();
                let l = LaTeXDisplayFullSettings::legacy_value();
                map_params(params, &self.latex_display.setting_descriptors, &d, &l)
            }
            EffectType::QrCode => {
                let d = QrCodeFullSettings::default();
                let l = QrCodeFullSettings::legacy_value();
                map_params(params, &self.qr_code.setting_descriptors, &d, &l)
            }
            EffectType::AssSubtitle => {
                let d = AssSubtitleFullSettings::default();
                let l = AssSubtitleFullSettings::legacy_value();
                map_params(params, &self.ass_subtitle.setting_descriptors, &d, &l)
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
            EffectType::CastShadow => (TrKey::EffectCastShadowName, TrKey::EffectCastShadowDesc),
            EffectType::ChromaKey => (TrKey::EffectChromaKeyName, TrKey::EffectChromaKeyDesc),
            EffectType::MidiDisplay => (TrKey::EffectMidiDisplayName, TrKey::EffectMidiDisplayDesc),
            EffectType::SvgDisplay => (TrKey::EffectSvgDisplayName, TrKey::EffectSvgDisplayDesc),
            EffectType::LaTeXDisplay => (TrKey::EffectLaTeXDisplayName, TrKey::EffectLaTeXDisplayDesc),
            EffectType::QrCode => (TrKey::EffectQrCodeName, TrKey::EffectQrCodeDesc),
            EffectType::AssSubtitle => (TrKey::EffectAssSubtitleName, TrKey::EffectAssSubtitleDesc),
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
                render_filter!(StrokeFullSettings, Stroke, &self.stroke.setting_descriptors, &in_layer, &mut out_layer, w, h, total, params);
            }
            EffectType::Repeater => {
                let mut s = RepeaterFullSettings::default();
                apply_settings_list(&self.repeater.setting_descriptors, params, &mut s)?;
                let mut src = vec![0u8; total];
                copy_layer_to_contiguous(&in_layer, &mut src, w, h);
                argb_to_rgba(&mut src);
                let layers = [CompositorLayer { rgba: &src, position_x: s.position_x, position_y: s.position_y, rotation_deg: s.rotation, blend_mode: s.blend_mode }];
                let mut dst = vec![0u8; total];
                let r: Repeater = (&s).into();
                r.composite_layers(&layers, &mut dst, w, h);
                rgba_to_argb(&mut dst);
                copy_contiguous_to_layer(&dst, &mut out_layer, w, h);
            }
            EffectType::SpriteSheet => return Err(Error::BadCallbackParameter),
            EffectType::AsciiArt => {
                render_filter!(AsciiArtFullSettings, AsciiArt, &self.ascii_art.setting_descriptors, &in_layer, &mut out_layer, w, h, total, params);
            }
            EffectType::PixelArt => {
                render_filter!(PixelArtFullSettings, PixelArt, &self.pixel_art.setting_descriptors, &in_layer, &mut out_layer, w, h, total, params);
            }
            EffectType::AmbientLight => {
                let mut s = AmbientLightFullSettings::default();
                apply_settings_list(&self.ambient_light.setting_descriptors, params, &mut s)?;
                let e: AmbientLight = (&s).into();
                let mut fg = vec![0u8; total];
                let mut bg = vec![0u8; total];
                let mut dst = vec![0u8; total];
                copy_layer_to_contiguous(&in_layer, &mut fg, w, h);
                argb_to_rgba(&mut fg);
                bg.copy_from_slice(&fg);
                e.apply_effect(&fg, &bg, &mut dst, w, h);
                rgba_to_argb(&mut dst);
                copy_contiguous_to_layer(&dst, &mut out_layer, w, h);
            }
            EffectType::LongShadow => {
                render_filter!(LongShadowFullSettings, LongShadow, &self.long_shadow.setting_descriptors, &in_layer, &mut out_layer, w, h, total, params);
            }
            EffectType::CastShadow => {
                render_filter!(CastShadowFullSettings, CastShadow, &self.cast_shadow.setting_descriptors, &in_layer, &mut out_layer, w, h, total, params);
            }
            EffectType::ChromaKey => {
                render_filter!(ChromaKeyFullSettings, ChromaKey, &self.chroma_key.setting_descriptors, &in_layer, &mut out_layer, w, h, total, params);
            }
            EffectType::MidiDisplay => {
                let mut s = MidiDisplayFullSettings::default();
                apply_settings_list(&self.midi_display.setting_descriptors, params, &mut s)?;
                let mut src = vec![0u8; total];
                let mut dst = vec![0u8; total];
                copy_layer_to_contiguous(&in_layer, &mut src, w, h);
                argb_to_rgba(&mut src);
                // MIDI Display requires file loading + MIDI parsing — not yet wired for AE.
                dst.copy_from_slice(&src);
                rgba_to_argb(&mut dst);
                copy_contiguous_to_layer(&dst, &mut out_layer, w, h);
            }
            EffectType::SvgDisplay | EffectType::LaTeXDisplay | EffectType::QrCode | EffectType::AssSubtitle => {
                // Generator effects require file/data input — pass through source for now.
                let mut src = vec![0u8; total];
                copy_layer_to_contiguous(&in_layer, &mut src, w, h);
                copy_contiguous_to_layer(&src, &mut out_layer, w, h);
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
                let mut tmp = AsciiArtFullSettings::default();
                apply_settings_list(&self.ascii_art.setting_descriptors, params, &mut tmp)?;
                let cm = tmp.color_mode as u32;
                let is_solid = cm == AsciiColorMode::Solid as u32 || cm == AsciiColorMode::SolidMapGrayscale as u32;
                if let Ok(p) = params.get(ParamID::Param(ascii_art_setting_id::FONT_COLOR.ae_id())) {
                    let was = p.ui_flags().contains(ae::ParamUIFlags::DISABLED);
                    if was == is_solid {
                        let mut p = p.clone();
                        p.set_ui_flag(ae::ParamUIFlags::DISABLED, !is_solid);
                        p.update_param_ui()?;
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
            EffectType::CastShadow => update_controls_disabled(params, &self.cast_shadow.setting_descriptors, true),
            EffectType::ChromaKey => update_controls_disabled(params, &self.chroma_key.setting_descriptors, true),
            EffectType::MidiDisplay => update_controls_disabled(params, &self.midi_display.setting_descriptors, true),
            EffectType::SvgDisplay => update_controls_disabled(params, &self.svg_display.setting_descriptors, true),
            EffectType::LaTeXDisplay => update_controls_disabled(params, &self.latex_display.setting_descriptors, true),
            EffectType::QrCode => update_controls_disabled(params, &self.qr_code.setting_descriptors, true),
            EffectType::AssSubtitle => update_controls_disabled(params, &self.ass_subtitle.setting_descriptors, true),
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
