use std::ffi::CStr;

use crate::i18n_keys;

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
        ParamStrokeColor = "Stroke Color";
        ParamStrokeColorDesc = "Color of the stroke.";
        ParamGradientStartColor = "Gradient Start Color";
        ParamGradientStartColorDesc = "Color at the gradient start.";
        ParamGradientEndColor = "Gradient End Color";
        ParamGradientEndColorDesc = "Color at the gradient end.";
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
        ParamSpritePlayCount = "Play Count";
        ParamSpritePlayCountDesc = "Number of times to play the animation. 0 = infinite. Negative = auto-compute speed to fit within duration.";
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
        ParamSpriteGridOverlayOpacity = "Grid Overlay Opacity";
        ParamSpriteGridOverlayOpacityDesc = "Opacity of the grid overlay and frame numbers in selection mode. 0 = hidden, 1 = fully opaque.";

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

        // ── ASCII Art: effect labels ──────────────────
        EffectAsciiArtName = "zzzFX ASCII Art Style";
        EffectAsciiArtDesc = "Converts input into ASCII art style by mapping luminance to character glyphs in a configurable grid.";

        // ── ASCII Art: native param labels & hints ─────
        NativeAsciiFontChoice = "Font";
        NativeAsciiFontChoiceHint = "Select a monospace font for character rendering.";
        NativeAsciiFontAutoDetect = "Auto-detect";
        NativeAsciiCustomChars = "Custom Characters";
        NativeAsciiPosition = "Position";
        NativeAsciiPositionHint = "Normalized position of the character grid (0-1). 0.5 = center.";
        ParamAsciiPositionX = "Position X";
        ParamAsciiPositionXDesc = "Horizontal anchor of the character grid. 0 = left, 0.5 = center, 1 = right.";
        ParamAsciiPositionY = "Position Y";
        ParamAsciiPositionYDesc = "Vertical anchor of the character grid. 0 = top, 0.5 = center, 1 = bottom.";
        NativeAsciiFontColor = "Font Color";
        NativeAsciiFontColorHint = "Color used for all characters when Color Mode is set to Solid Color.";
        NativeAsciiBgColor = "Background Color";
        NativeAsciiBgColorHint = "Color of the background behind characters.";
        NativeAsciiCustomCharsHint = "Characters ordered from darkest to lightest. Only used when Character Set is 'Custom'.";

        // ── ASCII Art: generic param labels ────────────
        ParamAsciiCharSetGroup = "Character Set";
        ParamAsciiCharSetGroupDesc = "Select which character categories to include in the ASCII art output.";
        ParamAsciiUseLatin = "Latin";
        ParamAsciiUseLatinDesc = "Include Latin alphabet letters ordered by visual density (WMBDKHQARGZPXSONEYUFVTJCLIwm...).";
        ParamAsciiUseSymbols = "Symbols";
        ParamAsciiUseSymbolsDesc = "Include ASCII symbols (@%#*+=-:. ).";
        ParamAsciiUseNumbers = "Numbers";
        ParamAsciiUseNumbersDesc = "Include digits (9876543210).";
        ParamAsciiUseBlocks = "Blocks";
        ParamAsciiUseBlocksDesc = "Include Unicode block characters (█▓▒░ ).";
        ParamAsciiUseChinese = "Chinese";
        ParamAsciiUseChineseDesc = "Include Chinese characters ordered by visual density.";
        ParamAsciiUseKatakana = "Katakana";
        ParamAsciiUseKatakanaDesc = "Include Japanese katakana characters.";
        ParamAsciiUseHiragana = "Hiragana";
        ParamAsciiUseHiraganaDesc = "Include Japanese hiragana characters.";
        ParamAsciiUseKorean = "Korean";
        ParamAsciiUseKoreanDesc = "Include Korean Hangul characters.";
        ParamAsciiUseCustom = "Custom";
        ParamAsciiUseCustomDesc = "Include user-defined custom characters.";
        ParamAsciiFontSize = "Font Size";
        ParamAsciiFontSizeDesc = "Cell size as a fraction of the smaller output dimension. 5 on 1920×1080 = 54 px.";
        ParamAsciiFontFill = "Font Fill";
        ParamAsciiFontFillDesc = "When enabled, the glyph is scaled to fill the entire cell. When disabled, the glyph is shown at native size, centered.";
        ParamAsciiFontScaleX = "Font Stretch X";
        ParamAsciiFontScaleXDesc = "Horizontal stretch factor for glyphs within each cell. 1.0 = native size.";
        ParamAsciiFontScaleY = "Font Stretch Y";
        ParamAsciiFontScaleYDesc = "Vertical stretch factor for glyphs within each cell. 1.0 = native size.";
        ParamAsciiFontRotation = "Font Rotation";
        ParamAsciiFontRotationDesc = "Rotation angle in degrees for glyphs within each cell.";
        ParamAsciiBrightness = "Brightness";
        ParamAsciiBrightnessDesc = "Pre-mapping brightness adjustment. 0.5 is neutral, 0 = darker, 1 = brighter.";
        ParamAsciiContrast = "Contrast";
        ParamAsciiContrastDesc = "Pre-mapping contrast adjustment. 0.5 is neutral, 0 = flat, 1 = maximum contrast.";
        ParamAsciiInvertLuma = "Invert Luminance";
        ParamAsciiInvertLumaDesc = "When enabled, dark source areas map to light characters and vice versa.";
        ParamAsciiFontColor = "Font Color";
        ParamAsciiFontColorDesc = "Color used for characters when Color Mode is set to Solid Color.";
        ParamAsciiGridColor = "Grid Color";
        ParamAsciiGridColorDesc = "Color of the cell grid lines.";
        ParamAsciiBgColor = "Background Color";
        ParamAsciiBgColorDesc = "Color of the background behind characters.";
        ParamAsciiColorMode = "Color Mode";
        ParamAsciiColorModeDesc = "How to color the output characters.";
        ParamAsciiFontName = "Font Name";
        ParamAsciiFontNameDesc = "Name of the system monospace font to use. Leave empty for auto-detection.";
        ParamAsciiCustomChars = "Custom Characters";
        ParamAsciiCustomCharsDesc = "Characters ordered from darkest to lightest. Only used when Character Set is 'Custom'.";

        // ── ASCII Art: color mode menu items ───────────
        MenuAsciiGrayscale = "Grayscale";
        MenuAsciiGrayscaleDesc = "White characters on a black background.";
        MenuAsciiColored = "Colored";
        MenuAsciiColoredDesc = "Characters retain the average color of their source region.";
        MenuAsciiSolid = "Solid Color";
        MenuAsciiSolidDesc = "All characters rendered in a single custom color.";
        MenuAsciiSolidMapGrayscale = "Solid Map Grayscale";
        MenuAsciiSolidMapGrayscaleDesc = "All characters rendered in a single custom color, with source luminance mapped to opacity.";

        // ── Pixel Art: effect labels ──────────────────
        EffectPixelArtName = "zzzFX Pixel Art Style";
        EffectPixelArtDesc = "Converts input into pixel-art style by quantizing colors in blocks with optional dithering and grid overlay.";

        // ── Pixel Art: generic param labels ────────────
        ParamPixelSizeH = "Pixel Size H";
        ParamPixelSizeHDesc = "Horizontal pixel block size as a fraction of output width. 0.1 on a 1920-wide frame = 192-pixel blocks.";
        ParamPixelSizeV = "Pixel Size V";
        ParamPixelSizeVDesc = "Vertical pixel block size as a fraction of output height. Ignored when 'Square' is enabled.";
        ParamSquare = "Square";
        ParamSquareDesc = "When enabled, vertical pixel size is locked to match horizontal. Disables the Vertical Pixel Size parameter.";
        ParamUseSamePixelSize = "Use Same Pixel Size";
        ParamUseSamePixelSizeDesc = "When enabled, all pixel blocks use the same integer pixel size. When disabled, cell sizes alternate (e.g. 20, 21, 20, 21) for smoother distribution of fractional sizes.";
        ParamColorLevels = "Color Levels";
        ParamColorLevelsDesc = "Number of color levels per channel. Lower values create fewer, more distinct color bands.";
        ParamDithering = "Dithering";
        ParamDitheringDesc = "Dithering algorithm to reduce banding artifacts in quantized color regions.";
        ParamDitheringAmount = "Dithering Amount";
        ParamDitheringAmountDesc = "Strength of the dithering effect. 0 = no dithering, 1 = full dithering.";
        ParamShowGrid = "Show Grid";
        ParamShowGridDesc = "Overlay grid lines between pixel blocks for a retro look.";
        ParamGridThickness = "Grid Thickness";
        ParamGridThicknessDesc = "Grid line thickness as a fraction of the pixel size.";
        ParamGridColorRed = "Grid Color Red";
        ParamGridColorRedDesc = "Red component of the grid line color.";
        ParamGridColorGreen = "Grid Color Green";
        ParamGridColorGreenDesc = "Green component of the grid line color.";
        ParamGridColorBlue = "Grid Color Blue";
        ParamGridColorBlueDesc = "Blue component of the grid line color.";
        ParamGridColor = "Grid Color";
        ParamGridColorDesc = "Color of the grid lines between pixel blocks.";
        ParamGridColorAlpha = "Grid Color Alpha";
        ParamGridColorAlphaDesc = "Alpha/opacity of the grid lines. 0 = fully transparent, 1 = fully opaque.";

        // Pixel Art: native param labels & hints
        NativeGridColor = "Grid Color";
        NativeGridColorHint = "Color of the grid lines between pixel blocks.";
        NativeGridPosition = "Grid Position";
        NativeGridPositionHint = "Anchor point for the pixel grid. (0,0) = top-left, (0.5,0.5) = center, (1,1) = bottom-right.";

        // HalfTone: native param labels & hints
        NativeDotPosition = "Dot Position";
        NativeDotPositionHint = "Anchor point for the halftone dot grid. (0,0) = top-left, (0.5,0.5) = center, (1,1) = bottom-right.";
        ParamGridPositionX = "Grid Position X";
        ParamGridPositionXDesc = "Horizontal anchor of the pixel grid. 0 = left-aligned, 0.5 = centered, 1 = right-aligned.";
        ParamGridPositionY = "Grid Position Y";
        ParamGridPositionYDesc = "Vertical anchor of the pixel grid. 0 = top-aligned, 0.5 = centered, 1 = bottom-aligned.";
        ParamPixelContrast = "Contrast";
        ParamPixelContrastDesc = "Pre-processing contrast. 0.5 is neutral (1.0x), 0 = 0.5x, 1 = 2.0x.";
        ParamPixelSaturation = "Saturation";
        ParamPixelSaturationDesc = "Color saturation. 0.5 is neutral (1.0x), 0 = grayscale, 1 = 2.0x.";

        // ── Pixel Art: dithering menu items ───────────
        MenuDitherNone = "None";
        MenuDitherNoneDesc = "No dithering. Produces sharp color bands.";
        MenuDitherOrdered = "Ordered";
        MenuDitherOrderedDesc = "Bayer matrix ordered dithering. Fast, deterministic pattern.";
        MenuDitherFloydSteinberg = "Floyd-Steinberg";
        MenuDitherFloydSteinbergDesc = "Error-diffusion dithering. Higher quality but more computationally expensive.";

        // ── Long Shadow: effect labels ─────────────
        EffectLongShadowName = "zzzFX Long Shadow";
        EffectLongShadowDesc = "Projects a long directional shadow from the alpha channel for a flat-design style effect.";

        // ── Long Shadow: native param labels & hints ─
        NativeShadowColor = "Shadow Color";
        NativeShadowColorHint = "Color of the long shadow.";
        NativeShadowOffset = "Shadow Offset";
        NativeShadowOffsetHint = "Origin offset for the shadow projection.";

        // ── Long Shadow: generic param labels ──────
        ParamShadowAngle = "Angle";
        ParamShadowAngleDesc = "Direction of the shadow in degrees. 0 = right, 90 = down.";
        ParamShadowLength = "Length";
        ParamShadowLengthDesc = "How far the shadow extends, as a fraction of the frame diagonal.";
        ParamShadowSoftness = "Softness";
        ParamShadowSoftnessDesc = "Edge blur of the shadow. 0 = razor sharp (flat design), 1 = very soft.";
        ParamShadowFade = "Fade";
        ParamShadowFadeDesc = "Opacity falloff along the shadow length. 0 = uniform, 1 = linear fade to transparent.";
        ParamShadowOpacity = "Opacity";
        ParamShadowOpacityDesc = "Overall shadow opacity multiplier.";
        ParamShadowAlphaThreshold = "Alpha Threshold";
        ParamShadowAlphaThresholdDesc = "Source pixels with alpha below this value do not cast shadows.";
        ParamShadowColor = "Shadow Color";
        ParamShadowColorDesc = "Color of the long shadow.";
        ParamShadowSourceOpacity = "Source Opacity";
        ParamShadowSourceOpacityDesc = "Opacity of the source image. 0 = shadow only, 1 = fully opaque.";

        // ── Ambient Light Fusion: effect labels ─────
        EffectAmbientLightName = "zzzFX Ambient Light Fusion";
        EffectAmbientLightDesc = "Extracts ambient light from the background and applies it to the foreground, creating a light wrap and color harmonization effect for seamless compositing.";

        // ── Ambient Light Fusion: generic param labels ─
        ParamAmbientLightIntensity = "Intensity";
        ParamAmbientLightIntensityDesc = "Overall strength of the ambient light fusion effect. 0 = off, 1 = full.";
        ParamAmbientLightEdgeWidth = "Edge Width";
        ParamAmbientLightEdgeWidthDesc = "How far the ambient light reaches inward from the foreground edges. 0 = no light, 1 = entire foreground illuminated.";
        ParamAmbientLightLightWrap = "Light Wrap";
        ParamAmbientLightLightWrapDesc = "How much background ambient light bleeds into the foreground edges.";
        ParamAmbientLightAmbientTint = "Ambient Tint";
        ParamAmbientLightAmbientTintDesc = "How much the interior foreground colors are harmonized with the ambient color temperature.";
        ParamAmbientLightBlurRadius = "Blur Radius";
        ParamAmbientLightBlurRadiusDesc = "Radius in pixels for the background blur that extracts the ambient light map. Larger values create broader, more uniform ambient light.";
        ParamAmbientLightBrightness = "Brightness";
        ParamAmbientLightBrightnessDesc = "Brightness multiplier applied to the ambient light before blending. 1.0 = neutral, 2.0 = twice as bright.";
        ParamAmbientLightFgOpacity = "Foreground Opacity";
        ParamAmbientLightFgOpacityDesc = "Opacity of the foreground layer. 0 = fully transparent, 1 = fully opaque.";
        ParamAmbientLightBgOpacity = "Background Opacity";
        ParamAmbientLightBgOpacityDesc = "Opacity of the background layer. 0 = fully transparent, 1 = fully opaque.";
        ParamAmbientLightSwapFgBg = "Swap Foreground and Background";
        ParamAmbientLightSwapFgBgDesc = "When enabled, swap the foreground and background input clips.";

        // ── MIDI Display: effect labels ──────────────────
        EffectMidiDisplayName = "zzzFX MIDI Display";
        EffectMidiDisplayDesc = "Renders a piano-roll visualization from MIDI files with configurable note appearance and playback.";

        // ── MIDI Display: native param labels & hints ─────
        NativeSelectMidiFile = "Select MIDI File...";
        NativeSelectMidiFileHint = "Choose a .mid or .midi file to visualize.";
        NativeNoteColor = "Note Color";
        NativeNoteColorHint = "Color of notes when Color Mode is set to Solid.";
        NativeNoteBorderColor = "Note Border Color";
        NativeNoteBorderColorHint = "Color of the note border.";
        NativeBackgroundColor = "Background Color";
        NativeBackgroundColorHint = "Color of the background behind the piano roll.";

        // ── MIDI Display: Timing params ──────────────────
        ParamMidiTimeOffsetS = "Time Offset (s)";
        ParamMidiTimeOffsetSDesc = "Offset applied to the timeline playback position, in seconds.";
        ParamMidiBpmSource = "BPM Source";
        ParamMidiBpmSourceDesc = "Whether to use the tempo from the MIDI file or a user-specified BPM.";
        ParamMidiUserBpm = "User BPM";
        ParamMidiUserBpmDesc = "Beats per minute used for playback timing, when BPM Source is set to User Specified.";
        ParamMidiSpeed = "Speed";
        ParamMidiSpeedDesc = "Playback speed multiplier. 1.0 = original speed.";

        // ── MIDI Display: Layout params ──────────────────
        ParamMidiOrientation = "Orientation";
        ParamMidiOrientationDesc = "Layout direction: Horizontal (time left-to-right, pitch bottom-to-top) or Vertical (time top-to-bottom, pitch left-to-right).";
        ParamMidiNoteHeightMin = "Note Height Min";
        ParamMidiNoteHeightMinDesc = "Minimum pixel height for each note row (semitone).";
        ParamMidiKeyRangeMin = "Key Range Min";
        ParamMidiKeyRangeMinDesc = "Lowest MIDI key to display (0 = C-1).";
        ParamMidiKeyRangeMax = "Key Range Max";
        ParamMidiKeyRangeMaxDesc = "Highest MIDI key to display (127 = G9).";
        ParamMidiShowKeyboard = "Show Keyboard";
        ParamMidiShowKeyboardDesc = "Display a piano keyboard on the side of the piano roll.";
        ParamMidiKeyboardWidth = "Keyboard Width";
        ParamMidiKeyboardWidthDesc = "Width of the keyboard as a fraction of the output dimension.";

        // ── MIDI Display: Note Appearance params ─────────
        ParamMidiNoteColor = "Note Color";
        ParamMidiNoteColorDesc = "Color of notes when Color Mode is set to Solid.";
        ParamMidiNoteBorderColor = "Note Border Color";
        ParamMidiNoteBorderColorDesc = "Color of the note border.";
        ParamMidiBackgroundColor = "Background Color";
        ParamMidiBackgroundColorDesc = "Color of the background behind the piano roll.";
        ParamMidiNoteColorMode = "Note Color Mode";
        ParamMidiNoteColorModeDesc = "How note colors are determined.";
        ParamMidiNoteColorR = "Note Color Red";
        ParamMidiNoteColorRDesc = "Red component of the note fill color (Solid mode).";
        ParamMidiNoteColorG = "Note Color Green";
        ParamMidiNoteColorGDesc = "Green component of the note fill color (Solid mode).";
        ParamMidiNoteColorB = "Note Color Blue";
        ParamMidiNoteColorBDesc = "Blue component of the note fill color (Solid mode).";
        ParamMidiNoteColorA = "Note Color Alpha";
        ParamMidiNoteColorADesc = "Alpha component of the note fill color.";
        ParamMidiNoteOpacity = "Note Opacity";
        ParamMidiNoteOpacityDesc = "Overall opacity multiplier for notes.";
        ParamMidiNoteBorderThickness = "Note Border Thickness";
        ParamMidiNoteBorderThicknessDesc = "Thickness of the note border in pixels.";
        ParamMidiNoteBorderColorR = "Note Border Color Red";
        ParamMidiNoteBorderColorRDesc = "Red component of the note border color.";
        ParamMidiNoteBorderColorG = "Note Border Color Green";
        ParamMidiNoteBorderColorGDesc = "Green component of the note border color.";
        ParamMidiNoteBorderColorB = "Note Border Color Blue";
        ParamMidiNoteBorderColorBDesc = "Blue component of the note border color.";
        ParamMidiNoteBorderColorA = "Note Border Color Alpha";
        ParamMidiNoteBorderColorADesc = "Alpha component of the note border color.";
        ParamMidiNoteBorderOpacity = "Note Border Opacity";
        ParamMidiNoteBorderOpacityDesc = "Overall opacity multiplier for note borders.";
        ParamMidiNoteCornerRadius = "Note Corner Radius";
        ParamMidiNoteCornerRadiusDesc = "Corner radius of note rectangles in pixels.";

        // ── MIDI Display: Velocity params ────────────────
        ParamMidiVelocityAffectsOpacity = "Velocity Affects Opacity";
        ParamMidiVelocityAffectsOpacityDesc = "Map note velocity to opacity (higher velocity = more opaque).";
        ParamMidiVelocityAffectsBrightness = "Velocity Affects Brightness";
        ParamMidiVelocityAffectsBrightnessDesc = "Map note velocity to brightness (higher velocity = brighter).";
        ParamMidiMinimumVelocity = "Minimum Velocity";
        ParamMidiMinimumVelocityDesc = "Notes with velocity below this threshold are not displayed.";

        // ── MIDI Display: Background params ──────────────
        ParamMidiBackgroundColorR = "Background Color Red";
        ParamMidiBackgroundColorRDesc = "Red component of the background color.";
        ParamMidiBackgroundColorG = "Background Color Green";
        ParamMidiBackgroundColorGDesc = "Green component of the background color.";
        ParamMidiBackgroundColorB = "Background Color Blue";
        ParamMidiBackgroundColorBDesc = "Blue component of the background color.";
        ParamMidiBackgroundColorA = "Background Color Alpha";
        ParamMidiBackgroundColorADesc = "Alpha component of the background color.";
        ParamMidiBackgroundOpacity = "Background Opacity";
        ParamMidiBackgroundOpacityDesc = "Opacity multiplier for the background.";

        // ── MIDI Display: Track Selection params ─────────
        ParamMidiTrackFilterMode = "Track Filter Mode";
        ParamMidiTrackFilterModeDesc = "Show notes from all tracks or a specific track number.";
        ParamMidiTrackNumber = "Track Number";
        ParamMidiTrackNumberDesc = "Track index to display when Track Filter Mode is set to Specific Track (0-based).";

        // ── MIDI Display: Playback params ────────────────
        ParamMidiLoop = "Loop";
        ParamMidiLoopDesc = "Loop playback when time exceeds the MIDI file duration.";
        ParamMidiQuantizeDisplay = "Quantize Display";
        ParamMidiQuantizeDisplayDesc = "Snap note start and end positions to the nearest beat grid.";
        ParamMidiShowVelocityAsHeight = "Show Velocity As Height";
        ParamMidiShowVelocityAsHeightDesc = "Make note height proportional to velocity. When disabled, all notes have uniform height.";

        // ── MIDI Display: menu item labels ───────────────
        MenuMidiFromMidi = "From MIDI";
        MenuMidiFromMidiDesc = "Use the tempo embedded in the MIDI file.";
        MenuMidiUserSpecified = "User Specified";
        MenuMidiUserSpecifiedDesc = "Use a manually specified BPM value.";
        MenuMidiHorizontal = "Horizontal";
        MenuMidiHorizontalDesc = "Time runs left to right, pitch runs bottom to top.";
        MenuMidiVertical = "Vertical";
        MenuMidiVerticalDesc = "Time runs top to bottom, pitch runs left to right.";
        MenuMidiSolid = "Solid";
        MenuMidiSolidDesc = "All notes use a single uniform color.";
        MenuMidiVelocity = "Velocity";
        MenuMidiVelocityDesc = "Note color varies by MIDI velocity (blue → green → red).";
        MenuMidiChannel = "Channel";
        MenuMidiChannelDesc = "Note color varies by MIDI channel (16 distinct colors).";
        MenuMidiTrack = "Track";
        MenuMidiTrackDesc = "Note color varies by MIDI track.";
        MenuMidiPitch = "Pitch";
        MenuMidiPitchDesc = "Note color varies by pitch (rainbow gradient).";
        MenuMidiAllTracks = "All Tracks";
        MenuMidiAllTracksDesc = "Display notes from all tracks.";
        MenuMidiSpecificTrack = "Specific Track";
        MenuMidiSpecificTrackDesc = "Display notes from a single track only.";

        // ── SVG Display: effect labels ──────────────────
        EffectSvgDisplayName = "zzzFX SVG Display";
        EffectSvgDisplayDesc = "Renders SVG files onto the output frame with scaling, positioning, rotation, and blending controls.";

        // ── SVG Display: native param labels & hints ─────
        NativeSelectSvgFile = "Select SVG File...";
        NativeSelectSvgFileHint = "Choose an .svg file to render.";
        NativeSvgBackgroundColor = "Background Color";
        NativeSvgBackgroundColorHint = "Color of the background behind the SVG.";
        NativeSvgPosition = "Position";
        NativeSvgPositionHint = "Normalized position of the SVG (0-1). 0.5 = center.";
        NativeReloadFile = "Reload File";
        NativeReloadFileHint = "Reload the file from disk.";

        // ── SVG Display: generic param labels ────────────
        ParamSvgScale = "Scale";
        ParamSvgScaleDesc = "Scale multiplier applied to the SVG (1.0 = native size).";
        ParamSvgFitToOutput = "Fit to Output";
        ParamSvgFitToOutputDesc = "Automatically scale the SVG to fit within the output frame bounds.";
        ParamSvgPositionX = "Position X";
        ParamSvgPositionXDesc = "Horizontal position of the SVG (0 = left, 0.5 = center, 1 = right).";
        ParamSvgPositionY = "Position Y";
        ParamSvgPositionYDesc = "Vertical position of the SVG (0 = top, 0.5 = center, 1 = bottom).";
        ParamSvgRotation = "Rotation";
        ParamSvgRotationDesc = "Rotation angle in degrees around the SVG center.";
        ParamSvgOpacity = "Opacity";
        ParamSvgOpacityDesc = "Overall opacity of the rendered SVG. 0 = fully transparent, 1 = fully opaque.";
        ParamSvgPreserveAspectRatio = "Preserve Aspect Ratio";
        ParamSvgPreserveAspectRatioDesc = "When enabled, the SVG aspect ratio is preserved when fitting to output.";
        ParamSvgDpi = "DPI";
        ParamSvgDpiDesc = "Dots per inch for interpreting SVG physical units (pt, cm, etc.).";

        // ── LaTeX Display: effect labels ──────────────────
        EffectLaTeXDisplayName = "zzzFX LaTeX Display";
        EffectLaTeXDisplayDesc = "Renders LaTeX math formulas onto the output frame with configurable font size, styling, and blending controls.";

        // ── LaTeX Display: native param labels & hints ─────
        NativeLaTeXFontChoice = "Font";
        NativeLaTeXFontChoiceHint = "Select a font for the LaTeX formula rendering.";
        NativeLaTeXFormula = "Formula";
        NativeLaTeXFormulaHint = "LaTeX math formula to render. Example: \\frac{a}{b} = \\sqrt{c}";
        NativeLaTeXTextColor = "Text Color";
        NativeLaTeXTextColorHint = "Color of the formula text.";
        NativeLaTeXBackgroundColor = "Background Color";
        NativeLaTeXBackgroundColorHint = "Color of the background behind the formula.";
        NativeLaTeXPosition = "Position";
        NativeLaTeXPositionHint = "Normalized position of the formula (0-1). 0.5 = center.";

        // ── LaTeX Display: generic param labels ────────────
        ParamLaTeXFontSize = "Font Size";
        ParamLaTeXFontSizeDesc = "Base font size in pixels used for the LaTeX formula rendering.";
        ParamLaTeXScale = "Scale";
        ParamLaTeXScaleDesc = "Additional scale multiplier applied to the rendered formula.";
        ParamLaTeXPositionX = "Position X";
        ParamLaTeXPositionXDesc = "Horizontal position (0 = left, 0.5 = center, 1 = right).";
        ParamLaTeXPositionY = "Position Y";
        ParamLaTeXPositionYDesc = "Vertical position (0 = top, 0.5 = center, 1 = bottom).";
        ParamLaTeXRotation = "Rotation";
        ParamLaTeXRotationDesc = "Rotation angle in degrees around the formula center.";
        ParamLaTeXOpacity = "Opacity";
        ParamLaTeXOpacityDesc = "Overall opacity of the rendered formula. 0 = fully transparent, 1 = fully opaque.";
        ParamLaTeXDpi = "DPI";
        ParamLaTeXDpiDesc = "Dots per inch for interpreting formula physical units.";
        ParamLaTeXMathStyle = "Math Style";
        ParamLaTeXMathStyleDesc = "Formula rendering style: Display (centered, larger) or Inline (compact, for use within text).";

        // ── LaTeX Display: menu items ─────────────────────
        MenuLaTeXDisplay = "Display";
        MenuLaTeXDisplayDesc = "Display-style math (centered, larger operators and fractions).";
        MenuLaTeXInline = "Inline";
        MenuLaTeXInlineDesc = "Inline-style math (compact, for use within text).";

        // ── Chroma Key: effect labels ────────────────────
        EffectChromaKeyName = "zzzFX Chroma Key";
        EffectChromaKeyDesc = "Keys out a user-selectable color from the foreground, with edge softness and spill suppression, compositing over the background.";

        // ── Chroma Key: native param labels & hints ────────
        NativeKeyColor = "Key Color";
        NativeKeyColorHint = "Color to key out from the foreground (default: green).";

        // ── Chroma Key: generic param labels ───────────────
        ParamChromaKeyColorRed = "Key Color Red";
        ParamChromaKeyColorRedDesc = "Red component of the key color.";
        ParamChromaKeyColorGreen = "Key Color Green";
        ParamChromaKeyColorGreenDesc = "Green component of the key color.";
        ParamChromaKeyColorBlue = "Key Color Blue";
        ParamChromaKeyColorBlueDesc = "Blue component of the key color.";
        ParamChromaKeyColorAlpha = "Key Color Alpha";
        ParamChromaKeyColorAlphaDesc = "Alpha component of the key color.";
        ParamChromaKeyThreshold = "Threshold";
        ParamChromaKeyThresholdDesc = "How close pixels must be to the key color to be removed. Lower values remove fewer pixels.";
        ParamChromaKeyEdgeSoftness = "Edge Softness";
        ParamChromaKeyEdgeSoftnessDesc = "Blend width at the key edge for smooth transitions.";
        ParamChromaKeySpillSuppression = "Spill Suppression";
        ParamChromaKeySpillSuppressionDesc = "Reduces key color spill on foreground edges by desaturating toward gray.";
        ParamChromaKeyShowMatte = "Show Matte";
        ParamChromaKeyShowMatteDesc = "Display the alpha matte as grayscale for debugging purposes.";
        ParamChromaKeyInvert = "Invert";
        ParamChromaKeyInvertDesc = "Invert the alpha matte. Keyed areas become opaque and non-keyed areas become transparent.";
        ParamChromaKeyKeyColor = "Key Color";
        ParamChromaKeyKeyColorDesc = "Color to key out from the foreground.";
        ParamChromaKeyEdgeBlur = "Edge Blur";
        ParamChromaKeyEdgeBlurDesc = "Spatial blur radius in pixels applied to the alpha matte for softer edges.";

        // ── Cast Shadow: effect labels ────────────────
        EffectCastShadowName = "zzzFX Cast Shadow";
        EffectCastShadowDesc = "Casts a transform-based shadow from the source alpha, with scale, perspective, and softness controls.";

        // ── Cast Shadow: native param labels & hints ───
        NativeCastShadowColor = "Shadow Color";
        NativeCastShadowColorHint = "Color of the cast shadow.";
        NativeCastShadowOffset = "Shadow Offset";
        NativeCastShadowOffsetHint = "Position offset of the shadow from the source.";
        NativeCastShadowManualOffset = "Manual Axis Center";
        NativeCastShadowManualOffsetHint = "Manually adjust the projection axis position. Only available in Manual Single Axis mode.";

        // ── Cast Shadow: generic param labels ──────────
        ParamCastShadowScale = "Scale";
        ParamCastShadowScaleDesc = "Scale factor of the shadow from the pivot point. 1.0 = no scaling.";
        ParamCastShadowSoftness = "Softness";
        ParamCastShadowSoftnessDesc = "Blur amount applied to the shadow edges. 0 = sharp, 1 = very soft.";
        ParamCastShadowShearAngle = "Shear Angle";
        ParamCastShadowShearAngleDesc = "Direction of the shear deformation. 0° = right, 90° = down.";
        ParamCastShadowShearAmount = "Shear Amount";
        ParamCastShadowShearAmountDesc = "Intensity of the shear deformation. 0 = no shear, 1 = maximum shear.";
        ParamCastShadowAlphaThreshold = "Alpha Threshold";
        ParamCastShadowAlphaThresholdDesc = "Source pixels with alpha below this value do not cast shadows.";
        ParamCastShadowSourceOpacity = "Source Opacity";
        ParamCastShadowSourceOpacityDesc = "Opacity of the source image. 0 = shadow only, 1 = fully opaque.";
        ParamCastShadowPivotAngle = "Pivot Angle";
        ParamCastShadowPivotAngleDesc = "Controls which edge of the bounding box serves as the pivot axis. 0° = bottom, 90° = left, 180° = top, 270° = right.";
        ParamCastShadowColor = "Shadow Color";
        ParamCastShadowColorDesc = "Color of the cast shadow.";
        ParamCastShadowPivotMode = "Pivot Mode";
        ParamCastShadowPivotModeDesc = "How the projection axis is determined.";
        ParamCastShadowFade = "Fade";
        ParamCastShadowFadeDesc = "Opacity falloff with distance from the pivot axis. 0 = no fade, 1 = maximum fade.";

        // Pivot mode menu items
        MenuPivotAutoSingle = "Auto - Single Axis";
        MenuPivotAutoSingleDesc = "Automatically detect one projection axis from the full bounding box of the source alpha.";
        MenuPivotAutoMulti = "Auto - Multi Axis";
        MenuPivotAutoMultiDesc = "Detect disconnected alpha regions and project a separate shadow for each.";
        MenuPivotManualSingle = "Manual - Single Axis";
        MenuPivotManualSingleDesc = "Place the projection axis at the frame center. Use Offset Distance and Offset Angle to adjust.";

        // -- QR Code: effect labels --
        EffectQrCodeName = "zzzFX QR Code";
        EffectQrCodeDesc = "Generates a QR code from text and renders it onto the output with configurable colors, scaling, and positioning.";

        // -- QR Code: native param labels & hints --
        NativeQrCodeContent = "Content";
        NativeQrCodeContentHint = "Text or URL to encode in the QR code.";
        NativeQrCodeModuleColor = "Module Color";
        NativeQrCodeModuleColorHint = "Color of the QR code modules (the dark squares).";
        NativeQrCodeLightModuleColor = "Light Module Color";
        NativeQrCodeLightModuleColorHint = "Color of the light (uncolored) modules inside the QR code. Default is white.";
        NativeQrCodeBackgroundColor = "Background Color";
        NativeQrCodeBackgroundColorHint = "Background color behind the QR code. Use alpha to control transparency.";
        NativeQrCodePosition = "Position";
        NativeQrCodePositionHint = "Normalized position of the QR code (0-1). 0.5 = center.";

        // -- QR Code: generic param labels --
        ParamQrCodeScale = "Scale";
        ParamQrCodeScaleDesc = "Scale multiplier applied to the QR code (1.0 = fit to output).";
        ParamQrCodePositionX = "Position X";
        ParamQrCodePositionXDesc = "Horizontal position (0 = left, 0.5 = center, 1 = right).";
        ParamQrCodePositionY = "Position Y";
        ParamQrCodePositionYDesc = "Vertical position (0 = top, 0.5 = center, 1 = bottom).";
        ParamQrCodeRotation = "Rotation";
        ParamQrCodeRotationDesc = "Rotation angle in degrees around the QR code center.";
        ParamQrCodeOpacity = "Opacity";
        ParamQrCodeOpacityDesc = "Overall opacity of the QR code. 0 = fully transparent, 1 = fully opaque.";
        ParamQrCodeMargin = "Margin";
        ParamQrCodeMarginDesc = "Quiet zone margin around the QR code in modules (0-10, standard is 4).";
        ParamQrCodeEcl = "Error Correction Level";
        ParamQrCodeEclDesc = "How much of the QR code can be restored if damaged. Higher levels mean denser codes.";
        ParamQrCodeModuleShape = "Module Shape";
        ParamQrCodeModuleShapeDesc = "Shape of individual QR code modules (squares, circles, etc.).";

        // -- QR Code: ECL menu items --
        MenuQrEclL = "L (Low, ~7%)";
        MenuQrEclLDesc = "Low error correction. Smallest code, least damage resistance.";
        MenuQrEclM = "M (Medium, ~15%)";
        MenuQrEclMDesc = "Medium error correction. Good balance of size and reliability.";
        MenuQrEclQ = "Q (Quartile, ~25%)";
        MenuQrEclQDesc = "High error correction. Larger code, good damage resistance.";
        MenuQrEclH = "H (High, ~30%)";
        MenuQrEclHDesc = "Maximum error correction. Largest code, best damage resistance.";

        // -- QR Code: Module Shape menu items --
        MenuQrShapeSquare = "Square";
        MenuQrShapeSquareDesc = "Standard square modules.";
        MenuQrShapeCircle = "Circle";
        MenuQrShapeCircleDesc = "Round dot modules.";
        MenuQrShapeRoundedSquare = "Rounded Square";
        MenuQrShapeRoundedSquareDesc = "Squares with rounded corners.";
        MenuQrShapeVertical = "Vertical";
        MenuQrShapeVerticalDesc = "Vertical bar modules.";
        MenuQrShapeHorizontal = "Horizontal";
        MenuQrShapeHorizontalDesc = "Horizontal bar modules.";
        MenuQrShapeDiamond = "Diamond";
        MenuQrShapeDiamondDesc = "Diamond-shaped modules.";

        // ── HalfTone: effect labels ──────────────────
        EffectHalfToneName = "zzzFX HalfTone";
        EffectHalfToneDesc = "Classic halftone screen pattern. Converts the image into dots where dot size varies with brightness.";

        // ── HalfTone: generic param labels ────────────
        ParamHalfToneDotSize = "Dot Size";
        ParamHalfToneDotSizeDesc = "Base dot size as a fraction of the frame diagonal. Larger values produce fewer, bigger dots.";
        ParamHalfToneAngle = "Angle";
        ParamHalfToneAngleDesc = "Screen angle in degrees. Controls the direction of the halftone grid.";
        ParamHalfToneDotShape = "Dot Shape";
        ParamHalfToneDotShapeDesc = "Shape of the halftone dots.";
        ParamHalfToneChannelMode = "Channel Mode";
        ParamHalfToneChannelModeDesc = "Luminance: single grayscale halftone. RGB: per-channel halftone with color-angle offsets.";
        ParamHalfToneInvert = "Invert";
        ParamHalfToneInvertDesc = "Invert the pattern — dots appear in dark areas instead of light areas.";
        ParamHalfToneContrast = "Contrast";
        ParamHalfToneContrastDesc = "Pre-processing contrast adjustment. 0.5 is neutral, 0 = flat, 1 = maximum contrast.";
        ParamHalfToneSmoothness = "Smoothness";
        ParamHalfToneSmoothnessDesc = "Anti-aliasing at dot edges. 0 = hard edges, 1 = fully soft.";
        ParamHalfTonePositionX = "Position X";
        ParamHalfTonePositionXDesc = "Horizontal offset of the dot grid. 0 = left, 0.5 = center, 1 = right.";
        ParamHalfTonePositionY = "Position Y";
        ParamHalfTonePositionYDesc = "Vertical offset of the dot grid. 0 = top, 0.5 = center, 1 = bottom.";
        ParamHalfToneBlendWithOriginal = "Blend With Original";
        ParamHalfToneBlendWithOriginalDesc = "Blend the halftone result with the original image. 0 = full halftone, 1 = original image.";

        // ── HalfTone: dot shape menu items ────────────
        MenuDotShapeCircle = "Circle";
        MenuDotShapeCircleDesc = "Round dots.";
        MenuDotShapeSquare = "Square";
        MenuDotShapeSquareDesc = "Square dots.";
        MenuDotShapeDiamond = "Diamond";
        MenuDotShapeDiamondDesc = "Diamond-shaped dots.";

        // ── HalfTone: channel mode menu items ─────────
        MenuChannelLuminance = "Luminance";
        MenuChannelLuminanceDesc = "Single-channel halftone based on perceived brightness.";
        MenuChannelRGB = "RGB";
        MenuChannelRGBDesc = "Per-channel halftone with color separation angles.";

        // ── MultiTone: effect labels ──────────────────
        EffectMultiToneName = "zzzFX MultiTone";
        EffectMultiToneDesc = "Posterization and color quantization effect. Reduces the number of tones for a stylized flat-color look.";

        // ── MultiTone: generic param labels ────────────
        ParamMultiToneLevels = "Tone Levels";
        ParamMultiToneLevelsDesc = "Number of tone levels per channel. Lower values create fewer, more distinct color bands.";
        ParamMultiToneMode = "Mode";
        ParamMultiToneModeDesc = "Quantization mode: Per Channel quantizes R, G, B independently. Luminance quantizes only perceived brightness.";
        ParamMultiToneDithering = "Dithering";
        ParamMultiToneDitheringDesc = "Dithering algorithm to reduce banding artifacts in quantized regions.";
        ParamMultiToneDitheringAmount = "Dithering Amount";
        ParamMultiToneDitheringAmountDesc = "Strength of the dithering effect. 0 = no dithering, 1 = full dithering.";
        ParamMultiToneEdgeSoftness = "Edge Softness";
        ParamMultiToneEdgeSoftnessDesc = "Smooth transitions at tone boundaries. 0 = hard edges, 1 = fully blended.";
        ParamMultiTonePreserveLuminosity = "Preserve Luminosity";
        ParamMultiTonePreserveLuminosityDesc = "Adjust the quantized result to match the original perceived brightness.";

        // ── MultiTone: color mapping params ────────────
        ParamMultiToneColorMapping = "Color Mapping";
        ParamMultiToneColorMappingDesc = "Replace quantized tones with a gradient color map. Uses the luminance of the quantized result to blend between shadow, midtone, and highlight colors.";
        ParamMultiToneShadowColor = "Shadow Color";
        ParamMultiToneShadowColorDesc = "Color mapped to the darkest quantized tones (luminance = 0).";
        ParamMultiToneMidtoneColor = "Midtone Color";
        ParamMultiToneMidtoneColorDesc = "Color mapped to the middle quantized tones (luminance = midtone position).";
        ParamMultiToneHighlightColor = "Highlight Color";
        ParamMultiToneHighlightColorDesc = "Color mapped to the brightest quantized tones (luminance = 1).";
        ParamMultiToneMidtonePosition = "Midtone Position";
        ParamMultiToneMidtonePositionDesc = "Position of the midtone in the luminance range. 0 = closer to shadow, 1 = closer to highlight.";
        ParamMultiToneBlendWithOriginal = "Blend With Original";
        ParamMultiToneBlendWithOriginalDesc = "Blend the color-mapped result with the original quantized colors. 0 = full color map, 1 = original posterized colors.";

        // ── MultiTone: mode menu items ────────────────
        MenuTonePerChannel = "Per Channel";
        MenuTonePerChannelDesc = "Quantize each color channel independently.";
        MenuToneLuminance = "Luminance";
        MenuToneLuminanceDesc = "Quantize only luminance, preserving hue and saturation.";
    }
}
