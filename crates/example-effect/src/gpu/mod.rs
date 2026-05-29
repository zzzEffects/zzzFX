pub mod solid_blend;
pub mod standard;

use std::borrow::Cow;

/// Load a WGSL shader by prepending the shared function definitions.
pub(crate) fn load_shader(specific: &'static str) -> wgpu::ShaderSource<'static> {
    let shared = include_str!("../../shaders/shared.wgsl");
    wgpu::ShaderSource::Wgsl(Cow::Owned(format!("{shared}\n{specific}")))
}
