mod effect;
pub mod blend;
mod solid_blend;
mod zzz_stroke;
pub mod settings;

pub use settings::solid::{SolidColorBlend, SolidColorBlendFullSettings};
pub use settings::standard::{ExampleEffect, ExampleEffectFullSettings};
pub use settings::zzz_stroke::{
    BlendMode as ZzzStrokeBlendMode, FillMode, GradientSettings, StrokePosition, ZzzStroke,
    ZzzStrokeFullSettings,
};
