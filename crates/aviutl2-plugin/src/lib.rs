use anyhow::Result;
use aviutl2::filter::{
    FilterConfigItem, FilterPlugin, FilterPluginFlags, FilterPluginTable,
    FilterProcVideo,
};
use aviutl2::generic::{GenericPlugin, GenericPluginTable, HostAppHandle, SubPlugin};
use aviutl2::AviUtl2Info;
use zzzfx::i18n::ja;
use zzzfx::settings::Settings;
use zzzfx::TrKey;

mod aul2;
mod params;

pub use aul2::{
    generate_aul2_en, generate_aul2_ko, generate_aul2_zh_cn, write_aul2_to,
};
use params::build_config_items;

// ── GenericPlugin (top-level entry point) ──────────────────────────

#[aviutl2::plugin(GenericPlugin)]
struct ZzzFxPlugin {
    stroke_filter: SubPlugin<StrokeFilter>,
    long_shadow_filter: SubPlugin<LongShadowFilter>,
    cast_shadow_filter: SubPlugin<CastShadowFilter>,
    ascii_art_filter: SubPlugin<AsciiArtFilter>,
    pixel_art_filter: SubPlugin<PixelArtFilter>,
    chroma_key_filter: SubPlugin<ChromaKeyFilter>,
    ambient_light_filter: SubPlugin<AmbientLightFilter>,
    repeater_filter: SubPlugin<RepeaterFilter>,
    sprite_sheet_filter: SubPlugin<SpriteSheetFilter>,
    ass_subtitle_filter: SubPlugin<AssSubtitleFilter>,
    midi_display_filter: SubPlugin<MidiDisplayFilter>,
    svg_display_filter: SubPlugin<SvgDisplayFilter>,
    latex_display_filter: SubPlugin<LaTeXDisplayFilter>,
    qr_code_filter: SubPlugin<QrCodeFilter>,
}

impl GenericPlugin for ZzzFxPlugin {
    fn new(info: AviUtl2Info) -> Result<Self> {
        let _ = aviutl2::tracing_subscriber::fmt()
            .with_max_level(aviutl2::tracing::Level::WARN)
            .event_format(aviutl2::logger::AviUtl2Formatter)
            .with_writer(aviutl2::logger::AviUtl2LogWriter)
            .try_init();

        zzzfx::i18n::set_lang(zzzfx::i18n::detect_system_lang());

        Ok(Self {
            stroke_filter: SubPlugin::<StrokeFilter>::new_filter_plugin(&info)?,
            long_shadow_filter: SubPlugin::<LongShadowFilter>::new_filter_plugin(&info)?,
            cast_shadow_filter: SubPlugin::<CastShadowFilter>::new_filter_plugin(&info)?,
            ascii_art_filter: SubPlugin::<AsciiArtFilter>::new_filter_plugin(&info)?,
            pixel_art_filter: SubPlugin::<PixelArtFilter>::new_filter_plugin(&info)?,
            chroma_key_filter: SubPlugin::<ChromaKeyFilter>::new_filter_plugin(&info)?,
            ambient_light_filter: SubPlugin::<AmbientLightFilter>::new_filter_plugin(&info)?,
            repeater_filter: SubPlugin::<RepeaterFilter>::new_filter_plugin(&info)?,
            sprite_sheet_filter: SubPlugin::<SpriteSheetFilter>::new_filter_plugin(&info)?,
            ass_subtitle_filter: SubPlugin::<AssSubtitleFilter>::new_filter_plugin(&info)?,
            midi_display_filter: SubPlugin::<MidiDisplayFilter>::new_filter_plugin(&info)?,
            svg_display_filter: SubPlugin::<SvgDisplayFilter>::new_filter_plugin(&info)?,
            latex_display_filter: SubPlugin::<LaTeXDisplayFilter>::new_filter_plugin(&info)?,
            qr_code_filter: SubPlugin::<QrCodeFilter>::new_filter_plugin(&info)?,
        })
    }

    fn plugin_info(&self) -> GenericPluginTable {
        GenericPluginTable {
            name: "zzzFX".into(),
            information: "zzzFX multi-effect plugin for AviUtl2".into(),
        }
    }

    fn register(&mut self, registry: &mut HostAppHandle) {
        registry.register_filter_plugin(&self.stroke_filter);
        registry.register_filter_plugin(&self.long_shadow_filter);
        registry.register_filter_plugin(&self.cast_shadow_filter);
        registry.register_filter_plugin(&self.ascii_art_filter);
        registry.register_filter_plugin(&self.pixel_art_filter);
        registry.register_filter_plugin(&self.chroma_key_filter);
        registry.register_filter_plugin(&self.ambient_light_filter);
        registry.register_filter_plugin(&self.repeater_filter);
        registry.register_filter_plugin(&self.sprite_sheet_filter);
        registry.register_filter_plugin(&self.ass_subtitle_filter);
        registry.register_filter_plugin(&self.midi_display_filter);
        registry.register_filter_plugin(&self.svg_display_filter);
        registry.register_filter_plugin(&self.latex_display_filter);
        registry.register_filter_plugin(&self.qr_code_filter);
    }
}

// ── Helper: Japanese filter name from TrKey ───────────────────────

fn ja_tr(key: TrKey) -> String {
    ja::translate_cstr(key)
        .to_str()
        .unwrap_or_else(|_| key.en()) // fall back to English if UTF-8 conversion fails
        .to_string()
}

// ── Macro: apply_effect-based FilterPlugin ────────────────────────

macro_rules! apply_effect_filter {
    ($struct:ident, $full_settings:ty, $effect:ty, $name_key:expr, $desc_key:expr) => {
        #[aviutl2::plugin(FilterPlugin)]
        struct $struct;

        impl FilterPlugin for $struct {
            fn new(_info: AviUtl2Info) -> Result<Self> {
                Ok(Self)
            }

            fn plugin_info(&self) -> FilterPluginTable {
                FilterPluginTable {
                    name: ja_tr($name_key),
                    label: Some("zzzFX".into()),
                    information: ja_tr($desc_key),
                    flags: aviutl2::bitflag!(FilterPluginFlags {
                        video: true,
                        filter: true,
                    }),
                    config_items: build_config_items::<$full_settings>(),
                }
            }

            fn proc_video(
                &self,
                config: &[FilterConfigItem],
                video: &mut FilterProcVideo,
            ) -> Result<()> {
                let mut settings = <$full_settings>::default();
                read_config(config, &mut settings);
                let effect: $effect = settings.into();
                let w = video.video_object.width as usize;
                let h = video.video_object.height as usize;
                if w == 0 || h == 0 {
                    return Ok(());
                }
                let len = w * h * 4;
                thread_local! {
                    static RENDER_BUFS: std::cell::RefCell<(Vec<u8>, Vec<u8>)> =
                        std::cell::RefCell::new((Vec::new(), Vec::new()));
                }
                let (mut src, mut dst) = RENDER_BUFS.with(|c| {
                    let (ref mut s, ref mut d) = *c.borrow_mut();
                    s.resize(len, 0);
                    d.resize(len, 0);
                    (std::mem::take(s), std::mem::take(d))
                });
                video.get_image_data(&mut src);
                effect.apply_effect(&src, &mut dst, w, h);
                video.set_image_data(&dst, video.video_object.width, video.video_object.height);
                RENDER_BUFS.with(|c| {
                    *c.borrow_mut() = (src, dst);
                });
                Ok(())
            }
        }
    };
}

// ── 5 simple filter effects with apply_effect(src, dst, w, h) ────

apply_effect_filter!(
    StrokeFilter, zzzfx::StrokeFullSettings, zzzfx::Stroke,
    TrKey::EffectStrokeName, TrKey::EffectStrokeDesc
);
apply_effect_filter!(
    LongShadowFilter, zzzfx::LongShadowFullSettings, zzzfx::LongShadow,
    TrKey::EffectLongShadowName, TrKey::EffectLongShadowDesc
);
apply_effect_filter!(
    CastShadowFilter, zzzfx::CastShadowFullSettings, zzzfx::CastShadow,
    TrKey::EffectCastShadowName, TrKey::EffectCastShadowDesc
);
apply_effect_filter!(
    AsciiArtFilter, zzzfx::AsciiArtFullSettings, zzzfx::AsciiArt,
    TrKey::EffectAsciiArtName, TrKey::EffectAsciiArtDesc
);
apply_effect_filter!(
    PixelArtFilter, zzzfx::PixelArtFullSettings, zzzfx::PixelArt,
    TrKey::EffectPixelArtName, TrKey::EffectPixelArtDesc
);

// ── ChromaKeyFilter (single-input, but has is_identity check) ─────

#[aviutl2::plugin(FilterPlugin)]
struct ChromaKeyFilter;

impl FilterPlugin for ChromaKeyFilter {
    fn new(_info: AviUtl2Info) -> Result<Self> {
        Ok(Self)
    }

    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: ja_tr(TrKey::EffectChromaKeyName),
            label: Some("zzzFX".into()),
            information: ja_tr(TrKey::EffectChromaKeyDesc),
            flags: aviutl2::bitflag!(FilterPluginFlags {
                video: true,
                filter: true,
            }),
            config_items: build_config_items::<zzzfx::ChromaKeyFullSettings>(),
        }
    }

    fn proc_video(
        &self,
        config: &[FilterConfigItem],
        video: &mut FilterProcVideo,
    ) -> Result<()> {
        let mut settings = zzzfx::ChromaKeyFullSettings::default();
        read_config(config, &mut settings);
        let effect: zzzfx::ChromaKey = settings.into();
        if effect.is_identity() {
            return Ok(());
        }
        let w = video.video_object.width as usize;
        let h = video.video_object.height as usize;
        if w == 0 || h == 0 {
            return Ok(());
        }
        let len = w * h * 4;
        let mut src = vec![0u8; len];
        let mut dst = vec![0u8; len];
        video.get_image_data(&mut src);
        effect.apply_effect(&src, &mut dst, w, h);
        video.set_image_data(&dst, video.video_object.width, video.video_object.height);
        Ok(())
    }
}

// ── AmbientLightFilter (dual-input: uses same frame as fg and bg) ─

#[aviutl2::plugin(FilterPlugin)]
struct AmbientLightFilter;

impl FilterPlugin for AmbientLightFilter {
    fn new(_info: AviUtl2Info) -> Result<Self> {
        Ok(Self)
    }

    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: ja_tr(TrKey::EffectAmbientLightName),
            label: Some("zzzFX".into()),
            information: ja_tr(TrKey::EffectAmbientLightDesc),
            flags: aviutl2::bitflag!(FilterPluginFlags {
                video: true,
                filter: true,
            }),
            config_items: build_config_items::<zzzfx::AmbientLightFullSettings>(),
        }
    }

    fn proc_video(
        &self,
        config: &[FilterConfigItem],
        video: &mut FilterProcVideo,
    ) -> Result<()> {
        let mut settings = zzzfx::AmbientLightFullSettings::default();
        read_config(config, &mut settings);
        let effect: zzzfx::AmbientLight = settings.into();
        if effect.is_identity() {
            return Ok(());
        }
        let w = video.video_object.width as usize;
        let h = video.video_object.height as usize;
        if w == 0 || h == 0 {
            return Ok(());
        }
        let len = w * h * 4;
        let mut fg = vec![0u8; len];
        let mut bg = vec![0u8; len];
        let mut dst = vec![0u8; len];
        video.get_image_data(&mut fg);
        // AviUtl2 processes a single frame — pass same as both foreground and background.
        bg.copy_from_slice(&fg);
        effect.apply_effect(&fg, &bg, &mut dst, w, h);
        video.set_image_data(&dst, video.video_object.width, video.video_object.height);
        Ok(())
    }
}

// ── Complex effects (generators — render full-frame content) ──────
// These don't use apply_effect; they have separate render functions
// that need file loading, caching, and time info.

#[aviutl2::plugin(FilterPlugin)]
struct RepeaterFilter;

impl FilterPlugin for RepeaterFilter {
    fn new(_info: AviUtl2Info) -> Result<Self> { Ok(Self) }
    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: ja_tr(TrKey::EffectRepeaterName),
            label: Some("zzzFX".into()),
            information: ja_tr(TrKey::EffectRepeaterDesc),
            flags: aviutl2::bitflag!(FilterPluginFlags { video: true, filter: true }),
            config_items: build_config_items::<zzzfx::RepeaterFullSettings>(),
        }
    }
    fn proc_video(&self, _config: &[FilterConfigItem], _video: &mut FilterProcVideo) -> Result<()> {
        // Repeater requires keyframe-based layer compositing with time info.
        // The AviUtl2 filter API does not provide keyframe iteration or compositing layers.
        // This effect is registered for parameter browsing; full functionality depends on
        // future aviutl2 crate API support for keyframe access.
        Ok(())
    }
}

#[aviutl2::plugin(FilterPlugin)]
struct SpriteSheetFilter;

impl FilterPlugin for SpriteSheetFilter {
    fn new(_info: AviUtl2Info) -> Result<Self> { Ok(Self) }
    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: ja_tr(TrKey::EffectSpritesheetName),
            label: Some("zzzFX".into()),
            information: ja_tr(TrKey::EffectSpritesheetDesc),
            flags: aviutl2::bitflag!(FilterPluginFlags { video: true, filter: true }),
            config_items: build_config_items::<zzzfx::SpriteSheetFullSettings>(),
        }
    }
    fn proc_video(&self, _config: &[FilterConfigItem], _video: &mut FilterProcVideo) -> Result<()> {
        // SpriteSheet requires image file loading, sprite decoding, and frame/time indexing.
        // File selection via AviUtl2 config items is not yet wired.
        Ok(())
    }
}

#[aviutl2::plugin(FilterPlugin)]
struct AssSubtitleFilter;

impl FilterPlugin for AssSubtitleFilter {
    fn new(_info: AviUtl2Info) -> Result<Self> { Ok(Self) }
    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: ja_tr(TrKey::EffectAssSubtitleName),
            label: Some("zzzFX".into()),
            information: ja_tr(TrKey::EffectAssSubtitleDesc),
            flags: aviutl2::bitflag!(FilterPluginFlags { video: true, filter: true }),
            config_items: build_config_items::<zzzfx::AssSubtitleFullSettings>(),
        }
    }
    fn proc_video(&self, _config: &[FilterConfigItem], _video: &mut FilterProcVideo) -> Result<()> {
        // ASS subtitle rendering requires file loading, parsing, and font caching.
        // File selection and time access are not yet wired for AviUtl2.
        Ok(())
    }
}

#[aviutl2::plugin(FilterPlugin)]
struct MidiDisplayFilter;

impl FilterPlugin for MidiDisplayFilter {
    fn new(_info: AviUtl2Info) -> Result<Self> { Ok(Self) }
    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: ja_tr(TrKey::EffectMidiDisplayName),
            label: Some("zzzFX".into()),
            information: ja_tr(TrKey::EffectMidiDisplayDesc),
            flags: aviutl2::bitflag!(FilterPluginFlags { video: true, filter: true }),
            config_items: build_config_items::<zzzfx::MidiDisplayFullSettings>(),
        }
    }
    fn proc_video(&self, _config: &[FilterConfigItem], _video: &mut FilterProcVideo) -> Result<()> {
        // MIDI visualization requires file loading, MIDI parsing, and time-based playback.
        Ok(())
    }
}

#[aviutl2::plugin(FilterPlugin)]
struct SvgDisplayFilter;

impl FilterPlugin for SvgDisplayFilter {
    fn new(_info: AviUtl2Info) -> Result<Self> { Ok(Self) }
    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: ja_tr(TrKey::EffectSvgDisplayName),
            label: Some("zzzFX".into()),
            information: ja_tr(TrKey::EffectSvgDisplayDesc),
            flags: aviutl2::bitflag!(FilterPluginFlags { video: true, filter: true }),
            config_items: build_config_items::<zzzfx::SvgDisplayFullSettings>(),
        }
    }
    fn proc_video(&self, _config: &[FilterConfigItem], _video: &mut FilterProcVideo) -> Result<()> {
        // SVG rendering requires file loading, parsing (resvg), and caching.
        Ok(())
    }
}

#[aviutl2::plugin(FilterPlugin)]
struct LaTeXDisplayFilter;

impl FilterPlugin for LaTeXDisplayFilter {
    fn new(_info: AviUtl2Info) -> Result<Self> { Ok(Self) }
    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: ja_tr(TrKey::EffectLaTeXDisplayName),
            label: Some("zzzFX".into()),
            information: ja_tr(TrKey::EffectLaTeXDisplayDesc),
            flags: aviutl2::bitflag!(FilterPluginFlags { video: true, filter: true }),
            config_items: build_config_items::<zzzfx::LaTeXDisplayFullSettings>(),
        }
    }
    fn proc_video(&self, _config: &[FilterConfigItem], _video: &mut FilterProcVideo) -> Result<()> {
        // LaTeX rendering requires formula parsing (ratex) and font loading.
        Ok(())
    }
}

#[aviutl2::plugin(FilterPlugin)]
struct QrCodeFilter;

impl FilterPlugin for QrCodeFilter {
    fn new(_info: AviUtl2Info) -> Result<Self> { Ok(Self) }
    fn plugin_info(&self) -> FilterPluginTable {
        FilterPluginTable {
            name: ja_tr(TrKey::EffectQrCodeName),
            label: Some("zzzFX".into()),
            information: ja_tr(TrKey::EffectQrCodeDesc),
            flags: aviutl2::bitflag!(FilterPluginFlags { video: true, filter: true }),
            config_items: build_config_items::<zzzfx::QrCodeFullSettings>(),
        }
    }
    fn proc_video(&self, _config: &[FilterConfigItem], _video: &mut FilterProcVideo) -> Result<()> {
        // QR code generation requires text input encoding (fast_qr).
        Ok(())
    }
}

// ── Generic config reader ─────────────────────────────────────────

fn read_config<T: Settings>(config: &[FilterConfigItem], settings: &mut T) {
    let descriptors = T::setting_descriptors();
    let mut idx = 0;
    read_descriptors(&descriptors, config, settings, &mut idx);
}

fn read_descriptors<T: Settings>(
    descriptors: &[zzzfx::settings::SettingDescriptor<T>],
    config: &[FilterConfigItem],
    settings: &mut T,
    idx: &mut usize,
) {
    use zzzfx::settings::{EnumValue, SettingKind};

    for desc in descriptors {
        match &desc.kind {
            SettingKind::FloatRange { .. } | SettingKind::Percentage { .. } => {
                if let Some(FilterConfigItem::Track(track)) = config.get(*idx) {
                    let _ = settings.set_field::<f32>(&desc.id, track.value as f32);
                }
                *idx += 1;
            }
            SettingKind::IntRange { .. } => {
                if let Some(FilterConfigItem::Track(track)) = config.get(*idx) {
                    let _ = settings.set_field::<i32>(&desc.id, track.value as i32);
                }
                *idx += 1;
            }
            SettingKind::Boolean => {
                if let Some(FilterConfigItem::Checkbox(check)) = config.get(*idx) {
                    let _ = settings.set_field::<bool>(&desc.id, check.value);
                }
                *idx += 1;
            }
            SettingKind::String { .. } => {
                match config.get(*idx) {
                    Some(FilterConfigItem::String(s)) => {
                        let _ = settings.set_field::<String>(&desc.id, s.value.clone());
                    }
                    Some(FilterConfigItem::Text(t)) => {
                        let _ = settings.set_field::<String>(&desc.id, t.value.clone());
                    }
                    _ => {}
                }
                *idx += 1;
            }
            SettingKind::PushButton { .. } => {
                // PushButton has no stored value
                *idx += 1;
            }
            SettingKind::Enumeration { .. } => {
                if let Some(FilterConfigItem::Select(select)) = config.get(*idx)
                    && let Some(item) = select.items.get(select.value as usize)
                {
                    let enum_val = EnumValue(item.value as u32);
                    let _ = settings.set_field::<EnumValue>(&desc.id, enum_val);
                }
                *idx += 1;
            }
            SettingKind::ColorRGBA { r_id, g_id, b_id, a_id } => {
                if let Some(FilterConfigItem::Color(color)) = config.get(*idx) {
                    let (r, g, b) = unpack_u32_to_rgb(color.value.0);
                    let _ = settings.set_field::<f32>(r_id, r);
                    let _ = settings.set_field::<f32>(g_id, g);
                    let _ = settings.set_field::<f32>(b_id, b);
                }
                *idx += 1;
                if let Some(FilterConfigItem::Track(track)) = config.get(*idx) {
                    let _ = settings.set_field::<f32>(a_id, track.value as f32);
                }
                *idx += 1;
            }
            SettingKind::ColorRGB { r_id, g_id, b_id } => {
                if let Some(FilterConfigItem::Color(color)) = config.get(*idx) {
                    let (r, g, b) = unpack_u32_to_rgb(color.value.0);
                    let _ = settings.set_field::<f32>(r_id, r);
                    let _ = settings.set_field::<f32>(g_id, g);
                    let _ = settings.set_field::<f32>(b_id, b);
                }
                *idx += 1;
            }
            SettingKind::Group { children } => {
                match config.get(*idx) {
                    Some(FilterConfigItem::Checkbox(check)) => {
                        let _ = settings.set_field::<bool>(&desc.id, check.value);
                    }
                    Some(FilterConfigItem::CheckSection(check)) => {
                        let _ = settings.set_field::<bool>(&desc.id, check.value);
                    }
                    _ => {}
                }
                *idx += 1;

                if let Some(FilterConfigItem::Group(_)) = config.get(*idx) {
                    *idx += 1;
                }

                read_descriptors(children, config, settings, idx);

                if let Some(FilterConfigItem::Group(g)) = config.get(*idx)
                    && g.name.is_none()
                {
                    *idx += 1;
                }
            }
        }
    }
}

fn unpack_u32_to_rgb(packed: u32) -> (f32, f32, f32) {
    let r = ((packed >> 16) & 0xFF) as f32 / 255.0;
    let g = ((packed >> 8) & 0xFF) as f32 / 255.0;
    let b = (packed & 0xFF) as f32 / 255.0;
    (r, g, b)
}

aviutl2::register_generic_plugin!(ZzzFxPlugin);
