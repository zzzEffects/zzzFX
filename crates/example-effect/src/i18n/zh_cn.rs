//! Chinese (zh_CN) translations for Example Effect ExTrKey variants.

use std::ffi::CStr;
use effect_settings::ExTrKey;

pub fn translate_cstr(key: ExTrKey) -> &'static CStr {
    match key {
        // SolidColorBlend param labels
        ExTrKey::ParamColorRed => c"颜色 - 红",
        ExTrKey::ParamColorRedDesc => c"纯色的红色分量。",
        ExTrKey::ParamColorGreen => c"颜色 - 绿",
        ExTrKey::ParamColorGreenDesc => c"纯色的绿色分量。",
        ExTrKey::ParamColorBlue => c"颜色 - 蓝",
        ExTrKey::ParamColorBlueDesc => c"纯色的蓝色分量。",
        ExTrKey::ParamBlendAmount => c"混合量",
        ExTrKey::ParamBlendAmountDesc => c"Alpha 通道混合。0% = 原始图像，100% = 纯色。",
        ExTrKey::ParamExampleBlendMode => c"混合模式",
        ExTrKey::ParamExampleBlendModeDesc => c"纯色与图像的混合方式。",

        // SolidColorBlend menu item labels
        ExTrKey::MenuNormal => c"正常",
        ExTrKey::MenuMultiply => c"正片叠底",
        ExTrKey::MenuScreen => c"滤色",
        ExTrKey::MenuOverlay => c"叠加",
        ExTrKey::MenuExampleNormalDesc => c"图像与纯色之间的线性插值。",
        ExTrKey::MenuExampleMultiplyDesc => c"将图像乘以纯色。",
        ExTrKey::MenuExampleScreenDesc => c"用纯色对图像进行滤色（反向乘法）。",
        ExTrKey::MenuExampleOverlayDesc => c"基于图像亮度结合正片叠底和滤色。",

        // Standard / legacy
        ExTrKey::ParamColor => c"颜色",
        ExTrKey::ParamColorDesc => c"效果的纯色。",
        ExTrKey::ParamStandardBlendMode => c"混合模式",
        ExTrKey::ParamStandardBlendModeDesc => c"纯色与图像的混合方式。",
        ExTrKey::ParamGroup1 => c"分组1",
        ExTrKey::ParamGroup1Desc => c"包含内部参数的嵌套分组。",
        ExTrKey::ParamInnerFloat => c"内部浮点数",
        ExTrKey::ParamInnerFloatDesc => c"分组内的浮点参数。",
        ExTrKey::ParamInnerBool => c"内部布尔值",
        ExTrKey::ParamInnerBoolDesc => c"分组内的布尔参数。",
        ExTrKey::ParamExampleEffectName => c"示例效果",
        ExTrKey::ParamGroup1Enabled => c"启用",

        // standard.rs extras
        ExTrKey::ParamBrightness => c"亮度",
        ExTrKey::ParamBrightnessDesc => c"整体亮度倍增器。",
        ExTrKey::ParamInvertColors => c"反转颜色",
        ExTrKey::ParamInvertColorsDesc => c"反转图像中的所有颜色。",
        ExTrKey::ParamTintRed => c"色调 - 红",
        ExTrKey::ParamTintRedDesc => c"红色通道色调倍增器。",
        ExTrKey::ParamTintGreen => c"色调 - 绿",
        ExTrKey::ParamTintGreenDesc => c"绿色通道色调倍增器。",
        ExTrKey::ParamTintBlue => c"色调 - 蓝",
        ExTrKey::ParamTintBlueDesc => c"蓝色通道色调倍增器。",
        ExTrKey::ParamAdvanced => c"高级",
        ExTrKey::ParamAdvancedDesc => c"其他高级设置。",
        ExTrKey::ParamContrast => c"对比度",
        ExTrKey::ParamContrastDesc => c"对比度调整。",
        ExTrKey::ParamSaturation => c"饱和度",
        ExTrKey::ParamSaturationDesc => c"颜色饱和度调整。",
        ExTrKey::ParamColorPreset => c"颜色预设",
        ExTrKey::ParamColorPresetDesc => c"选择颜色预设。",
        ExTrKey::MenuNone => c"无",
        ExTrKey::MenuNoneDesc => c"无颜色预设。",
        ExTrKey::MenuWarm => c"暖色",
        ExTrKey::MenuWarmDesc => c"暖色调。",
        ExTrKey::MenuCool => c"冷色",
        ExTrKey::MenuCoolDesc => c"冷色调。",
        ExTrKey::MenuSepia => c"怀旧",
        ExTrKey::MenuSepiaDesc => c"怀旧色调。",
    }
}
