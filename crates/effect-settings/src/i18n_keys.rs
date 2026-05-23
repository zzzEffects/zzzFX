//! Translation key enums — single source of truth for all translatable strings.
//!
//! The `i18n_keys!` macro is invoked once per product:
//! - `TrKey` — keys for the zzzFX plugin family
//! - `ExTrKey` — keys for the Example Effect plugin family
//!
//! Each generated enum implements [`I18nKey`], which provides `.en()` (English)
//! and `.en_cstr()` (null-terminated for FFI).  Consumer crates use the
//! [`Settings`](crate::Settings) trait's associated `Key` type to select which
//! key enum their descriptors use.

use std::ffi::CStr;

/// Trait implemented by all i18n key enums generated via `i18n_keys!`.
pub trait I18nKey: Copy + std::fmt::Debug {
    /// English (canonical) string for this key.
    fn en(self) -> &'static str;

    /// English string as a null-terminated `&CStr` for FFI.
    fn en_cstr(self) -> &'static CStr;
}

#[macro_export]
macro_rules! i18n_keys {
    // ── Public entry: choose the target enum name ───────────────────────
    ($vis:vis $EnumName:ident { $($(#[$comment:meta])* $variant:ident = $en:literal;)* }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $EnumName {
            $($variant,)*
        }

        impl $EnumName {
            /// English (canonical) string for this key.
            pub fn en(self) -> &'static str {
                match self {
                    $($EnumName::$variant => $en,)*
                }
            }

            /// English string as a null-terminated `&CStr` for FFI.
            pub fn en_cstr(self) -> &'static CStr {
                match self {
                    $(
                        $EnumName::$variant => {
                            let bytes: &[u8] = concat!($en, "\0").as_bytes();
                            unsafe { CStr::from_bytes_with_nul_unchecked(bytes) }
                        }
                    ),*
                }
            }

            /// All variants (for iteration / tests).
            pub fn all() -> &'static [$EnumName] {
                &[$($EnumName::$variant,)*]
            }
        }

        impl $crate::i18n_keys::I18nKey for $EnumName {
            fn en(self) -> &'static str { self.en() }
            fn en_cstr(self) -> &'static CStr { self.en_cstr() }
        }
    };
}

// ── zzzFX keys ─────────────────────────────────────────────────────────────

i18n_keys! {
    pub TrKey {
        // Effect labels
        EffectStrokeName = "zzzFX Stroke";
        EffectRepeaterName = "zzzFX Repeater";
        EffectSpritesheetName = "zzzFX Sprite Sheet";

        // Effect descriptions
        EffectStrokeDesc = "Alpha-channel stroke effect with distance transform, multiple fill modes, and blend options.";
        EffectRepeaterDesc = "Keyframe-driven repeater that composites multiple time-offset layers with configurable position, rotation, and blending.";
        EffectSpritesheetDesc = "Sprite sheet reader that decodes grid-based sprite sheets with animation, scaling, and playback controls.";

        // Stroke: native param labels & hints
        NativeStrokeColor = "Stroke Color";
        NativeStrokeColorHint = "Color of the stroke.";
        NativeGradientStart = "Gradient Start";
        NativeGradientStartHint = "Normalized start position (0-1).";
        NativeGradientStartColor = "Gradient Start Color";
        NativeGradientStartColorHint = "Color at gradient start.";
        NativeGradientEnd = "Gradient End";
        NativeGradientEndHint = "Normalized end position (0-1).";
        NativeGradientEndColor = "Gradient End Color";
        NativeGradientEndColorHint = "Color at gradient end.";

        // Repeater: native param labels & hints
        NativePosition = "Position";
        NativePositionHint = "Position of the repeat layer (0-1 normalized).";

        // Sprite Sheet: native param labels & hints
        NativeSelectSpriteSheet = "Select Sprite Sheet...";
        NativeSelectSpriteSheetHint = "Choose a sprite sheet image file.";
        NativeSpriteRange = "Sprite Range";
        NativeSpriteRangeHint = "Index of the first and last sprite in the animation.";
        NativeRepeatRange = "Repeat Range";
        NativeRepeatRangeHint = "Delimit a range within which sprites will be repeated.";
        NativeSpritesCut = "Sprites Cut";
        NativeSpritesCutHint = "Cut the sprite sheet and read it separately.";
        NativeFilePath = "File Path";
        NativeControls = "Controls";

        // Common
        CommonEnabled = "Enabled";

        // Stroke: generic param labels
        ParamStrokePosition = "Stroke Position";
        ParamStrokePositionDesc = "Where the stroke is drawn relative to the alpha boundary.";
        ParamFillMode = "Fill Mode";
        ParamFillModeDesc = "How the stroke color is determined.";
        ParamStrokeWidth = "Stroke Width";
        ParamStrokeWidthDesc = "Width of the stroke, normalized to the larger image dimension.";
        ParamStrokeColorRed = "Stroke Color Red";
        ParamStrokeColorRedDesc = "Red component of the stroke color.";
        ParamStrokeColorGreen = "Stroke Color Green";
        ParamStrokeColorGreenDesc = "Green component of the stroke color.";
        ParamStrokeColorBlue = "Stroke Color Blue";
        ParamStrokeColorBlueDesc = "Blue component of the stroke color.";
        ParamStrokeColorAlpha = "Stroke Color Alpha";
        ParamStrokeColorAlphaDesc = "Alpha component of the stroke color.";
        ParamAlphaThreshold = "Alpha Threshold";
        ParamAlphaThresholdDesc = "Alpha value above which pixels are considered inside the shape.";
        ParamEdgeBlend = "Edge Blend";
        ParamEdgeBlendDesc = "Controls how the source edges blend with the stroke. 0 = hard edges, 1 = full blend.";
        ParamStrokeFeathering = "Stroke Feathering";
        ParamStrokeFeatheringDesc = "Softens the stroke edges. Higher values produce softer transitions.";
        ParamSourceOpacity = "Source Opacity";
        ParamSourceOpacityDesc = "Opacity of the source image. 0 = fully transparent, 1 = fully opaque.";
        ParamBlendMode = "Blend Mode";
        ParamBlendModeDesc = "How the stroke is composited with the source image.";
        ParamGradientSettings = "Gradient Settings";
        ParamGradientSettingsDesc = "Gradient parameters used when Fill Mode is set to a gradient option.";
        ParamStartPointX = "Start Point X";
        ParamStartPointXDesc = "X coordinate of the gradient start point.";
        ParamStartPointY = "Start Point Y";
        ParamStartPointYDesc = "Y coordinate of the gradient start point.";
        ParamStartColorRed = "Start Color Red";
        ParamStartColorRedDesc = "Red component of the gradient start color.";
        ParamStartColorGreen = "Start Color Green";
        ParamStartColorGreenDesc = "Green component of the gradient start color.";
        ParamStartColorBlue = "Start Color Blue";
        ParamStartColorBlueDesc = "Blue component of the gradient start color.";
        ParamStartColorAlpha = "Start Color Alpha";
        ParamStartColorAlphaDesc = "Alpha component of the gradient start color.";
        ParamEndPointX = "End Point X";
        ParamEndPointXDesc = "X coordinate of the gradient end point.";
        ParamEndPointY = "End Point Y";
        ParamEndPointYDesc = "Y coordinate of the gradient end point.";
        ParamEndColorRed = "End Color Red";
        ParamEndColorRedDesc = "Red component of the gradient end color.";
        ParamEndColorGreen = "End Color Green";
        ParamEndColorGreenDesc = "Green component of the gradient end color.";
        ParamEndColorBlue = "End Color Blue";
        ParamEndColorBlueDesc = "Blue component of the gradient end color.";
        ParamEndColorAlpha = "End Color Alpha";
        ParamEndColorAlphaDesc = "Alpha component of the gradient end color.";
        ParamUseSharpCorners = "Use Sharp Corners";
        ParamUseSharpCornersDesc = "When enabled, stroke corners are sharp (square) instead of rounded.";

        // Stroke menu item labels & descriptions
        MenuStrokeOuter = "Outer";
        MenuStrokeOuterDesc = "Stroke is drawn outside the shape.";
        MenuStrokeInner = "Inner";
        MenuStrokeInnerDesc = "Stroke is drawn inside the shape.";
        MenuStrokeCenter = "Center";
        MenuStrokeCenterDesc = "Stroke is centered on the alpha boundary.";
        MenuSolidColor = "Solid Color";
        MenuSolidColorDesc = "Uniform stroke color.";
        MenuDistanceGradient = "Distance Gradient";
        MenuDistanceGradientDesc = "Gradient based on distance from start point.";
        MenuGradient = "Gradient";
        MenuGradientDesc = "Linear gradient from start to end point.";
        MenuSourceColorExtension = "Source Color Extension";
        MenuSourceColorExtensionDesc = "Stroke uses the color of the nearest edge pixel.";

        // Blend mode menu item labels (22 modes, shared by stroke + repeater)
        MenuNormal = "Normal";
        MenuNormalDesc = "Standard alpha blending.";
        MenuDissolve = "Dissolve";
        MenuDissolveDesc = "Random dithering based on alpha.";
        MenuDarken = "Darken";
        MenuDarkenDesc = "Keeps the darker of stroke and source.";
        MenuMultiply = "Multiply";
        MenuMultiplyDesc = "Multiplies stroke and source.";
        MenuColorBurn = "Color Burn";
        MenuColorBurnDesc = "Darkens source to reflect stroke.";
        MenuLinearBurn = "Linear Burn";
        MenuLinearBurnDesc = "Linear darkening of source.";
        MenuAdd = "Add";
        MenuAddDesc = "Adds stroke and source values.";
        MenuScreen = "Screen";
        MenuScreenDesc = "Inverse multiply, lightens.";
        MenuColorDodge = "Color Dodge";
        MenuColorDodgeDesc = "Brightens source to reflect stroke.";
        MenuLinearDodge = "Linear Dodge";
        MenuLinearDodgeDesc = "Linear brightening (same as Add).";
        MenuOverlay = "Overlay";
        MenuOverlayDesc = "Combines Multiply and Screen.";
        MenuSoftLight = "Soft Light";
        MenuSoftLightDesc = "Subtle contrast blend.";
        MenuLinearLight = "Linear Light";
        MenuLinearLightDesc = "Linear contrast blend.";
        MenuHardMix = "Hard Mix";
        MenuHardMixDesc = "High-contrast threshold blend.";
        MenuDifference = "Difference";
        MenuDifferenceDesc = "Absolute difference between stroke and source.";
        MenuExclusion = "Exclusion";
        MenuExclusionDesc = "Lower-contrast difference.";
        MenuSubtract = "Subtract";
        MenuSubtractDesc = "Subtracts stroke from source.";
        MenuDivide = "Divide";
        MenuDivideDesc = "Divides source by stroke.";
        MenuStencilAlpha = "Stencil Alpha";
        MenuStencilAlphaDesc = "Uses stroke alpha as a stencil.";
        MenuStencilLuma = "Stencil Luma";
        MenuStencilLumaDesc = "Uses stroke luminance as a stencil.";
        MenuOutlineAlpha = "Outline Alpha";
        MenuOutlineAlphaDesc = "Replaces image with stroke, preserving alpha.";
        MenuOutlineLuma = "Outline Luma";
        MenuOutlineLumaDesc = "Replaces image with stroke, using luminescence.";

        // Repeater: generic param labels
        ParamTimeOffset = "Time Offset";
        ParamTimeOffsetDesc = "Time offset in seconds. Keyframes on this parameter trigger repeat layers. Output time = max(0, currentTime - value).";
        ParamPositionX = "Position X";
        ParamPositionXDesc = "X coordinate of the repeat layer position (0 = left, 1 = right).";
        ParamPositionY = "Position Y";
        ParamPositionYDesc = "Y coordinate of the repeat layer position (0 = top, 1 = bottom).";
        ParamRotation = "Rotation";
        ParamRotationDesc = "Rotation of the repeat layer in degrees around the image center.";
        ParamLayerOrder = "Layer Order";
        ParamLayerOrderDesc = "Whether new repeat layers appear above or below existing content.";
        ParamMaxLayers = "Max Layers";
        ParamMaxLayersDesc = "Maximum number of layers (including the original). 0 = unlimited.";
        ParamRepeaterBlendMode = "Blend Mode";
        ParamRepeaterBlendModeDesc = "How repeat layers are composited with each other.";

        // Repeater menu items
        MenuAbove = "Above";
        MenuAboveDesc = "New repeat layers are composited on top of existing content.";
        MenuBelow = "Below";
        MenuBelowDesc = "New repeat layers are composited beneath existing content.";

        // Repeater blend mode descriptions (different context than stroke)
        MenuRepeaterDarkenDesc = "Keeps the darker of the two layers.";
        MenuRepeaterMultiplyDesc = "Multiplies the two layers.";
        MenuRepeaterColorBurnDesc = "Darkens base to reflect blend layer.";
        MenuRepeaterLinearBurnDesc = "Linear darkening of base.";
        MenuRepeaterAddDesc = "Adds layer values together.";
        MenuRepeaterScreenDesc = "Inverse multiply, lightens.";
        MenuRepeaterColorDodgeDesc = "Brightens base to reflect blend layer.";
        MenuRepeaterLinearDodgeDesc = "Linear brightening (same as Add).";
        MenuRepeaterOverlayDesc = "Combines Multiply and Screen.";
        MenuRepeaterSoftLightDesc = "Subtle contrast blend.";
        MenuRepeaterLinearLightDesc = "Linear contrast blend.";
        MenuRepeaterHardMixDesc = "High-contrast threshold blend.";
        MenuRepeaterDifferenceDesc = "Absolute difference between layers.";
        MenuRepeaterExclusionDesc = "Lower-contrast difference.";
        MenuRepeaterSubtractDesc = "Subtracts blend layer from base.";
        MenuRepeaterDivideDesc = "Divides base by blend layer.";
        MenuRepeaterStencilAlphaDesc = "Uses layer alpha as a stencil.";
        MenuRepeaterStencilLumaDesc = "Uses layer luminance as a stencil.";
        MenuRepeaterOutlineAlphaDesc = "Replaces image with layer, preserving alpha.";
        MenuRepeaterOutlineLumaDesc = "Replaces image with layer, using luminescence.";

        // Sprite Sheet: generic param labels
        ParamColumns = "Columns";
        ParamColumnsDesc = "Number of sprite columns in the sheet. Sprite width = sheet width / columns.";
        ParamRows = "Rows";
        ParamRowsDesc = "Number of sprite rows in the sheet. Sprite height = sheet height / rows.";
        ParamSpriteRangeStart = "Sprite Range Start";
        ParamSpriteRangeStartDesc = "Index of the first sprite in the animation.";
        ParamSpriteRangeEnd = "Sprite Range End";
        ParamSpriteRangeEndDesc = "Index of the last sprite in the animation.";
        ParamFrameOffset = "Frame Offset";
        ParamFrameOffsetDesc = "Frame offset (floored). Shift the animation by partial or whole frames.";
        ParamSpeed = "Speed";
        ParamSpeedDesc = "Playback speed. 0 = paused.";
        ParamReadingDirection = "Reading Direction";
        ParamReadingDirectionDesc = "The reading direction of the sprites.";
        ParamPlaybackMode = "Playback Mode";
        ParamPlaybackModeDesc = "The playback mode for the sprite animation.";
        ParamLoopOffset = "Loop Offset";
        ParamLoopOffsetDesc = "Frame offset for the first sprite in a single loop.";
        ParamRepeatRangeStart = "Repeat Range Start";
        ParamRepeatRangeStartDesc = "First sprite index in the repeat sub-range.";
        ParamRepeatRangeEnd = "Repeat Range End";
        ParamRepeatRangeEndDesc = "Last sprite index in the repeat sub-range.";
        ParamRepeatCount = "Repeat Count";
        ParamRepeatCountDesc = "How many times to repeat the repeat range (0 = no repeat).";
        ParamSpritesCutX = "Sprites Cut X";
        ParamSpritesCutXDesc = "Number of horizontal cut blocks in the sprite sheet.";
        ParamSpritesCutY = "Sprites Cut Y";
        ParamSpritesCutYDesc = "Number of vertical cut blocks in the sprite sheet.";
        ParamScale = "Scale";
        ParamScaleDesc = "Scale factor applied to the output sprite (1.0 = original size).";
        ParamScaleAlgorithm = "Scale Algorithm";
        ParamScaleAlgorithmDesc = "Resampling algorithm used when scaling.";
        ParamSpriteDisplacementX = "Displacement X";
        ParamSpriteDisplacementXDesc = "Horizontal pixel offset applied after scaling and centering.";
        ParamSpriteDisplacementY = "Displacement Y";
        ParamSpriteDisplacementYDesc = "Vertical pixel offset applied after scaling and centering.";
        ParamSpriteDisplacement = "Displacement";
        ParamSpriteDisplacementDesc = "Normalized position offset (0-1, default 0.5 = center).";
        ParamSpriteRotation = "Rotation";
        ParamSpriteRotationDesc = "Rotation angle in degrees around the image center.";
        ParamSpriteDisplacementPixelBased = "Pixel-Based Displacement";
        ParamSpriteDisplacementPixelBasedDesc = "Quantize displacement to the scaled pixel grid for pixel-art movement.";
        ParamSpriteRotationPixelBased = "Pixel-Based Rotation";
        ParamSpriteRotationPixelBasedDesc = "Use RotSprite pixel-art rotation preserving sharp edges.";
        ParamSpriteSelectionMode = "Selection Mode";
        ParamSpriteSelectionModeDesc = "Interactive sprite selection with grid overlay and frame picking.";
        ParamSpriteFitToOutput = "Fit Sprite Sheet to Output";
        ParamSpriteFitToOutputDesc = "Scale the full sprite sheet to fit within output bounds while preserving aspect ratio.";
        ParamSpriteShowGridOverlay = "Show Grid Overlay";
        ParamSpriteShowGridOverlayDesc = "Display grid lines and frame numbers over the sprite sheet in selection mode.";

        // Sprite sheet menu items
        MenuHForward = "H. Forward";
        MenuHForwardDesc = "Read sprites horizontally, left to right.";
        MenuHBackward = "H. Backward";
        MenuHBackwardDesc = "Read sprites horizontally, right to left.";
        MenuVForward = "V. Forward";
        MenuVForwardDesc = "Read sprites vertically, top to bottom.";
        MenuVBackward = "V. Backward";
        MenuVBackwardDesc = "Read sprites vertically, bottom to top.";
        MenuHForwardS = "H. Forward (S)";
        MenuHForwardSDesc = "Read sprites horizontally in S-shape.";
        MenuHBackwardS = "H. Backward (S)";
        MenuHBackwardSDesc = "Read sprites horizontally backward in S-shape.";
        MenuVForwardS = "V. Forward (S)";
        MenuVForwardSDesc = "Read sprites vertically in S-shape.";
        MenuVBackwardS = "V. Backward (S)";
        MenuVBackwardSDesc = "Read sprites vertically backward in S-shape.";
        MenuNormalReverse = "Normal & Reverse";
        MenuNormalReverseDesc = "Play forward then backward.";
        MenuNormalReverseMerge = "N.&R. (Merge)";
        MenuNormalReverseMergeDesc = "Play forward then backward, merging repeated first/last frames.";
        MenuNearestNeighbor = "Nearest Neighbor";
        MenuNearestNeighborDesc = "Fastest, no interpolation.";
        MenuTriangle = "Triangle";
        MenuTriangleDesc = "Bilinear interpolation.";
        MenuCatmullRom = "Catmull-Rom";
        MenuCatmullRomDesc = "Cubic filter, sharp results.";
        MenuGaussian = "Gaussian";
        MenuGaussianDesc = "Gaussian blur filter.";
        MenuLanczos3 = "Lanczos3";
        MenuLanczos3Desc = "Highest quality, 3-lobe Lanczos.";
        MenuPlaybackNormalDesc = "Play sprites in normal order.";

        // ── ASS Subtitle: effect labels ──────────────────
        EffectAssSubtitleName = "zzzFX ASS Subtitle";
        EffectAssSubtitleDesc = "Renders ASS/SSA subtitle files onto the output with style support and blending options.";

        // ── ASS Subtitle: native param labels & hints ─────
        NativeSelectAssFile = "Select ASS File...";
        NativeSelectAssFileHint = "Choose an .ass or .ssa subtitle file to render.";

        // ── ASS Subtitle: generic param labels ────────────
        ParamAssTimeOffsetS = "Time Offset (s)";
        ParamAssTimeOffsetSDesc = "Offset applied to subtitle timestamps in seconds. Adjust this so that subtitle events align with the project timeline.";
        ParamAssScale = "Scale";
        ParamAssScaleDesc = "Global scale factor for all subtitles (1.0 = original size).";
        ParamAssPositionX = "Position X";
        ParamAssPositionXDesc = "Horizontal offset for all subtitles (0 = left, 1 = right).";
        ParamAssPositionY = "Position Y";
        ParamAssPositionYDesc = "Vertical offset for all subtitles (0 = top, 1 = bottom).";
        ParamAssFontScaleX = "Font Scale X";
        ParamAssFontScaleXDesc = "Horizontal font scale factor (1.0 = original).";
        ParamAssFontScaleY = "Font Scale Y";
        ParamAssFontScaleYDesc = "Vertical font scale factor (1.0 = original).";
        ParamAssBlendMode = "Blend Mode";
        ParamAssBlendModeDesc = "How subtitles are composited with the output.";

        // ── ASS Subtitle: blend mode menu descriptions ────
        MenuAssBlendNormalDesc = "Standard alpha blending onto the output.";
        MenuAssBlendAddDesc = "Adds subtitle pixel values to the output.";
        MenuAssBlendScreenDesc = "Screens subtitle with output (inverse multiply, lightens).";
        MenuAssBlendMultiplyDesc = "Multiplies subtitle with output (darkens).";
        MenuAssBlendOverlayDesc = "Combines Multiply and Screen based on output brightness.";

        // ── ASS Subtitle: font override ──────────────────
        NativeFontOverride = "Font Override";
        NativeFontOverrideHint = "Override the ASS file's font with a system font.";
        NativeFontOverrideChoice = "Use font from ASS file";
        ParamAssFontOverrideString = "Font Override Name";

        NativeAssPosition = "Position";
        NativeAssPositionHint = "Position of the subtitles (0-1 normalized).";
        NativeAssFontScale = "Font Scale";
        NativeAssFontScaleHint = "Scale factor applied to font rendering (1.0 = original).";

        ParamAssUseNativeSize = "Use Generator Frame Size";
        ParamAssUseNativeSizeDesc = "When enabled, subtitles are rendered at the generator's output size. When disabled, ASS PlayRes is used for coordinate mapping to preserve original layout.";
    }
}

// ── Example Effect keys ────────────────────────────────────────────────────

i18n_keys! {
    pub ExTrKey {
        // SolidColorBlend param labels
        ParamColorRed = "Color Red";
        ParamColorRedDesc = "Red component of the solid color.";
        ParamColorGreen = "Color Green";
        ParamColorGreenDesc = "Green component of the solid color.";
        ParamColorBlue = "Color Blue";
        ParamColorBlueDesc = "Blue component of the solid color.";
        ParamBlendAmount = "Blend Amount";
        ParamBlendAmountDesc = "Alpha channel blending. 0% = original image, 100% = solid color.";
        ParamExampleBlendMode = "Blend Mode";
        ParamExampleBlendModeDesc = "How the solid color is blended with the image.";

        // SolidColorBlend menu item labels (shared names, example-specific descriptions)
        MenuNormal = "Normal";
        MenuMultiply = "Multiply";
        MenuScreen = "Screen";
        MenuOverlay = "Overlay";
        MenuExampleNormalDesc = "Linear interpolation between image and solid color.";
        MenuExampleMultiplyDesc = "Multiplies the image by the solid color.";
        MenuExampleScreenDesc = "Screens the image with the solid color (inverse multiply).";
        MenuExampleOverlayDesc = "Combines Multiply and Screen based on image brightness.";

        // Standard / legacy
        ParamColor = "Color";
        ParamColorDesc = "Solid color for the effect.";
        ParamStandardBlendMode = "Blend Mode";
        ParamStandardBlendModeDesc = "How the solid color is blended with the image.";
        ParamGroup1 = "Group1";
        ParamGroup1Desc = "Nested group with inner parameters.";
        ParamInnerFloat = "Inner Float";
        ParamInnerFloatDesc = "A floating-point parameter inside a group.";
        ParamInnerBool = "Inner Bool";
        ParamInnerBoolDesc = "A boolean parameter inside a group.";
        ParamExampleEffectName = "Example Effect";
        ParamGroup1Enabled = "Enabled";

        // standard.rs extras
        ParamBrightness = "Brightness";
        ParamBrightnessDesc = "Overall brightness multiplier.";
        ParamInvertColors = "Invert Colors";
        ParamInvertColorsDesc = "Invert all colors in the image.";
        ParamTintRed = "Tint Red";
        ParamTintRedDesc = "Red channel tint multiplier.";
        ParamTintGreen = "Tint Green";
        ParamTintGreenDesc = "Green channel tint multiplier.";
        ParamTintBlue = "Tint Blue";
        ParamTintBlueDesc = "Blue channel tint multiplier.";
        ParamAdvanced = "Advanced";
        ParamAdvancedDesc = "Additional advanced settings.";
        ParamContrast = "Contrast";
        ParamContrastDesc = "Contrast adjustment.";
        ParamSaturation = "Saturation";
        ParamSaturationDesc = "Color saturation adjustment.";
        ParamColorPreset = "Color Preset";
        ParamColorPresetDesc = "Choose a color preset.";
        MenuNone = "None";
        MenuNoneDesc = "No color preset.";
        MenuWarm = "Warm";
        MenuWarmDesc = "Warm color tone.";
        MenuCool = "Cool";
        MenuCoolDesc = "Cool color tone.";
        MenuSepia = "Sepia";
        MenuSepiaDesc = "Sepia color tone.";
    }
}
