//! ASS/SSA subtitle parser and renderer.
//!
//! Parses `.ass` files via `oximedia-subtitle`, resolves styles and override tags,
//! then rasterizes subtitle text using fontdue onto an RGBA8 output buffer.

pub mod types;
pub mod parser;
pub mod font;
pub mod render;
pub mod cache;
pub mod composite;

mod transform;

// Re-export all public types
pub use types::*;
pub use parser::{ass_color_to_rgba, parse_ass_file, parse_override_tags, parse_tag_segments};
pub use font::FontCache;
pub use render::render_ass_subtitle_frame;
pub use cache::RenderCache;

#[cfg(test)]
mod tests;
