#![cfg(any(windows, target_os = "macos"))]

mod handle;
mod window_handle;

use std::{
    fs::File,
    num::NonZero,
};

use after_effects::{self as ae};
use example_effect::{
    ExampleEffect, ExampleEffectFullSettings,
    settings::{
        EnumValue, SettingDescriptor, SettingKind, SettingID, Settings, SettingsList,
    },
};
use raw_window_handle::Win32WindowHandle;
use window_handle::WindowAndDisplayHandle;

// ---------------------------------------------------------------------------
// Plugin struct
// ---------------------------------------------------------------------------

struct Plugin {
    settings: SettingsList<ExampleEffectFullSettings>,
}

impl Default for Plugin {
    fn default() -> Self {
        Self {
            settings: SettingsList::<ExampleEffectFullSettings>::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Parameter IDs
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum ParamID {
    Param(i32),
    GroupStart(i32),
    GroupEnd(i32),
    LoadPresetButton,
    SavePresetButton,
}

trait IDExt {
    fn ae_id(&self) -> i32;
}

impl<T: Settings> IDExt for SettingID<T> {
    fn ae_id(&self) -> i32 {
        self.id as i32 + 1
    }
}

// ---------------------------------------------------------------------------
// Logarithmic slider helpers
// ---------------------------------------------------------------------------

const LOG_SLIDER_BASE: f64 = 100.0;

fn map_logarithmic(value: f64, min: f64, max: f64, base: f64) -> f64 {
    (max - min) * ((f64::powf(base, (value - min) / (max - min)) - 1.0) / (base - 1.0)) + min
}

fn map_logarithmic_inverse(value: f64, min: f64, max: f64, base: f64) -> f64 {
    f64::log(((value - min) / (max - min)) * (base - 1.0) + 1.0, base) * (max - min) + min
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
        params.add_customized(
            ParamID::LoadPresetButton,
            "",
            ae::ButtonDef::setup(|def| {
                def.set_label("Load Preset...");
            }),
            |p| {
                p.set_flag(ParamFlag::START_COLLAPSED, true);
                p.set_flag(ParamFlag::SUPERVISE, true);
                -1
            },
        )?;

        params.add_customized(
            ParamID::SavePresetButton,
            "",
            ae::ButtonDef::setup(|def| {
                def.set_label("Save Preset...");
            }),
            |p| {
                p.set_flag(ParamFlag::START_COLLAPSED, true);
                p.set_flag(ParamFlag::SUPERVISE, true);
                -1
            },
        )?;

        Self::map_params(
            params,
            &self.settings.setting_descriptors,
            &ExampleEffectFullSettings::default(),
            &ExampleEffectFullSettings::legacy_value(),
        )?;

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
            Command::Render {
                in_layer,
                out_layer,
            } => self.legacy_render(in_data, out_data, in_layer, out_layer, params)?,
            Command::SmartPreRender { extra } => self.pre_render(in_data, out_data, extra)?,
            Command::SmartRender { extra } => {
                self.smart_render(in_data, out_data, extra, params)?
            }
            Command::UpdateParamsUi => {
                Self::update_controls_disabled(params, &self.settings.setting_descriptors, true)?
            }
            Command::UserChangedParam { param_index } => {
                self.handle_param_callback(params, in_data, out_data, param_index)?
            }
            Command::GetFlattenedSequenceData => {}
            _ => {}
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helper: ceil division
// ---------------------------------------------------------------------------

fn ceil_div(a: i32, b: i32) -> i32 {
    (a / b) + (a % b != 0) as i32
}

fn ceil_mul_rational(n: i32, scale: RationalScale) -> i32 {
    ceil_div(n * scale.num, scale.den as i32)
}

// ---------------------------------------------------------------------------
// Plugin implementation
// ---------------------------------------------------------------------------

impl Plugin {
    fn global_setup(
        &self,
        in_data: InData,
        mut _out_data: OutData,
        _params: &mut Parameters<ParamID>,
    ) -> Result<(), Error> {
        let is_premiere = in_data.is_premiere();
        if is_premiere {
            let pf = suites::PixelFormat::new()?;
            pf.clear_supported_pixel_formats(in_data.effect_ref())?;
            pf.add_supported_pixel_format(in_data.effect_ref(), pr::PixelFormat::Bgra4444_8u)?;
            pf.add_supported_pixel_format(in_data.effect_ref(), pr::PixelFormat::Bgra4444_16u)?;
            pf.add_supported_pixel_format(in_data.effect_ref(), pr::PixelFormat::Bgra4444_32f)?;
        }

        Ok(())
    }

    fn about(&self, _in_data: InData, mut out_data: OutData) -> Result<(), Error> {
        const DESCRIPTION: &str = "Example effect plugin demonstrating multi-host Rust plugin skeleton.";
        out_data.set_return_msg(
            format!(
                "Example Effect {}.{}.{}\r\r{DESCRIPTION}",
                env!("EFFECT_VERSION_MAJOR"),
                env!("EFFECT_VERSION_MINOR"),
                env!("EFFECT_VERSION_PATCH")
            )
            .as_str(),
        );
        Ok(())
    }

    fn pre_render(
        &self,
        in_data: InData,
        _out_data: OutData,
        mut extra: PreRenderExtra,
    ) -> Result<(), Error> {
        let mut req = extra.output_request();
        req.preserve_rgb_of_zero_alpha = 1;

        req.rect.left = 0;
        req.rect.right = ceil_mul_rational(in_data.width(), in_data.downsample_x());
        req.rect.top = 0;
        req.rect.bottom = ceil_mul_rational(in_data.height(), in_data.downsample_y());

        let in_res = extra.callbacks().checkout_layer(
            0,
            0,
            &req,
            in_data.current_time(),
            in_data.time_step(),
            in_data.time_scale(),
        )?;

        let out_width = ceil_mul_rational(in_res.ref_width, in_data.downsample_x());
        let out_height = ceil_mul_rational(in_res.ref_height, in_data.downsample_y());

        let constrained_rect = Rect {
            left: 0,
            top: 0,
            right: out_width,
            bottom: out_height,
        };

        extra.set_result_rect(constrained_rect);
        extra.set_max_result_rect(constrained_rect);
        extra.set_returns_extra_pixels(true);

        Ok(())
    }

    fn legacy_render(
        &self,
        in_data: InData,
        _out_data: OutData,
        in_layer: Layer,
        out_layer: Layer,
        params: &mut Parameters<ParamID>,
    ) -> Result<(), Error> {
        if !in_data.is_premiere() {
            return Err(Error::BadCallbackParameter);
        }

        if in_layer.width() != out_layer.width() || in_layer.height() != out_layer.height() {
            return Err(Error::BadCallbackParameter);
        }

        self.do_render(in_layer, out_layer, params)?;

        Ok(())
    }

    fn smart_render(
        &self,
        _in_data: InData,
        _out_data: OutData,
        extra: SmartRenderExtra,
        params: &mut Parameters<ParamID>,
    ) -> Result<(), Error> {
        let Some(input_world) = extra.callbacks().checkout_layer_pixels(0)? else {
            return Ok(());
        };
        let Some(output_world) = extra.callbacks().checkout_output()? else {
            return Ok(());
        };

        self.do_render(input_world, output_world, params)
    }

    /// Passthrough render: copies source pixels to destination unchanged.
    fn do_render(
        &self,
        in_layer: Layer,
        mut out_layer: Layer,
        params: &mut Parameters<ParamID>,
    ) -> Result<(), Error> {
        // Apply settings from params (demonstrates the parameter system)
        let _effect: ExampleEffect = self.apply_settings(params)?.into();

        // Extract dimensions before mutable borrow
        let src_row_bytes = in_layer.row_bytes();
        let dst_row_bytes = out_layer.row_bytes();
        let height = in_layer.height().min(out_layer.height()) as usize;
        let width = in_layer.width().min(out_layer.width()) as usize;
        let pixel_size = 4; // Assume BGRA 8-bit

        let src_stride = if src_row_bytes > 0 {
            src_row_bytes as usize
        } else {
            -src_row_bytes as usize
        };
        let dst_stride = if dst_row_bytes > 0 {
            dst_row_bytes as usize
        } else {
            -dst_row_bytes as usize
        };

        let row_bytes = (width as usize) * pixel_size;

        let src_buf = in_layer.buffer();
        let dst_buf = out_layer.buffer_mut();

        for y in 0..height {
            let src_offset = y * src_stride;
            let dst_offset = y * dst_stride;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    src_buf.as_ptr().add(src_offset),
                    dst_buf.as_mut_ptr().add(dst_offset),
                    row_bytes,
                );
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Parameter mapping (generic — works with any Settings type)
    // -----------------------------------------------------------------------

    fn update_controls_disabled(
        params: &mut Parameters<ParamID>,
        descriptors: &[SettingDescriptor<ExampleEffectFullSettings>],
        enabled: bool,
    ) -> Result<(), Error> {
        for descriptor in descriptors {
            if let SettingKind::Group { children, .. } = &descriptor.kind {
                let group_enabled = params
                    .get(ParamID::Param(descriptor.id.ae_id()))?
                    .as_checkbox()?
                    .value();
                Self::update_controls_disabled(params, children, enabled && group_enabled)?;
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

    fn map_params(
        params: &mut Parameters<ParamID>,
        descriptors: &[SettingDescriptor<ExampleEffectFullSettings>],
        default_settings: &ExampleEffectFullSettings,
        legacy_default_settings: &ExampleEffectFullSettings,
    ) -> Result<(), Error> {
        fn get_defaults<T: example_effect::settings::SettingField + 'static>(
            defaults: &ExampleEffectFullSettings,
            legacy_defaults: &ExampleEffectFullSettings,
            descriptor: &SettingDescriptor<ExampleEffectFullSettings>,
        ) -> Result<[T; 2], Error> {
            Ok([
                defaults
                    .get_field(&descriptor.id)
                    .map_err(|_| Error::BadCallbackParameter)?,
                legacy_defaults
                    .get_field(&descriptor.id)
                    .map_err(|_| Error::BadCallbackParameter)?,
            ])
        }

        for descriptor in descriptors {
            match &descriptor.kind {
                SettingKind::Enumeration { options } => {
                    let [default_idx, legacy_default_idx] = get_defaults::<EnumValue>(
                        default_settings,
                        legacy_default_settings,
                        descriptor,
                    )?
                    .map(|default| {
                        options
                            .iter()
                            .position(|item| item.index == default.0)
                            .unwrap() as i32
                            + 1
                    });
                    params.add_customized(
                        ParamID::Param(descriptor.id.ae_id()),
                        descriptor.label,
                        ae::PopupDef::setup(|p| {
                            p.set_options(&options.iter().map(|o| o.label).collect::<Vec<_>>());
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
                    let [default_value, legacy_default_value] = get_defaults::<f32>(
                        default_settings,
                        legacy_default_settings,
                        descriptor,
                    )?
                    .map(|default| match (*logarithmic, default as f64) {
                        (true, v) => {
                            map_logarithmic_inverse(v, 0.0, 1.0, LOG_SLIDER_BASE)
                        }
                        (false, v) => v,
                    } * 100.0);
                    params.add_customized(
                        ParamID::Param(descriptor.id.ae_id()),
                        descriptor.label,
                        ae::FloatSliderDef::setup(|f| {
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
                    )?
                }
                SettingKind::IntRange { range } => {
                    let [default_value, legacy_default_value] =
                        get_defaults::<i32>(default_settings, legacy_default_settings, descriptor)?;
                    params.add_customized(
                        ParamID::Param(descriptor.id.ae_id()),
                        descriptor.label,
                        ae::FloatSliderDef::setup(|f| {
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
                    )?
                }
                SettingKind::FloatRange { range, logarithmic } => {
                    let [default_value, legacy_default_value] =
                        get_defaults::<f32>(default_settings, legacy_default_settings, descriptor)?
                            .map(|default| match (*logarithmic, default as f64) {
                                (true, v) => map_logarithmic_inverse(
                                    v,
                                    *range.start() as f64,
                                    *range.end() as f64,
                                    LOG_SLIDER_BASE,
                                ),
                                (false, v) => v,
                            });
                    params.add_customized(
                        ParamID::Param(descriptor.id.ae_id()),
                        descriptor.label,
                        ae::FloatSliderDef::setup(|f| {
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
                    )?
                }
                SettingKind::Boolean => {
                    let [default_value, legacy_default_value] = get_defaults::<bool>(
                        default_settings,
                        legacy_default_settings,
                        descriptor,
                    )?;
                    params.add_customized(
                        ParamID::Param(descriptor.id.ae_id()),
                        descriptor.label,
                        ae::CheckBoxDef::setup(|c| {
                            c.set_default(default_value);
                            c.set_value(legacy_default_value);
                            c.set_label(descriptor.label);
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
                    let [default_value, legacy_default_value] = get_defaults::<bool>(
                        default_settings,
                        legacy_default_settings,
                        descriptor,
                    )?;
                    params.add_group(
                        ParamID::GroupStart(descriptor_id),
                        ParamID::GroupEnd(descriptor_id),
                        descriptor.label,
                        false,
                        |g| {
                            g.add_customized(
                                ParamID::Param(descriptor_id),
                                descriptor.label,
                                ae::CheckBoxDef::setup(|c| {
                                    c.set_default(default_value);
                                    c.set_value(legacy_default_value);
                                    c.set_label("Enabled");
                                }),
                                |p| {
                                    p.set_id(descriptor_id);
                                    p.set_flag(ParamFlag::START_COLLAPSED, true);
                                    p.set_flag(ParamFlag::USE_VALUE_FOR_OLD_PROJECTS, true);
                                    -1
                                },
                            )?;
                            Self::map_params(
                                g,
                                children,
                                default_settings,
                                legacy_default_settings,
                            )?;
                            Ok(())
                        },
                    )?;
                }
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Preset load/save
    // -----------------------------------------------------------------------

    fn get_window_handle(
        &self,
        #[allow(unused_variables)] in_data: &InData,
    ) -> Result<Option<WindowAndDisplayHandle>, Error> {
        #[cfg(windows)]
        {
            let hwnd = if in_data.is_premiere() {
                premiere::suites::Window::new()
                    .map_err(|_| Error::Generic)?
                    .get_main_window() as usize as isize
            } else {
                let utility = ae::aegp::suites::Utility::new()?;
                utility.main_hwnd()? as usize as isize
            };

            Ok(NonZero::<isize>::new(hwnd)
                .map(|hwnd| unsafe { WindowAndDisplayHandle::new(Win32WindowHandle::new(hwnd)) }))
        }

        #[cfg(not(windows))]
        Ok(None)
    }

    fn handle_param_callback(
        &self,
        params: &mut Parameters<ParamID>,
        in_data: InData,
        mut out_data: OutData,
        param_index: usize,
    ) -> Result<(), Error> {
        match params.type_at(param_index) {
            ParamID::LoadPresetButton => {
                let mut dialog = rfd::FileDialog::new()
                    .add_filter("Example Effect preset", &["json"]);

                if let Some(handle) = self.get_window_handle(&in_data)? {
                    dialog = dialog.set_parent(&handle);
                }

                let Some(preset_path) = dialog.pick_file() else {
                    return Ok(());
                };

                let loaded_preset: Result<_, Box<dyn std::error::Error>> = (|| {
                    let file_contents = std::fs::read_to_string(preset_path)?;
                    Ok(self.settings.from_json_generic(&file_contents)?)
                })();
                let loaded_preset = match loaded_preset {
                    Ok(loaded_preset) => loaded_preset,
                    Err(e) => {
                        out_data.set_error_msg(&format!("Error loading preset: {e}"));
                        return Ok(());
                    }
                };

                Self::update_params_from_settings(
                    &self.settings.setting_descriptors,
                    params,
                    &loaded_preset,
                )?;
            }
            ParamID::SavePresetButton => {
                let mut dialog = rfd::FileDialog::new()
                    .add_filter("Example Effect preset", &["json"])
                    .set_file_name("settings.json");

                if let Some(handle) = self.get_window_handle(&in_data)? {
                    dialog = dialog.set_parent(&handle);
                }

                let Some(preset_path) = dialog.save_file() else {
                    return Ok(());
                };

                let effect_settings = self.apply_settings(params)?;
                let res: Result<(), Box<dyn std::error::Error>> = (|| {
                    let mut destination = File::create(preset_path)?;
                    self.settings
                        .write_json_to_io(&effect_settings, &mut destination)?;

                    Ok(())
                })();
                if let Err(e) = res {
                    out_data.set_error_msg(&format!("Error saving preset: {e}"));
                    return Ok(());
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn update_params_from_settings(
        descriptors: &[SettingDescriptor<ExampleEffectFullSettings>],
        params: &mut Parameters<ParamID>,
        settings: &ExampleEffectFullSettings,
    ) -> Result<(), Error> {
        for descriptor in descriptors {
            match &descriptor.kind {
                SettingKind::Enumeration { options, .. } => {
                    let mut param = params.get_mut(ParamID::Param(descriptor.id.ae_id()))?;
                    let mut param = param.as_popup_mut()?;
                    let setting = settings.get_field::<EnumValue>(&descriptor.id).unwrap().0;
                    param.set_value(
                        options
                            .iter()
                            .position(|item| item.index == setting)
                            .unwrap() as i32
                            + 1,
                    );
                    param.set_value_changed();
                }
                SettingKind::Percentage { logarithmic, .. } => {
                    let mut param = params.get_mut(ParamID::Param(descriptor.id.ae_id()))?;
                    let mut param = param.as_float_slider_mut()?;
                    let setting = settings.get_field::<f32>(&descriptor.id).unwrap();
                    param.set_value(
                        if *logarithmic {
                            map_logarithmic_inverse(setting.into(), 0.0, 1.0, LOG_SLIDER_BASE)
                        } else {
                            setting.into()
                        } * 100.0,
                    );
                    param.set_value_changed();
                }
                SettingKind::IntRange { .. } => {
                    let mut param = params.get_mut(ParamID::Param(descriptor.id.ae_id()))?;
                    let mut param = param.as_float_slider_mut()?;
                    let setting = settings.get_field::<i32>(&descriptor.id).unwrap();
                    param.set_value(setting.into());
                    param.set_value_changed();
                }
                SettingKind::FloatRange {
                    range, logarithmic, ..
                } => {
                    let mut param = params.get_mut(ParamID::Param(descriptor.id.ae_id()))?;
                    let mut param = param.as_float_slider_mut()?;
                    let setting = settings.get_field::<f32>(&descriptor.id).unwrap();
                    param.set_value(if *logarithmic {
                        map_logarithmic_inverse(
                            setting.into(),
                            *range.start() as f64,
                            *range.end() as f64,
                            LOG_SLIDER_BASE,
                        )
                    } else {
                        setting.into()
                    });
                    param.set_value_changed();
                }
                SettingKind::Boolean => {
                    let mut param = params.get_mut(ParamID::Param(descriptor.id.ae_id()))?;
                    let mut param = param.as_checkbox_mut()?;
                    let setting = settings.get_field::<bool>(&descriptor.id).unwrap();
                    param.set_value(setting);
                    param.set_value_changed();
                }
                SettingKind::Group { children, .. } => {
                    let mut param = params.get_mut(ParamID::Param(descriptor.id.ae_id()))?;
                    let mut param = param.as_checkbox_mut()?;
                    let setting = settings.get_field::<bool>(&descriptor.id).unwrap();
                    param.set_value(setting);
                    param.set_value_changed();

                    Self::update_params_from_settings(children, params, settings)?;
                }
            }
        }

        Ok(())
    }

    fn apply_settings(
        &self,
        params: &mut Parameters<ParamID>,
    ) -> Result<ExampleEffectFullSettings, Error> {
        let mut settings = ExampleEffectFullSettings::default();

        fn apply_settings_list(
            descriptors: &[SettingDescriptor<ExampleEffectFullSettings>],
            params: &mut Parameters<ParamID>,
            settings: &mut ExampleEffectFullSettings,
        ) -> Result<(), Error> {
            for descriptor in descriptors {
                match &descriptor.kind {
                    SettingKind::Enumeration { options, .. } => {
                        let selected_item_position = params
                            .get(ParamID::Param(descriptor.id.ae_id()))?
                            .as_popup()?
                            .value()
                            - 1;
                        if selected_item_position < 0 {
                            continue;
                        }
                        let menu_enum_value = options[selected_item_position as usize].index;
                        settings
                            .set_field::<EnumValue>(&descriptor.id, EnumValue(menu_enum_value))
                            .map_err(|_| Error::BadCallbackParameter)?;
                    }
                    SettingKind::Percentage { logarithmic, .. } => {
                        let mut slider_value = params
                            .get(ParamID::Param(descriptor.id.ae_id()))?
                            .as_float_slider()?
                            .value()
                            * 0.01;

                        if *logarithmic {
                            slider_value = map_logarithmic(slider_value, 0.0, 1.0, LOG_SLIDER_BASE);
                        }
                        settings
                            .set_field::<f32>(&descriptor.id, slider_value as f32)
                            .map_err(|_| Error::BadCallbackParameter)?;
                    }
                    SettingKind::IntRange { .. } => {
                        let slider_value = params
                            .get(ParamID::Param(descriptor.id.ae_id()))?
                            .as_float_slider()?
                            .value()
                            .round() as i32;
                        settings
                            .set_field::<i32>(&descriptor.id, slider_value)
                            .map_err(|_| Error::BadCallbackParameter)?;
                    }
                    SettingKind::FloatRange {
                        logarithmic, range, ..
                    } => {
                        let mut slider_value = params
                            .get(ParamID::Param(descriptor.id.ae_id()))?
                            .as_float_slider()?
                            .value();

                        if *logarithmic {
                            slider_value = map_logarithmic(
                                slider_value,
                                *range.start() as f64,
                                *range.end() as f64,
                                LOG_SLIDER_BASE,
                            );
                        }
                        settings
                            .set_field::<f32>(&descriptor.id, slider_value as f32)
                            .map_err(|_| Error::BadCallbackParameter)?;
                    }
                    SettingKind::Boolean => {
                        settings
                            .set_field::<bool>(
                                &descriptor.id,
                                params
                                    .get(ParamID::Param(descriptor.id.ae_id()))?
                                    .as_checkbox()?
                                    .value(),
                            )
                            .map_err(|_| Error::BadCallbackParameter)?;
                    }
                    SettingKind::Group { children, .. } => {
                        settings
                            .set_field::<bool>(
                                &descriptor.id,
                                params
                                    .get(ParamID::Param(descriptor.id.ae_id()))?
                                    .as_checkbox()?
                                    .value(),
                            )
                            .map_err(|_| Error::BadCallbackParameter)?;

                        apply_settings_list(children, params, settings)?;
                    }
                }
            }

            Ok(())
        }

        apply_settings_list(&self.settings.setting_descriptors, params, &mut settings)?;

        Ok(settings)
    }
}
