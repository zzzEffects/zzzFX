//! Translation template for zzzFX — copy this file to bootstrap a new language.
//!
//! This template covers **only `TrKey`** (zzzFX keys).  For Example Effect
//! translations see `example-effect/src/i18n/lang_template.rs`.
//!
//! # How to add a new language (e.g. Japanese `ja`)
//!
//! **Step 1 — Copy this template**
//!
//! ```bash
//! cp crates/zzzfx-core/src/i18n/lang_template.rs \
//!    crates/zzzfx-core/src/i18n/ja.rs
//! ```
//!
//! **Step 2 — Translate every match arm**
//!
//! Replace each `c"..."` English placeholder with translated text.  The
//! compiler enforces exhaustiveness — every `TrKey` variant must have a
//! value, so you cannot accidentally skip a key.
//!
//! Only `translate_cstr()` is needed; `tr()` and `tr_cstr()` in `mod.rs`
//! both derive from it — no need to duplicate the match.
//!
//! Run `cargo check -p zzzfx-core` frequently while translating.
//!
//! **Step 3 — Register the language in `mod.rs`**
//!
//! 1. Add `mod ja;` next to `mod zh_cn;`.
//! 2. Add `Ja = 2` to the `Lang` enum.
//! 3. In `tr()`, add:
//!    ```rust
//!    Lang::Ja => ja::translate_cstr(key).to_str().unwrap(),
//!    ```
//! 4. In `tr_cstr()`, add:
//!    ```rust
//!    Lang::Ja => ja::translate_cstr(key),
//!    ```
//! 5. Extend `detect_system_lang()` to detect the new locale.
//!
//! **Step 4 — Run the verification tests**
//!
//! Add your language to `crates/zzzfx-core/tests/i18n_tests.rs`:
//!
//! ```rust
//! #[test]
//! fn all_keys_have_non_empty_ja() {
//!     i18n::set_lang(i18n::Lang::Ja);
//!     for key in ALL {
//!         assert!(!i18n::tr(*key).is_empty(), "Empty ja for {:?}", key);
//!     }
//! }
//! ```
//!
//! # Key naming conventions
//!
//! | Prefix | Meaning |
//! |--------|---------|
//! | `Effect*Name` / `Effect*Desc` | Plugin name / description shown in host |
//! | `Native*` | Label/hint for native OFX parameters (RGBA, Double2D, etc.) |
//! | `Param*` | Label for generic settings; `*Desc` suffix = hint/tooltip |
//! | `Menu*` | Dropdown menu item label; `*Desc` suffix = item description |
//! | `Common*` | Shared strings (e.g. "Enabled") |

use std::ffi::CStr;
use effect_settings::TrKey;

/// Return a null-terminated `&'static CStr` for every translation key.
///
/// This is the **only** function a language module must provide.
/// `zzzfx_core::i18n::tr()` automatically derives `&str` from this via
/// `.to_str().unwrap()` — no need to write a second match.
///
/// Replace every `c"..."` placeholder with the translated text.
pub fn translate_cstr(key: TrKey) -> &'static CStr {
    match key {
        // ── Effect labels ─────────────────────────────────
        TrKey::EffectStrokeName => c"zzzFX Stroke",
        TrKey::EffectRepeaterName => c"zzzFX Repeater",
        TrKey::EffectSpritesheetName => c"zzzFX Sprite Sheet",

        // ── Effect descriptions ───────────────────────────
        TrKey::EffectStrokeDesc => c"Alpha-channel stroke effect with distance transform, multiple fill modes, and blend options.",
        TrKey::EffectRepeaterDesc => c"Keyframe-driven repeater that composites multiple time-offset layers with configurable position, rotation, and blending.",
        TrKey::EffectSpritesheetDesc => c"Sprite sheet reader that decodes grid-based sprite sheets with animation, scaling, and playback controls.",

        // ── Stroke: native param labels & hints ───────────
        TrKey::NativeStrokeColor => c"Stroke Color",
        TrKey::NativeStrokeColorHint => c"Color of the stroke.",
        TrKey::NativeGradientStart => c"Gradient Start",
        TrKey::NativeGradientStartHint => c"Normalized start position (0-1).",
        TrKey::NativeGradientStartColor => c"Gradient Start Color",
        TrKey::NativeGradientStartColorHint => c"Color at gradient start.",
        TrKey::NativeGradientEnd => c"Gradient End",
        TrKey::NativeGradientEndHint => c"Normalized end position (0-1).",
        TrKey::NativeGradientEndColor => c"Gradient End Color",
        TrKey::NativeGradientEndColorHint => c"Color at gradient end.",

        // ── Repeater: native param labels & hints ─────────
        TrKey::NativePosition => c"Position",
        TrKey::NativePositionHint => c"Position of the repeat layer (0-1 normalized).",

        // ── Sprite Sheet: native param labels & hints ─────
        TrKey::NativeSelectSpriteSheet => c"Select Sprite Sheet...",
        TrKey::NativeSelectSpriteSheetHint => c"Choose a sprite sheet image file.",
        TrKey::NativeSpriteRange => c"Sprite Range",
        TrKey::NativeSpriteRangeHint => c"Index of the first and last sprite in the animation.",
        TrKey::NativeRepeatRange => c"Repeat Range",
        TrKey::NativeRepeatRangeHint => c"Delimit a range within which sprites will be repeated.",
        TrKey::NativeSpritesCut => c"Sprites Cut",
        TrKey::NativeSpritesCutHint => c"Cut the sprite sheet and read it separately.",
        TrKey::NativeFilePath => c"File Path",
        TrKey::NativeControls => c"Controls",

        // ── Common ────────────────────────────────────────
        TrKey::CommonEnabled => c"Enabled",

        // ── Stroke: generic param labels ──────────────────
        TrKey::ParamStrokePosition => c"Stroke Position",
        TrKey::ParamStrokePositionDesc => c"Where the stroke is drawn relative to the alpha boundary.",
        TrKey::ParamFillMode => c"Fill Mode",
        TrKey::ParamFillModeDesc => c"How the stroke color is determined.",
        TrKey::ParamStrokeWidth => c"Stroke Width",
        TrKey::ParamStrokeWidthDesc => c"Width of the stroke, normalized to the larger image dimension.",
        TrKey::ParamStrokeColorRed => c"Stroke Color Red",
        TrKey::ParamStrokeColorRedDesc => c"Red component of the stroke color.",
        TrKey::ParamStrokeColorGreen => c"Stroke Color Green",
        TrKey::ParamStrokeColorGreenDesc => c"Green component of the stroke color.",
        TrKey::ParamStrokeColorBlue => c"Stroke Color Blue",
        TrKey::ParamStrokeColorBlueDesc => c"Blue component of the stroke color.",
        TrKey::ParamStrokeColorAlpha => c"Stroke Color Alpha",
        TrKey::ParamStrokeColorAlphaDesc => c"Alpha component of the stroke color.",
        TrKey::ParamAlphaThreshold => c"Alpha Threshold",
        TrKey::ParamAlphaThresholdDesc => c"Alpha value above which pixels are considered inside the shape.",
        TrKey::ParamEdgeBlend => c"Edge Blend",
        TrKey::ParamEdgeBlendDesc => c"Controls how the source edges blend with the stroke. 0 = hard edges, 1 = full blend.",
        TrKey::ParamStrokeFeathering => c"Stroke Feathering",
        TrKey::ParamStrokeFeatheringDesc => c"Softens the stroke edges. Higher values produce softer transitions.",
        TrKey::ParamSourceOpacity => c"Source Opacity",
        TrKey::ParamSourceOpacityDesc => c"Opacity of the source image. 0 = fully transparent, 1 = fully opaque.",
        TrKey::ParamBlendMode => c"Blend Mode",
        TrKey::ParamBlendModeDesc => c"How the stroke is composited with the source image.",
        TrKey::ParamGradientSettings => c"Gradient Settings",
        TrKey::ParamGradientSettingsDesc => c"Gradient parameters used when Fill Mode is set to a gradient option.",
        TrKey::ParamStartPointX => c"Start Point X",
        TrKey::ParamStartPointXDesc => c"X coordinate of the gradient start point.",
        TrKey::ParamStartPointY => c"Start Point Y",
        TrKey::ParamStartPointYDesc => c"Y coordinate of the gradient start point.",
        TrKey::ParamStartColorRed => c"Start Color Red",
        TrKey::ParamStartColorRedDesc => c"Red component of the gradient start color.",
        TrKey::ParamStartColorGreen => c"Start Color Green",
        TrKey::ParamStartColorGreenDesc => c"Green component of the gradient start color.",
        TrKey::ParamStartColorBlue => c"Start Color Blue",
        TrKey::ParamStartColorBlueDesc => c"Blue component of the gradient start color.",
        TrKey::ParamStartColorAlpha => c"Start Color Alpha",
        TrKey::ParamStartColorAlphaDesc => c"Alpha component of the gradient start color.",
        TrKey::ParamEndPointX => c"End Point X",
        TrKey::ParamEndPointXDesc => c"X coordinate of the gradient end point.",
        TrKey::ParamEndPointY => c"End Point Y",
        TrKey::ParamEndPointYDesc => c"Y coordinate of the gradient end point.",
        TrKey::ParamEndColorRed => c"End Color Red",
        TrKey::ParamEndColorRedDesc => c"Red component of the gradient end color.",
        TrKey::ParamEndColorGreen => c"End Color Green",
        TrKey::ParamEndColorGreenDesc => c"Green component of the gradient end color.",
        TrKey::ParamEndColorBlue => c"End Color Blue",
        TrKey::ParamEndColorBlueDesc => c"Blue component of the gradient end color.",
        TrKey::ParamEndColorAlpha => c"End Color Alpha",
        TrKey::ParamEndColorAlphaDesc => c"Alpha component of the gradient end color.",
        TrKey::ParamUseSharpCorners => c"Use Sharp Corners",
        TrKey::ParamUseSharpCornersDesc => c"When enabled, stroke corners are sharp (square) instead of rounded.",

        // ── Stroke menu item labels & descriptions ───────
        TrKey::MenuStrokeOuter => c"Outer",
        TrKey::MenuStrokeOuterDesc => c"Stroke is drawn outside the shape.",
        TrKey::MenuStrokeInner => c"Inner",
        TrKey::MenuStrokeInnerDesc => c"Stroke is drawn inside the shape.",
        TrKey::MenuStrokeCenter => c"Center",
        TrKey::MenuStrokeCenterDesc => c"Stroke is centered on the alpha boundary.",
        TrKey::MenuSolidColor => c"Solid Color",
        TrKey::MenuSolidColorDesc => c"Uniform stroke color.",
        TrKey::MenuDistanceGradient => c"Distance Gradient",
        TrKey::MenuDistanceGradientDesc => c"Gradient based on distance from start point.",
        TrKey::MenuGradient => c"Gradient",
        TrKey::MenuGradientDesc => c"Linear gradient from start to end point.",
        TrKey::MenuSourceColorExtension => c"Source Color Extension",
        TrKey::MenuSourceColorExtensionDesc => c"Stroke uses the color of the nearest edge pixel.",

        // ── Blend mode menu item labels (22 modes) ───────
        TrKey::MenuNormal => c"Normal",
        TrKey::MenuNormalDesc => c"Standard alpha blending.",
        TrKey::MenuDissolve => c"Dissolve",
        TrKey::MenuDissolveDesc => c"Random dithering based on alpha.",
        TrKey::MenuDarken => c"Darken",
        TrKey::MenuDarkenDesc => c"Keeps the darker of stroke and source.",
        TrKey::MenuMultiply => c"Multiply",
        TrKey::MenuMultiplyDesc => c"Multiplies stroke and source.",
        TrKey::MenuColorBurn => c"Color Burn",
        TrKey::MenuColorBurnDesc => c"Darkens source to reflect stroke.",
        TrKey::MenuLinearBurn => c"Linear Burn",
        TrKey::MenuLinearBurnDesc => c"Linear darkening of source.",
        TrKey::MenuAdd => c"Add",
        TrKey::MenuAddDesc => c"Adds stroke and source values.",
        TrKey::MenuScreen => c"Screen",
        TrKey::MenuScreenDesc => c"Inverse multiply, lightens.",
        TrKey::MenuColorDodge => c"Color Dodge",
        TrKey::MenuColorDodgeDesc => c"Brightens source to reflect stroke.",
        TrKey::MenuLinearDodge => c"Linear Dodge",
        TrKey::MenuLinearDodgeDesc => c"Linear brightening (same as Add).",
        TrKey::MenuOverlay => c"Overlay",
        TrKey::MenuOverlayDesc => c"Combines Multiply and Screen.",
        TrKey::MenuSoftLight => c"Soft Light",
        TrKey::MenuSoftLightDesc => c"Subtle contrast blend.",
        TrKey::MenuLinearLight => c"Linear Light",
        TrKey::MenuLinearLightDesc => c"Linear contrast blend.",
        TrKey::MenuHardMix => c"Hard Mix",
        TrKey::MenuHardMixDesc => c"High-contrast threshold blend.",
        TrKey::MenuDifference => c"Difference",
        TrKey::MenuDifferenceDesc => c"Absolute difference between stroke and source.",
        TrKey::MenuExclusion => c"Exclusion",
        TrKey::MenuExclusionDesc => c"Lower-contrast difference.",
        TrKey::MenuSubtract => c"Subtract",
        TrKey::MenuSubtractDesc => c"Subtracts stroke from source.",
        TrKey::MenuDivide => c"Divide",
        TrKey::MenuDivideDesc => c"Divides source by stroke.",
        TrKey::MenuStencilAlpha => c"Stencil Alpha",
        TrKey::MenuStencilAlphaDesc => c"Uses stroke alpha as a stencil.",
        TrKey::MenuStencilLuma => c"Stencil Luma",
        TrKey::MenuStencilLumaDesc => c"Uses stroke luminance as a stencil.",
        TrKey::MenuOutlineAlpha => c"Outline Alpha",
        TrKey::MenuOutlineAlphaDesc => c"Replaces image with stroke, preserving alpha.",
        TrKey::MenuOutlineLuma => c"Outline Luma",
        TrKey::MenuOutlineLumaDesc => c"Replaces image with stroke, using luminescence.",

        // ── Repeater: generic param labels ────────────────
        TrKey::ParamTimeOffset => c"Time Offset",
        TrKey::ParamTimeOffsetDesc => c"Time offset in seconds. Keyframes on this parameter trigger repeat layers. Output time = max(0, currentTime - value).",
        TrKey::ParamPositionX => c"Position X",
        TrKey::ParamPositionXDesc => c"X coordinate of the repeat layer position (0 = left, 1 = right).",
        TrKey::ParamPositionY => c"Position Y",
        TrKey::ParamPositionYDesc => c"Y coordinate of the repeat layer position (0 = top, 1 = bottom).",
        TrKey::ParamRotation => c"Rotation",
        TrKey::ParamRotationDesc => c"Rotation of the repeat layer in degrees around the image center.",
        TrKey::ParamLayerOrder => c"Layer Order",
        TrKey::ParamLayerOrderDesc => c"Whether new repeat layers appear above or below existing content.",
        TrKey::ParamMaxLayers => c"Max Layers",
        TrKey::ParamMaxLayersDesc => c"Maximum number of layers (including the original). 0 = unlimited.",
        TrKey::ParamRepeaterBlendMode => c"Blend Mode",
        TrKey::ParamRepeaterBlendModeDesc => c"How repeat layers are composited with each other.",

        // ── Repeater menu items ───────────────────────────
        TrKey::MenuAbove => c"Above",
        TrKey::MenuAboveDesc => c"New repeat layers are composited on top of existing content.",
        TrKey::MenuBelow => c"Below",
        TrKey::MenuBelowDesc => c"New repeat layers are composited beneath existing content.",

        // ── Repeater blend mode descriptions ──────────────
        TrKey::MenuRepeaterDarkenDesc => c"Keeps the darker of the two layers.",
        TrKey::MenuRepeaterMultiplyDesc => c"Multiplies the two layers.",
        TrKey::MenuRepeaterColorBurnDesc => c"Darkens base to reflect blend layer.",
        TrKey::MenuRepeaterLinearBurnDesc => c"Linear darkening of base.",
        TrKey::MenuRepeaterAddDesc => c"Adds layer values together.",
        TrKey::MenuRepeaterScreenDesc => c"Inverse multiply, lightens.",
        TrKey::MenuRepeaterColorDodgeDesc => c"Brightens base to reflect blend layer.",
        TrKey::MenuRepeaterLinearDodgeDesc => c"Linear brightening (same as Add).",
        TrKey::MenuRepeaterOverlayDesc => c"Combines Multiply and Screen.",
        TrKey::MenuRepeaterSoftLightDesc => c"Subtle contrast blend.",
        TrKey::MenuRepeaterLinearLightDesc => c"Linear contrast blend.",
        TrKey::MenuRepeaterHardMixDesc => c"High-contrast threshold blend.",
        TrKey::MenuRepeaterDifferenceDesc => c"Absolute difference between layers.",
        TrKey::MenuRepeaterExclusionDesc => c"Lower-contrast difference.",
        TrKey::MenuRepeaterSubtractDesc => c"Subtracts blend layer from base.",
        TrKey::MenuRepeaterDivideDesc => c"Divides base by blend layer.",
        TrKey::MenuRepeaterStencilAlphaDesc => c"Uses layer alpha as a stencil.",
        TrKey::MenuRepeaterStencilLumaDesc => c"Uses layer luminance as a stencil.",
        TrKey::MenuRepeaterOutlineAlphaDesc => c"Replaces image with layer, preserving alpha.",
        TrKey::MenuRepeaterOutlineLumaDesc => c"Replaces image with layer, using luminescence.",

        // ── Sprite Sheet: generic param labels ────────────
        TrKey::ParamColumns => c"Columns",
        TrKey::ParamColumnsDesc => c"Number of sprite columns in the sheet. Sprite width = sheet width / columns.",
        TrKey::ParamRows => c"Rows",
        TrKey::ParamRowsDesc => c"Number of sprite rows in the sheet. Sprite height = sheet height / rows.",
        TrKey::ParamSpriteRangeStart => c"Sprite Range Start",
        TrKey::ParamSpriteRangeStartDesc => c"Index of the first sprite in the animation.",
        TrKey::ParamSpriteRangeEnd => c"Sprite Range End",
        TrKey::ParamSpriteRangeEndDesc => c"Index of the last sprite in the animation.",
        TrKey::ParamFrameOffset => c"Frame Offset",
        TrKey::ParamFrameOffsetDesc => c"Frame offset (floored). Shift the animation by partial or whole frames.",
        TrKey::ParamSpritePlayCount => c"Play Count",
        TrKey::ParamSpritePlayCountDesc => c"Number of times to play the animation. 0 = infinite. Negative = auto-compute speed to fit within duration.",
        TrKey::ParamSpeed => c"Speed",
        TrKey::ParamSpeedDesc => c"Playback speed. 0 = paused.",
        TrKey::ParamReadingDirection => c"Reading Direction",
        TrKey::ParamReadingDirectionDesc => c"The reading direction of the sprites.",
        TrKey::ParamPlaybackMode => c"Playback Mode",
        TrKey::ParamPlaybackModeDesc => c"The playback mode for the sprite animation.",
        TrKey::ParamLoopOffset => c"Loop Offset",
        TrKey::ParamLoopOffsetDesc => c"Frame offset for the first sprite in a single loop.",
        TrKey::ParamRepeatRangeStart => c"Repeat Range Start",
        TrKey::ParamRepeatRangeStartDesc => c"First sprite index in the repeat sub-range.",
        TrKey::ParamRepeatRangeEnd => c"Repeat Range End",
        TrKey::ParamRepeatRangeEndDesc => c"Last sprite index in the repeat sub-range.",
        TrKey::ParamRepeatCount => c"Repeat Count",
        TrKey::ParamRepeatCountDesc => c"How many times to repeat the repeat range (0 = no repeat).",
        TrKey::ParamSpritesCutX => c"Sprites Cut X",
        TrKey::ParamSpritesCutXDesc => c"Number of horizontal cut blocks in the sprite sheet.",
        TrKey::ParamSpritesCutY => c"Sprites Cut Y",
        TrKey::ParamSpritesCutYDesc => c"Number of vertical cut blocks in the sprite sheet.",
        TrKey::ParamScale => c"Scale",
        TrKey::ParamScaleDesc => c"Scale factor applied to the output sprite (1.0 = original size).",
        TrKey::ParamScaleAlgorithm => c"Scale Algorithm",
        TrKey::ParamScaleAlgorithmDesc => c"Resampling algorithm used when scaling.",
        TrKey::ParamSpriteDisplacementX => c"Displacement X",
        TrKey::ParamSpriteDisplacementXDesc => c"Horizontal pixel offset applied after scaling and centering.",
        TrKey::ParamSpriteDisplacementY => c"Displacement Y",
        TrKey::ParamSpriteDisplacementYDesc => c"Vertical pixel offset applied after scaling and centering.",
        TrKey::ParamSpriteDisplacement => c"Displacement",
        TrKey::ParamSpriteDisplacementDesc => c"Normalized position offset (0-1, default 0.5 = center).",
        TrKey::ParamSpriteRotation => c"Rotation",
        TrKey::ParamSpriteRotationDesc => c"Rotation angle in degrees around the image center.",
        TrKey::ParamSpriteDisplacementPixelBased => c"Pixel-Based Displacement",
        TrKey::ParamSpriteDisplacementPixelBasedDesc => c"Quantize displacement to the scaled pixel grid for pixel-art movement.",
        TrKey::ParamSpriteRotationPixelBased => c"Pixel-Based Rotation",
        TrKey::ParamSpriteRotationPixelBasedDesc => c"Use RotSprite pixel-art rotation preserving sharp edges.",
        TrKey::ParamSpriteSelectionMode => c"Selection Mode",
        TrKey::ParamSpriteSelectionModeDesc => c"Interactive sprite selection with grid overlay and frame picking.",
        TrKey::ParamSpriteFitToOutput => c"Fit Sprite Sheet to Output",
        TrKey::ParamSpriteFitToOutputDesc => c"Scale the full sprite sheet to fit within output bounds while preserving aspect ratio.",
        TrKey::ParamSpriteGridOverlayOpacity => c"Grid Overlay Opacity",
        TrKey::ParamSpriteGridOverlayOpacityDesc => c"Opacity of the grid overlay and frame numbers in selection mode. 0 = hidden, 1 = fully opaque.",

        // ── Sprite sheet menu items ───────────────────────
        TrKey::MenuHForward => c"H. Forward",
        TrKey::MenuHForwardDesc => c"Read sprites horizontally, left to right.",
        TrKey::MenuHBackward => c"H. Backward",
        TrKey::MenuHBackwardDesc => c"Read sprites horizontally, right to left.",
        TrKey::MenuVForward => c"V. Forward",
        TrKey::MenuVForwardDesc => c"Read sprites vertically, top to bottom.",
        TrKey::MenuVBackward => c"V. Backward",
        TrKey::MenuVBackwardDesc => c"Read sprites vertically, bottom to top.",
        TrKey::MenuHForwardS => c"H. Forward (S)",
        TrKey::MenuHForwardSDesc => c"Read sprites horizontally in S-shape.",
        TrKey::MenuHBackwardS => c"H. Backward (S)",
        TrKey::MenuHBackwardSDesc => c"Read sprites horizontally backward in S-shape.",
        TrKey::MenuVForwardS => c"V. Forward (S)",
        TrKey::MenuVForwardSDesc => c"Read sprites vertically in S-shape.",
        TrKey::MenuVBackwardS => c"V. Backward (S)",
        TrKey::MenuVBackwardSDesc => c"Read sprites vertically backward in S-shape.",
        TrKey::MenuNormalReverse => c"Normal & Reverse",
        TrKey::MenuNormalReverseDesc => c"Play forward then backward.",
        TrKey::MenuNormalReverseMerge => c"N.&R. (Merge)",
        TrKey::MenuNormalReverseMergeDesc => c"Play forward then backward, merging repeated first/last frames.",
        TrKey::MenuNearestNeighbor => c"Nearest Neighbor",
        TrKey::MenuNearestNeighborDesc => c"Fastest, no interpolation.",
        TrKey::MenuTriangle => c"Triangle",
        TrKey::MenuTriangleDesc => c"Bilinear interpolation.",
        TrKey::MenuCatmullRom => c"Catmull-Rom",
        TrKey::MenuCatmullRomDesc => c"Cubic filter, sharp results.",
        TrKey::MenuGaussian => c"Gaussian",
        TrKey::MenuGaussianDesc => c"Gaussian blur filter.",
        TrKey::MenuLanczos3 => c"Lanczos3",
        TrKey::MenuLanczos3Desc => c"Highest quality, 3-lobe Lanczos.",
        TrKey::MenuPlaybackNormalDesc => c"Play sprites in normal order.",
    }
}
