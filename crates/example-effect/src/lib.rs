mod effect;
pub mod i18n;
mod solid_blend;
pub mod settings;
#[cfg(feature = "gpu")]
pub mod gpu;

pub use effect_settings::setting_id;

/// Reciprocal of 255.0 — multiply by this instead of dividing by 255.0.
pub const RECIP_255: f32 = 1.0_f32 / 255.0_f32;

pub use settings::solid::{SolidColorBlend, SolidColorBlendFullSettings};
pub use settings::standard::{ExampleEffect, ExampleEffectFullSettings};
