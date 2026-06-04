use std::ffi::OsStr;

#[rustfmt::skip]
fn main() {
    let is_ae = std::env::var_os("CARGO_CFG_WINDOWS").is_some()
        || std::env::var_os("CARGO_CFG_TARGET_OS").as_deref() == Some(OsStr::new("macos"));
    if !is_ae {
        return;
    }

    const PF_PLUG_IN_VERSION: u16 = 13;
    const PF_PLUG_IN_SUBVERS: u16 = 28;
    const EFFECT_VERSION_MAJOR: u32 = 0;
    const EFFECT_VERSION_MINOR: u32 = 1;
    const EFFECT_VERSION_PATCH: u32 = 0;

    use pipl::*;

    // ── Primary effect (Stroke) — standard pipl::plugin_build() ──────────
    pipl::plugin_build(vec![
        Property::Kind(PIPLType::AEEffect),
        Property::Name("zzzFX Stroke"),
        Property::Category("zzzFX"),

        #[cfg(target_os = "windows")]
        Property::CodeWin64X86("EffectMainStroke"),
        #[cfg(target_os = "macos")]
        Property::CodeMacIntel64("EffectMainStroke"),
        #[cfg(target_os = "macos")]
        Property::CodeMacARM64("EffectMainStroke"),

        Property::AE_PiPL_Version { major: 2, minor: 0 },
        Property::AE_Effect_Spec_Version { major: PF_PLUG_IN_VERSION, minor: PF_PLUG_IN_SUBVERS },
        Property::AE_Effect_Version {
            version: EFFECT_VERSION_MAJOR,
            subversion: EFFECT_VERSION_MINOR,
            bugversion: EFFECT_VERSION_PATCH,
            stage: Stage::Develop,
            build: 1,
        },
        Property::AE_Effect_Info_Flags(0),
        Property::AE_Effect_Global_OutFlags(
            OutFlags::NonParamVary |
            OutFlags::DeepColorAware |
            OutFlags::SendUpdateParamsUI |
            OutFlags::PiplOverridesOutdataOutflags
        ),
        Property::AE_Effect_Global_OutFlags_2(
            OutFlags2::ParamGroupStartCollapsedFlag |
            OutFlags2::SupportsSmartRender |
            OutFlags2::FloatColorAware |
            OutFlags2::RevealsZeroAlpha |
            OutFlags2::SupportsThreadedRendering |
            OutFlags2::SupportsGetFlattenedSequenceData
        ),
        Property::AE_Effect_Match_Name("zzzfx-stroke"),
        Property::AE_Reserved_Info(8),
        Property::AE_Effect_Support_URL("https://github.com/zzzEffect/zzzFX"),
    ]);

    // ── Additional effects — PiPL resources at IDs 16001+ ─────────────────
    // Using append_rc_content() + compile() — same mechanism as plugin_build.
    #[cfg(target_os = "windows")]
    {
        fn to_seq(bytes: &[u8]) -> String {
            bytes.iter().fold(String::new(), |mut s, b| {
                s.push_str(&format!("\\x{b:02x}"));
                s
            })
        }

        let effects: &[(&'static str, &'static str, &'static str)] = &[
            ("zzzFX Stroke",              "zzzfx-stroke",          "EffectMainStroke"),
            ("zzzFX Repeater",            "zzzfx-repeater",       "EffectMainRepeater"),
            ("zzzFX Sprite Sheet",        "zzzfx-sprite-sheet",   "EffectMainSpriteSheet"),
            ("zzzFX ASCII Art Style",     "zzzfx-ascii-art",      "EffectMainAsciiArt"),
            ("zzzFX Pixel Art Style",     "zzzfx-pixel-art",      "EffectMainPixelArt"),
            ("zzzFX Ambient Light Fusion","zzzfx-ambient-light",   "EffectMainAmbientLight"),
            ("zzzFX Long Shadow",         "zzzfx-long-shadow",     "EffectMainLongShadow"),
            ("zzzFX Cast Shadow",         "zzzfx-cast-shadow",     "EffectMainCastShadow"),
            ("zzzFX Chroma Key",          "zzzfx-chroma-key",      "EffectMainChromaKey"),
            ("zzzFX MIDI Display",        "zzzfx-midi-display",    "EffectMainMidiDisplay"),
            ("zzzFX SVG Display",         "zzzfx-svg-display",     "EffectMainSvgDisplay"),
            ("zzzFX LaTeX Display",       "zzzfx-latex-display",   "EffectMainLaTeXDisplay"),
            ("zzzFX QR Code",             "zzzfx-qr-code",         "EffectMainQrCode"),
            ("zzzFX ASS Subtitle",        "zzzfx-ass-subtitle",    "EffectMainAssSubtitle"),
        ];

        let mut all_pipls = Vec::new();
        for (idx, (name, match_name, entry_name)) in effects.iter().enumerate() {
            let (name, match_name, entry_name) = (*name, *match_name, *entry_name);
            let pipl = build_pipl(vec![
                Property::Kind(PIPLType::AEEffect),
                Property::Name(name),
                Property::Category("zzzFX"),
                Property::CodeWin64X86(entry_name),
                Property::AE_PiPL_Version { major: 2, minor: 0 },
                Property::AE_Effect_Spec_Version { major: PF_PLUG_IN_VERSION, minor: PF_PLUG_IN_SUBVERS },
                Property::AE_Effect_Version {
                    version: EFFECT_VERSION_MAJOR,
                    subversion: EFFECT_VERSION_MINOR,
                    bugversion: EFFECT_VERSION_PATCH,
                    stage: Stage::Develop, build: 1,
                },
                Property::AE_Effect_Info_Flags(0),
                Property::AE_Effect_Global_OutFlags(
                    OutFlags::NonParamVary | OutFlags::DeepColorAware |
                    OutFlags::SendUpdateParamsUI | OutFlags::PiplOverridesOutdataOutflags
                ),
                Property::AE_Effect_Global_OutFlags_2(
                    OutFlags2::ParamGroupStartCollapsedFlag | OutFlags2::SupportsSmartRender |
                    OutFlags2::FloatColorAware | OutFlags2::RevealsZeroAlpha |
                    OutFlags2::SupportsThreadedRendering | OutFlags2::SupportsGetFlattenedSequenceData
                ),
                Property::AE_Effect_Match_Name(match_name),
                Property::AE_Reserved_Info(8),
                Property::AE_Effect_Support_URL("https://example.com/plugin"),
            ]).unwrap();
            all_pipls.push((16000 + idx as i16, pipl));
        }

        let mut res = winres::WindowsResource::new();
        for (id, pipl) in &all_pipls {
            res.append_rc_content(&format!(
                "{} PiPL DISCARDABLE BEGIN \"{}\" END\n",
                id,
                to_seq(pipl)
            ));
        }
        res.compile().unwrap();
    }

    println!("cargo:rustc-env=EFFECT_VERSION_MAJOR={EFFECT_VERSION_MAJOR}");
    println!("cargo:rustc-env=EFFECT_VERSION_MINOR={EFFECT_VERSION_MINOR}");
    println!("cargo:rustc-env=EFFECT_VERSION_PATCH={EFFECT_VERSION_PATCH}");
    println!("cargo:rustc-cfg=with_premiere");
    println!("cargo:rustc-cfg=catch_panics");
}
