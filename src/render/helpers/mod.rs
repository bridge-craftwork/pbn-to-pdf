//! Helper utilities for PDF rendering

pub mod card_assets;
pub mod colors;
pub mod compress;
pub mod document;
pub mod fonts;
pub mod layer;
pub mod layout;
pub mod text_metrics;

pub use card_assets::{CardAssets, CardLoadError, CARD_HEIGHT_MM, CARD_WIDTH_MM};
pub use colors::{SuitColors, BLACK};
pub use compress::compress_pdf;
pub use fonts::{BuiltinFontSet, FontFamily, FontManager};
pub use layer::LayerBuilder;
pub use layout::LayoutEngine;
pub use text_metrics::{
    get_builtin_measurer, get_helvetica_bold_measurer, get_helvetica_measurer,
    get_times_bold_italic_measurer, get_times_bold_measurer, get_times_italic_measurer,
    get_times_measurer, BuiltinFontMeasurer, FontMetrics,
};
