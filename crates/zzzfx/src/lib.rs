pub mod blend;
pub mod gpu;
pub mod i18n;
pub mod settings;

mod ambient_light;
mod cast_shadow;
mod ascii_art;
mod chroma_key;
mod long_shadow;
pub mod midi_display;
mod stroke;
mod repeater;
mod sprite_sheet;
pub mod ass_subtitle;
mod pixel_art;
pub mod latex_display;
pub mod svg_display;

pub use repeater::CompositorLayer;

pub use settings::ambient_light::{AmbientLight, AmbientLightFullSettings};
pub use settings::cast_shadow::{CastShadow, CastShadowFullSettings};
pub use settings::chroma_key::{ChromaKey, ChromaKeyFullSettings};
pub use settings::ascii_art::{
    ColorMode as AsciiColorMode, AsciiArt, AsciiArtFullSettings,
};
pub use settings::ass_subtitle::{
    AssBlendMode, AssSubtitle, AssSubtitleFullSettings,
};
pub use settings::pixel_art::{
    Dithering as PixelDithering, PixelArt, PixelArtFullSettings,
};
pub use settings::long_shadow::{LongShadow, LongShadowFullSettings};
pub use settings::midi_display::{
    MidiBpmSource, MidiNoteColorMode, MidiOrientation, MidiTrackFilterMode,
    MidiDisplay, MidiDisplayFullSettings,
};
pub use settings::ascii_art::setting_id as ascii_art_setting_id;
pub use settings::pixel_art::setting_id as pixel_art_setting_id;
pub use settings::repeater::{
    LayerOrder, Repeater, RepeaterFullSettings,
};
pub use settings::sprite_sheet::{
    PlaybackMode, ReadingDirection, ScaleAlgorithm, SpriteSheet,
    SpriteSheetFullSettings,
};
pub use settings::stroke::{
    BlendMode as StrokeBlendMode, FillMode, GradientSettings, StrokePosition, Stroke,
    StrokeFullSettings,
};
pub use settings::latex_display::{LaTeXDisplay, LaTeXDisplayFullSettings, MathStyle as LaTeXMathStyle};
pub use settings::svg_display::{SvgDisplay, SvgDisplayFullSettings};

use resvg::usvg;
use std::sync::OnceLock;

/// Lazy font database shared by all effects that need system fonts (SVG, LaTeX).
pub(crate) fn get_fontdb() -> &'static usvg::fontdb::Database {
    static FONTDB: OnceLock<usvg::fontdb::Database> = OnceLock::new();
    FONTDB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}
