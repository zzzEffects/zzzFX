use effect_settings::TrKey;
use zzzfx_core::i18n;

#[test]
fn all_keys_have_non_empty_en() {
    for key in TrKey::all() {
        let en = key.en();
        assert!(!en.is_empty(), "Empty English text for key: {:?}", key);
    }
}

#[test]
fn all_keys_have_non_empty_zh_cn() {
    i18n::set_lang(i18n::Lang::ZhCn);
    for key in TrKey::all() {
        let translated = i18n::tr(*key);
        assert!(
            !translated.is_empty(),
            "Empty zh_CN translation for key: {:?}",
            key
        );
    }
}

#[test]
fn all_keys_have_non_empty_cstr() {
    for key in TrKey::all() {
        let en_c = key.en_cstr();
        assert!(
            !en_c.to_bytes().is_empty(),
            "Empty en_cstr for key: {:?}",
            key
        );
    }
    i18n::set_lang(i18n::Lang::ZhCn);
    for key in TrKey::all() {
        let zh_c = i18n::tr_cstr(*key);
        assert!(
            !zh_c.to_bytes().is_empty(),
            "Empty zh_CN cstr for key: {:?}",
            key
        );
    }
}

#[test]
fn language_detection_defaults_to_en() {
    // Explicitly set to English
    i18n::set_lang(i18n::Lang::En);
    assert_eq!(i18n::lang(), i18n::Lang::En);

    let text = i18n::tr(TrKey::ParamStrokePosition);
    assert_eq!(text, "Stroke Position");
}

#[test]
fn chinese_translation_returns_different_text() {
    i18n::set_lang(i18n::Lang::En);
    let en = i18n::tr(TrKey::ParamStrokePosition);
    assert_eq!(en, "Stroke Position");

    i18n::set_lang(i18n::Lang::ZhCn);
    let zh = i18n::tr(TrKey::ParamStrokePosition);
    assert_ne!(zh, en);
    assert_eq!(zh, "描边位置");
}
