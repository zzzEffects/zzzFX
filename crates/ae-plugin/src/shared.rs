use after_effects::{self as ae, Error};
use zzzfx::settings::{EnumValue, SettingDescriptor, SettingKind, SettingID, Settings, TrKey};

// ---------------------------------------------------------------------------
// Parameter IDs
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ParamID {
    Param(i32),
    GroupStart(i32),
    GroupEnd(i32),
}

pub trait IDExt {
    fn ae_id(&self) -> i32;
}

impl<T: Settings> IDExt for SettingID<T> {
    fn ae_id(&self) -> i32 {
        let mut hash: u32 = 5381;
        for &b in self.name.as_bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(b as u32);
        }
        (hash & 0x7FFFFFFF) as i32
    }
}

// ---------------------------------------------------------------------------
// Logarithmic slider helpers
// ---------------------------------------------------------------------------

pub const LOG_SLIDER_BASE: f64 = 100.0;

pub fn map_logarithmic(value: f64, min: f64, max: f64, base: f64) -> f64 {
    (max - min) * ((f64::powf(base, (value - min) / (max - min)) - 1.0) / (base - 1.0)) + min
}

pub fn map_logarithmic_inverse(value: f64, min: f64, max: f64, base: f64) -> f64 {
    f64::log(((value - min) / (max - min)) * (base - 1.0) + 1.0, base) * (max - min) + min
}

// ---------------------------------------------------------------------------
// Math helpers
// ---------------------------------------------------------------------------

pub fn ceil_div(a: i32, b: i32) -> i32 {
    (a / b) + (a % b != 0) as i32
}

pub fn ceil_mul_rational(n: i32, scale: ae::RationalScale) -> i32 {
    ceil_div(n * scale.num, scale.den as i32)
}

// ---------------------------------------------------------------------------
// Generic parameter mapping (works for any T: Settings)
// ---------------------------------------------------------------------------

pub fn update_controls_disabled<T: Settings>(
    params: &mut ae::Parameters<ParamID>,
    descriptors: &[SettingDescriptor<T>],
    enabled: bool,
) -> Result<(), Error> {
    use ae::{ParamUIFlags};
    for descriptor in descriptors {
        if let SettingKind::Group { children, .. } = &descriptor.kind {
            let group_enabled = params
                .get(ParamID::Param(descriptor.id.ae_id()))?
                .as_checkbox()?
                .value();
            update_controls_disabled(params, children, enabled && group_enabled)?;
        }
        if let Ok(p) = params.get(ParamID::Param(descriptor.id.ae_id())) {
            let was_enabled = !p.ui_flags().contains(ParamUIFlags::DISABLED);
            if was_enabled != enabled {
                let mut p = p.clone();
                p.set_ui_flag(ParamUIFlags::DISABLED, !enabled);
                p.update_param_ui()?;
            }
        }
    }
    Ok(())
}

pub fn map_params<T: Settings<Key = TrKey> + 'static>(
    params: &mut ae::Parameters<ParamID>,
    descriptors: &[SettingDescriptor<T>],
    default_settings: &T,
    legacy_default_settings: &T,
) -> Result<(), Error> {
    use ae::{CheckBoxDef, ColorDef, FloatSliderDef, ParamFlag, PopupDef, ValueDisplayFlag};

    fn get_defaults<U: zzzfx::settings::SettingField + 'static, T: Settings>(
        defaults: &T,
        legacy_defaults: &T,
        descriptor: &SettingDescriptor<T>,
    ) -> Result<[U; 2], Error> {
        Ok([
            defaults.get_field(&descriptor.id).map_err(|_| Error::BadCallbackParameter)?,
            legacy_defaults.get_field(&descriptor.id).map_err(|_| Error::BadCallbackParameter)?,
        ])
    }

    for descriptor in descriptors {
        match &descriptor.kind {
            SettingKind::Enumeration { options } => {
                let [default_idx, legacy_default_idx] = get_defaults::<EnumValue, T>(
                    default_settings, legacy_default_settings, descriptor,
                )?
                .map(|default| {
                    options.iter().position(|item| item.index == default.0).unwrap() as i32 + 1
                });
                params.add_customized(
                    ParamID::Param(descriptor.id.ae_id()),
                    zzzfx::i18n::tr(descriptor.label_key),
                    PopupDef::setup(|p| {
                        p.set_options(&options.iter().map(|o| zzzfx::i18n::tr(o.label_key)).collect::<Vec<_>>());
                        p.set_default(default_idx);
                        p.set_value(legacy_default_idx);
                    }),
                    |p| {
                        p.set_id(descriptor.id.ae_id());
                        p.set_flag(ParamFlag::START_COLLAPSED, true);
                        p.set_flag(ParamFlag::USE_VALUE_FOR_OLD_PROJECTS, true);
                        -1
                    },
                )?;
            }
            SettingKind::Percentage { logarithmic } => {
                let [default_value, legacy_default_value] = get_defaults::<f32, T>(
                    default_settings, legacy_default_settings, descriptor,
                )?
                .map(|default| match (*logarithmic, default as f64) {
                    (true, v) => map_logarithmic_inverse(v, 0.0, 1.0, LOG_SLIDER_BASE),
                    (false, v) => v,
                } * 100.0);
                params.add_customized(
                    ParamID::Param(descriptor.id.ae_id()),
                    zzzfx::i18n::tr(descriptor.label_key),
                    FloatSliderDef::setup(|f| {
                        f.set_slider_min(0.0);
                        f.set_valid_min(0.0);
                        f.set_slider_max(100.0);
                        f.set_valid_max(100.0);
                        f.set_default(default_value);
                        f.set_value(legacy_default_value);
                        f.set_display_flags(ValueDisplayFlag::PERCENT);
                        f.set_precision(1);
                    }),
                    |p| {
                        p.set_id(descriptor.id.ae_id());
                        p.set_flag(ParamFlag::START_COLLAPSED, true);
                        p.set_flag(ParamFlag::USE_VALUE_FOR_OLD_PROJECTS, true);
                        -1
                    },
                )?;
            }
            SettingKind::FloatRange { range, logarithmic } => {
                let [default_value, legacy_default_value] = get_defaults::<f32, T>(
                    default_settings, legacy_default_settings, descriptor,
                )?
                .map(|default| match (*logarithmic, default as f64) {
                    (true, v) => map_logarithmic_inverse(v, *range.start() as f64, *range.end() as f64, LOG_SLIDER_BASE),
                    (false, v) => v,
                });
                params.add_customized(
                    ParamID::Param(descriptor.id.ae_id()),
                    zzzfx::i18n::tr(descriptor.label_key),
                    FloatSliderDef::setup(|f| {
                        f.set_slider_min(*range.start());
                        f.set_valid_min(*range.start());
                        f.set_slider_max(*range.end());
                        f.set_valid_max(*range.end());
                        f.set_default(default_value);
                        f.set_value(legacy_default_value);
                        f.set_precision(2);
                    }),
                    |p| {
                        p.set_id(descriptor.id.ae_id());
                        p.set_flag(ParamFlag::START_COLLAPSED, true);
                        p.set_flag(ParamFlag::USE_VALUE_FOR_OLD_PROJECTS, true);
                        -1
                    },
                )?;
            }
            SettingKind::IntRange { range } => {
                let [default_value, legacy_default_value] = get_defaults::<i32, T>(
                    default_settings, legacy_default_settings, descriptor,
                )?;
                params.add_customized(
                    ParamID::Param(descriptor.id.ae_id()),
                    zzzfx::i18n::tr(descriptor.label_key),
                    FloatSliderDef::setup(|f| {
                        f.set_slider_min(*range.start() as f32);
                        f.set_valid_min(*range.start() as f32);
                        f.set_slider_max(*range.end() as f32);
                        f.set_valid_max(*range.end() as f32);
                        f.set_default(default_value as f64);
                        f.set_value(legacy_default_value as f64);
                        f.set_precision(0);
                    }),
                    |p| {
                        p.set_id(descriptor.id.ae_id());
                        p.set_flag(ParamFlag::START_COLLAPSED, true);
                        p.set_flag(ParamFlag::USE_VALUE_FOR_OLD_PROJECTS, true);
                        -1
                    },
                )?;
            }
            SettingKind::Boolean => {
                let [default_value, legacy_default_value] = get_defaults::<bool, T>(
                    default_settings, legacy_default_settings, descriptor,
                )?;
                params.add_customized(
                    ParamID::Param(descriptor.id.ae_id()),
                    zzzfx::i18n::tr(descriptor.label_key),
                    CheckBoxDef::setup(|c| {
                        c.set_default(default_value);
                        c.set_value(legacy_default_value);
                        c.set_label(zzzfx::i18n::tr(descriptor.label_key));
                    }),
                    |p| {
                        p.set_id(descriptor.id.ae_id());
                        p.set_flag(ParamFlag::START_COLLAPSED, true);
                        p.set_flag(ParamFlag::USE_VALUE_FOR_OLD_PROJECTS, true);
                        -1
                    },
                )?;
            }
            SettingKind::Group { children } => {
                let descriptor_id = descriptor.id.ae_id();
                let [default_value, legacy_default_value] = get_defaults::<bool, T>(
                    default_settings, legacy_default_settings, descriptor,
                )?;
                params.add_group(
                    ParamID::GroupStart(descriptor_id),
                    ParamID::GroupEnd(descriptor_id),
                    zzzfx::i18n::tr(descriptor.label_key),
                    false,
                    |g| {
                        g.add_customized(
                            ParamID::Param(descriptor_id),
                            zzzfx::i18n::tr(descriptor.label_key),
                            CheckBoxDef::setup(|c| {
                                c.set_default(default_value);
                                c.set_value(legacy_default_value);
                                c.set_label(zzzfx::i18n::tr(TrKey::CommonEnabled));
                            }),
                            |p| {
                                p.set_id(descriptor_id);
                                p.set_flag(ParamFlag::START_COLLAPSED, true);
                                p.set_flag(ParamFlag::USE_VALUE_FOR_OLD_PROJECTS, true);
                                -1
                            },
                        )?;
                        map_params(g, children, default_settings, legacy_default_settings)?;
                        Ok(())
                    },
                )?;
            }
            SettingKind::ColorRGBA { r_id, g_id, b_id, a_id } => {
                let default_r = default_settings.get_field::<f32>(r_id).map_err(|_| Error::BadCallbackParameter)?;
                let legacy_r = legacy_default_settings.get_field::<f32>(r_id).map_err(|_| Error::BadCallbackParameter)?;
                let default_g = default_settings.get_field::<f32>(g_id).map_err(|_| Error::BadCallbackParameter)?;
                let legacy_g = legacy_default_settings.get_field::<f32>(g_id).map_err(|_| Error::BadCallbackParameter)?;
                let default_b = default_settings.get_field::<f32>(b_id).map_err(|_| Error::BadCallbackParameter)?;
                let legacy_b = legacy_default_settings.get_field::<f32>(b_id).map_err(|_| Error::BadCallbackParameter)?;
                let default_a = default_settings.get_field::<f32>(a_id).map_err(|_| Error::BadCallbackParameter)?;
                let legacy_a = legacy_default_settings.get_field::<f32>(a_id).map_err(|_| Error::BadCallbackParameter)?;
                let to_u8 = |v: f32| -> u8 { (v.clamp(0.0, 1.0) * 255.0).round() as u8 };
                params.add_customized(
                    ParamID::Param(descriptor.id.ae_id()),
                    zzzfx::i18n::tr(descriptor.label_key),
                    ColorDef::setup(|c| {
                        c.set_default(ae::Pixel8 { alpha: to_u8(default_a), red: to_u8(default_r), green: to_u8(default_g), blue: to_u8(default_b) });
                        c.set_value(ae::Pixel8 { alpha: to_u8(legacy_a), red: to_u8(legacy_r), green: to_u8(legacy_g), blue: to_u8(legacy_b) });
                    }),
                    |p| {
                        p.set_id(descriptor.id.ae_id());
                        p.set_flag(ParamFlag::START_COLLAPSED, true);
                        p.set_flag(ParamFlag::USE_VALUE_FOR_OLD_PROJECTS, true);
                        -1
                    },
                )?;
            }
            SettingKind::ColorRGB { r_id, g_id, b_id } => {
                let default_r = default_settings.get_field::<f32>(r_id).map_err(|_| Error::BadCallbackParameter)?;
                let legacy_r = legacy_default_settings.get_field::<f32>(r_id).map_err(|_| Error::BadCallbackParameter)?;
                let default_g = default_settings.get_field::<f32>(g_id).map_err(|_| Error::BadCallbackParameter)?;
                let legacy_g = legacy_default_settings.get_field::<f32>(g_id).map_err(|_| Error::BadCallbackParameter)?;
                let default_b = default_settings.get_field::<f32>(b_id).map_err(|_| Error::BadCallbackParameter)?;
                let legacy_b = legacy_default_settings.get_field::<f32>(b_id).map_err(|_| Error::BadCallbackParameter)?;
                let to_u8 = |v: f32| -> u8 { (v.clamp(0.0, 1.0) * 255.0).round() as u8 };
                params.add_customized(
                    ParamID::Param(descriptor.id.ae_id()),
                    zzzfx::i18n::tr(descriptor.label_key),
                    ColorDef::setup(|c| {
                        c.set_default(ae::Pixel8 { alpha: 255, red: to_u8(default_r), green: to_u8(default_g), blue: to_u8(default_b) });
                        c.set_value(ae::Pixel8 { alpha: 255, red: to_u8(legacy_r), green: to_u8(legacy_g), blue: to_u8(legacy_b) });
                    }),
                    |p| {
                        p.set_id(descriptor.id.ae_id());
                        p.set_flag(ParamFlag::START_COLLAPSED, true);
                        p.set_flag(ParamFlag::USE_VALUE_FOR_OLD_PROJECTS, true);
                        -1
                    },
                )?;
            }
        }
    }

    Ok(())
}

pub fn apply_settings_list<T: Settings>(
    descriptors: &[SettingDescriptor<T>],
    params: &mut ae::Parameters<ParamID>,
    settings: &mut T,
) -> Result<(), Error> {
    for descriptor in descriptors {
        match &descriptor.kind {
            SettingKind::Enumeration { options, .. } => {
                let selected_item_position = params
                    .get(ParamID::Param(descriptor.id.ae_id()))?
                    .as_popup()?
                    .value() - 1;
                if selected_item_position < 0 { continue; }
                let menu_enum_value = options[selected_item_position as usize].index;
                settings.set_field::<EnumValue>(&descriptor.id, EnumValue(menu_enum_value))
                    .map_err(|_| Error::BadCallbackParameter)?;
            }
            SettingKind::Percentage { logarithmic, .. } => {
                let mut slider_value = params
                    .get(ParamID::Param(descriptor.id.ae_id()))?
                    .as_float_slider()?
                    .value() * 0.01;
                if *logarithmic {
                    slider_value = map_logarithmic(slider_value, 0.0, 1.0, LOG_SLIDER_BASE);
                }
                settings.set_field::<f32>(&descriptor.id, slider_value as f32)
                    .map_err(|_| Error::BadCallbackParameter)?;
            }
            SettingKind::IntRange { .. } => {
                let slider_value = params
                    .get(ParamID::Param(descriptor.id.ae_id()))?
                    .as_float_slider()?
                    .value().round() as i32;
                settings.set_field::<i32>(&descriptor.id, slider_value)
                    .map_err(|_| Error::BadCallbackParameter)?;
            }
            SettingKind::FloatRange { logarithmic, range, .. } => {
                let mut slider_value = params
                    .get(ParamID::Param(descriptor.id.ae_id()))?
                    .as_float_slider()?
                    .value();
                if *logarithmic {
                    slider_value = map_logarithmic(
                        slider_value, *range.start() as f64, *range.end() as f64, LOG_SLIDER_BASE,
                    );
                }
                settings.set_field::<f32>(&descriptor.id, slider_value as f32)
                    .map_err(|_| Error::BadCallbackParameter)?;
            }
            SettingKind::Boolean => {
                settings.set_field::<bool>(
                    &descriptor.id,
                    params.get(ParamID::Param(descriptor.id.ae_id()))?
                        .as_checkbox()?
                        .value(),
                ).map_err(|_| Error::BadCallbackParameter)?;
            }
            SettingKind::Group { children, .. } => {
                settings.set_field::<bool>(
                    &descriptor.id,
                    params.get(ParamID::Param(descriptor.id.ae_id()))?
                        .as_checkbox()?
                        .value(),
                ).map_err(|_| Error::BadCallbackParameter)?;
                apply_settings_list(children, params, settings)?;
            }
            SettingKind::ColorRGBA { r_id, g_id, b_id, a_id } => {
                let color = params
                    .get(ParamID::Param(descriptor.id.ae_id()))?
                    .as_color()?
                    .value();
                settings.set_field::<f32>(r_id, color.red as f32 / 255.0).map_err(|_| Error::BadCallbackParameter)?;
                settings.set_field::<f32>(g_id, color.green as f32 / 255.0).map_err(|_| Error::BadCallbackParameter)?;
                settings.set_field::<f32>(b_id, color.blue as f32 / 255.0).map_err(|_| Error::BadCallbackParameter)?;
                settings.set_field::<f32>(a_id, color.alpha as f32 / 255.0).map_err(|_| Error::BadCallbackParameter)?;
            }
            SettingKind::ColorRGB { r_id, g_id, b_id } => {
                let color = params
                    .get(ParamID::Param(descriptor.id.ae_id()))?
                    .as_color()?
                    .value();
                settings.set_field::<f32>(r_id, color.red as f32 / 255.0).map_err(|_| Error::BadCallbackParameter)?;
                settings.set_field::<f32>(g_id, color.green as f32 / 255.0).map_err(|_| Error::BadCallbackParameter)?;
                settings.set_field::<f32>(b_id, color.blue as f32 / 255.0).map_err(|_| Error::BadCallbackParameter)?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Common global_setup (same for all effects)
// ---------------------------------------------------------------------------

pub fn global_setup_common(in_data: ae::InData) -> Result<(), Error> {
    if in_data.is_premiere() {
        let pf = ae::suites::PixelFormat::new()?;
        pf.clear_supported_pixel_formats(in_data.effect_ref())?;
        pf.add_supported_pixel_format(in_data.effect_ref(), ae::pr::PixelFormat::Bgra4444_8u)?;
        pf.add_supported_pixel_format(in_data.effect_ref(), ae::pr::PixelFormat::Bgra4444_16u)?;
        pf.add_supported_pixel_format(in_data.effect_ref(), ae::pr::PixelFormat::Bgra4444_32f)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Common pre_render (same for all filter effects)
// ---------------------------------------------------------------------------

pub fn pre_render_common(
    in_data: ae::InData,
    mut extra: ae::PreRenderExtra,
) -> Result<(), Error> {
    use ae::Rect;

    let mut req = extra.output_request();
    req.preserve_rgb_of_zero_alpha = 1;

    req.rect.left = 0;
    req.rect.right = ceil_mul_rational(in_data.width(), in_data.downsample_x());
    req.rect.top = 0;
    req.rect.bottom = ceil_mul_rational(in_data.height(), in_data.downsample_y());

    let in_res = extra.callbacks().checkout_layer(
        0, 0, &req, in_data.current_time(), in_data.time_step(), in_data.time_scale(),
    )?;

    let out_width = ceil_mul_rational(in_res.ref_width, in_data.downsample_x());
    let out_height = ceil_mul_rational(in_res.ref_height, in_data.downsample_y());

    let constrained_rect = Rect { left: 0, top: 0, right: out_width, bottom: out_height };
    extra.set_result_rect(constrained_rect);
    extra.set_max_result_rect(constrained_rect);
    extra.set_returns_extra_pixels(true);
    Ok(())
}

// ---------------------------------------------------------------------------
// Common do_render pattern: copy strided source -> contiguous, apply effect, copy back
// ---------------------------------------------------------------------------

pub fn copy_layer_to_contiguous(in_layer: &ae::Layer, buf: &mut [u8], width: usize, height: usize) {
    let src_row_bytes = in_layer.row_bytes();
    let stride = if src_row_bytes > 0 { src_row_bytes as usize } else { -src_row_bytes as usize };
    let row_bytes = width * 4;
    let src_buf = in_layer.buffer();
    for y in 0..height {
        unsafe {
            std::ptr::copy_nonoverlapping(
                src_buf.as_ptr().add(y * stride),
                buf.as_mut_ptr().add(y * row_bytes),
                row_bytes,
            );
        }
    }
}

pub fn copy_contiguous_to_layer(buf: &[u8], out_layer: &mut ae::Layer, width: usize, height: usize) {
    let dst_row_bytes = out_layer.row_bytes();
    let stride = if dst_row_bytes > 0 { dst_row_bytes as usize } else { -dst_row_bytes as usize };
    let row_bytes = width * 4;
    let dst_buf = out_layer.buffer_mut();
    for y in 0..height {
        unsafe {
            std::ptr::copy_nonoverlapping(
                buf.as_ptr().add(y * row_bytes),
                dst_buf.as_mut_ptr().add(y * stride),
                row_bytes,
            );
        }
    }
}
