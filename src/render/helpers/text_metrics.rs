//! Text measurement utilities for PDF builtin fonts
//!
//! This module provides functions to measure text dimensions before rendering,
//! allowing for precise layout calculations using PDF builtin font metrics.

use printpdf::BuiltinFont;

/// Trait for text measurement operations
pub trait TextMeasure {
    /// Measure text width in mm at a given font size
    fn measure_text(&self, text: &str, font_size: f32) -> f32;

    /// Get cap height in mm for a given font size
    fn cap_height_mm(&self, font_size: f32) -> f32;

    /// Get descender depth in mm (positive value)
    fn descender_mm(&self, font_size: f32) -> f32;
}

/// Font metrics for layout calculations
#[derive(Debug, Clone)]
pub struct FontMetrics {
    /// Units per em (for scaling)
    pub units_per_em: i32,
    /// Ascender height in font units
    pub ascender: i16,
    /// Descender depth in font units (typically negative)
    pub descender: i16,
    /// Line gap in font units
    pub line_gap: i16,
    /// Cap height in font units (height of capital letters)
    pub cap_height: i16,
}

impl FontMetrics {
    /// Convert font units to points at a given font size
    pub fn to_points(&self, font_units: i16, font_size: f32) -> f32 {
        (font_units as f32 / self.units_per_em as f32) * font_size
    }

    /// Convert font units to mm at a given font size
    /// 1 point = 0.3528 mm
    pub fn to_mm(&self, font_units: i16, font_size: f32) -> f32 {
        self.to_points(font_units, font_size) * 0.3528
    }

    /// Get ascender height in mm at a given font size
    pub fn ascender_mm(&self, font_size: f32) -> f32 {
        self.to_mm(self.ascender, font_size)
    }

    /// Get descender depth in mm at a given font size (positive value)
    pub fn descender_mm(&self, font_size: f32) -> f32 {
        self.to_mm(-self.descender, font_size) // Make positive
    }

    /// Get cap height in mm at a given font size
    pub fn cap_height_mm(&self, font_size: f32) -> f32 {
        self.to_mm(self.cap_height, font_size)
    }

    /// Get total line height in mm at a given font size
    pub fn line_height_mm(&self, font_size: f32) -> f32 {
        self.to_mm(self.ascender - self.descender + self.line_gap, font_size)
    }
}

// =============================================================================
// Builtin PDF Font Metrics
// =============================================================================
//
// PDF's Standard 14 fonts have well-defined metrics from Adobe's AFM files.
// Character widths are in 1000 units per em.

/// Text measurer for PDF builtin fonts
///
/// Uses hardcoded Adobe AFM metrics for accurate text measurement.
pub struct BuiltinFontMeasurer {
    font: BuiltinFont,
}

impl BuiltinFontMeasurer {
    pub fn new(font: BuiltinFont) -> Self {
        Self { font }
    }

    /// Get character width in 1000 units per em
    fn char_width(&self, c: char) -> u16 {
        // Special handling for suit symbols (rendered with embedded DejaVu Sans)
        // DejaVu Sans has 2048 units per em, suit symbols are 1836 units wide
        // Scaled to 1000 units: 1836 * 1000 / 2048 = 896
        if matches!(c, '\u{2660}' | '\u{2663}' | '\u{2665}' | '\u{2666}') {
            return 896;
        }

        // Beyond ASCII, measure what the font will actually draw
        if !c.is_ascii() {
            return match winansi_char(c) {
                Some(drawn) if drawn.is_ascii() => self.char_width(drawn),
                Some(drawn) => self.upper_width(drawn),
                None => 0,
            };
        }

        let code = c as u8;
        match self.font {
            BuiltinFont::TimesRoman => TIMES_ROMAN_WIDTHS
                .get(code as usize)
                .copied()
                .unwrap_or(250),
            BuiltinFont::TimesBold => TIMES_BOLD_WIDTHS.get(code as usize).copied().unwrap_or(250),
            BuiltinFont::TimesItalic => TIMES_ITALIC_WIDTHS
                .get(code as usize)
                .copied()
                .unwrap_or(250),
            BuiltinFont::TimesBoldItalic => TIMES_BOLD_ITALIC_WIDTHS
                .get(code as usize)
                .copied()
                .unwrap_or(250),
            BuiltinFont::Helvetica => HELVETICA_WIDTHS.get(code as usize).copied().unwrap_or(278),
            BuiltinFont::HelveticaBold => HELVETICA_BOLD_WIDTHS
                .get(code as usize)
                .copied()
                .unwrap_or(278),
            BuiltinFont::HelveticaOblique => {
                HELVETICA_WIDTHS.get(code as usize).copied().unwrap_or(278)
            }
            BuiltinFont::HelveticaBoldOblique => HELVETICA_BOLD_WIDTHS
                .get(code as usize)
                .copied()
                .unwrap_or(278),
            BuiltinFont::Courier
            | BuiltinFont::CourierBold
            | BuiltinFont::CourierOblique
            | BuiltinFont::CourierBoldOblique => 600, // Monospace
            BuiltinFont::Symbol | BuiltinFont::ZapfDingbats => 500,
        }
    }

    /// Width of a Windows-1252 character above ASCII, in 1000 units per em
    fn upper_width(&self, c: char) -> u16 {
        let Some(index) = CP1252_UPPER.iter().position(|&u| u == c) else {
            return 500;
        };
        let widths = match self.font {
            BuiltinFont::TimesRoman => &TIMES_ROMAN_UPPER_WIDTHS,
            BuiltinFont::TimesBold => &TIMES_BOLD_UPPER_WIDTHS,
            BuiltinFont::TimesItalic => &TIMES_ITALIC_UPPER_WIDTHS,
            BuiltinFont::TimesBoldItalic => &TIMES_BOLD_ITALIC_UPPER_WIDTHS,
            BuiltinFont::Helvetica | BuiltinFont::HelveticaOblique => &HELVETICA_UPPER_WIDTHS,
            BuiltinFont::HelveticaBold | BuiltinFont::HelveticaBoldOblique => {
                &HELVETICA_BOLD_UPPER_WIDTHS
            }
            BuiltinFont::Courier
            | BuiltinFont::CourierBold
            | BuiltinFont::CourierOblique
            | BuiltinFont::CourierBoldOblique => return 600,
            BuiltinFont::Symbol | BuiltinFont::ZapfDingbats => return 500,
        };
        widths[index]
    }

    /// Measure text width in points
    pub fn measure_width_pt(&self, text: &str, font_size: f32) -> f32 {
        let total_width: u32 = text.chars().map(|c| self.char_width(c) as u32).sum();
        (total_width as f32 / 1000.0) * font_size
    }

    /// Measure text width in mm
    pub fn measure_width_mm(&self, text: &str, font_size: f32) -> f32 {
        self.measure_width_pt(text, font_size) * 0.3528
    }

    /// Get cap height in mm for the font at given size
    pub fn cap_height_mm(&self, font_size: f32) -> f32 {
        let cap_height = match self.font {
            BuiltinFont::TimesRoman
            | BuiltinFont::TimesBold
            | BuiltinFont::TimesItalic
            | BuiltinFont::TimesBoldItalic => 662, // Times
            BuiltinFont::Helvetica
            | BuiltinFont::HelveticaBold
            | BuiltinFont::HelveticaOblique
            | BuiltinFont::HelveticaBoldOblique => 718, // Helvetica
            BuiltinFont::Courier
            | BuiltinFont::CourierBold
            | BuiltinFont::CourierOblique
            | BuiltinFont::CourierBoldOblique => 562, // Courier
            BuiltinFont::Symbol | BuiltinFont::ZapfDingbats => 700,
        };
        (cap_height as f32 / 1000.0) * font_size * 0.3528
    }

    /// Get ascender height in mm
    pub fn ascender_mm(&self, font_size: f32) -> f32 {
        let ascender = match self.font {
            BuiltinFont::TimesRoman
            | BuiltinFont::TimesBold
            | BuiltinFont::TimesItalic
            | BuiltinFont::TimesBoldItalic => 683,
            BuiltinFont::Helvetica
            | BuiltinFont::HelveticaBold
            | BuiltinFont::HelveticaOblique
            | BuiltinFont::HelveticaBoldOblique => 718,
            BuiltinFont::Courier
            | BuiltinFont::CourierBold
            | BuiltinFont::CourierOblique
            | BuiltinFont::CourierBoldOblique => 629,
            BuiltinFont::Symbol | BuiltinFont::ZapfDingbats => 800,
        };
        (ascender as f32 / 1000.0) * font_size * 0.3528
    }

    /// Get descender depth in mm (positive value)
    pub fn descender_mm(&self, font_size: f32) -> f32 {
        let descender = match self.font {
            BuiltinFont::TimesRoman
            | BuiltinFont::TimesBold
            | BuiltinFont::TimesItalic
            | BuiltinFont::TimesBoldItalic => 217,
            BuiltinFont::Helvetica
            | BuiltinFont::HelveticaBold
            | BuiltinFont::HelveticaOblique
            | BuiltinFont::HelveticaBoldOblique => 207,
            BuiltinFont::Courier
            | BuiltinFont::CourierBold
            | BuiltinFont::CourierOblique
            | BuiltinFont::CourierBoldOblique => 157,
            BuiltinFont::Symbol | BuiltinFont::ZapfDingbats => 200,
        };
        (descender as f32 / 1000.0) * font_size * 0.3528
    }

    /// Get recommended line height in mm
    pub fn line_height_mm(&self, font_size: f32) -> f32 {
        self.ascender_mm(font_size) + self.descender_mm(font_size)
    }
}

impl TextMeasure for BuiltinFontMeasurer {
    fn measure_text(&self, text: &str, font_size: f32) -> f32 {
        self.measure_width_mm(text, font_size)
    }

    fn cap_height_mm(&self, font_size: f32) -> f32 {
        self.cap_height_mm(font_size)
    }

    fn descender_mm(&self, font_size: f32) -> f32 {
        self.descender_mm(font_size)
    }
}

/// Get a builtin font measurer for Times-Roman (serif regular)
pub fn get_times_measurer() -> &'static BuiltinFontMeasurer {
    use std::sync::OnceLock;
    static MEASURER: OnceLock<BuiltinFontMeasurer> = OnceLock::new();
    MEASURER.get_or_init(|| BuiltinFontMeasurer::new(BuiltinFont::TimesRoman))
}

/// Get a builtin font measurer for Times-Bold
pub fn get_times_bold_measurer() -> &'static BuiltinFontMeasurer {
    use std::sync::OnceLock;
    static MEASURER: OnceLock<BuiltinFontMeasurer> = OnceLock::new();
    MEASURER.get_or_init(|| BuiltinFontMeasurer::new(BuiltinFont::TimesBold))
}

/// Get a builtin font measurer for Times-Italic
pub fn get_times_italic_measurer() -> &'static BuiltinFontMeasurer {
    use std::sync::OnceLock;
    static MEASURER: OnceLock<BuiltinFontMeasurer> = OnceLock::new();
    MEASURER.get_or_init(|| BuiltinFontMeasurer::new(BuiltinFont::TimesItalic))
}

/// Get a builtin font measurer for Times-BoldItalic
pub fn get_times_bold_italic_measurer() -> &'static BuiltinFontMeasurer {
    use std::sync::OnceLock;
    static MEASURER: OnceLock<BuiltinFontMeasurer> = OnceLock::new();
    MEASURER.get_or_init(|| BuiltinFontMeasurer::new(BuiltinFont::TimesBoldItalic))
}

/// Get a builtin font measurer for Helvetica (sans-serif regular)
pub fn get_helvetica_measurer() -> &'static BuiltinFontMeasurer {
    use std::sync::OnceLock;
    static MEASURER: OnceLock<BuiltinFontMeasurer> = OnceLock::new();
    MEASURER.get_or_init(|| BuiltinFontMeasurer::new(BuiltinFont::Helvetica))
}

/// Get a builtin font measurer for Helvetica-Bold
pub fn get_helvetica_bold_measurer() -> &'static BuiltinFontMeasurer {
    use std::sync::OnceLock;
    static MEASURER: OnceLock<BuiltinFontMeasurer> = OnceLock::new();
    MEASURER.get_or_init(|| BuiltinFontMeasurer::new(BuiltinFont::HelveticaBold))
}

/// Get the appropriate builtin font measurer for a BuiltinFont
pub fn get_builtin_measurer(font: BuiltinFont) -> &'static BuiltinFontMeasurer {
    match font {
        BuiltinFont::TimesRoman => get_times_measurer(),
        BuiltinFont::TimesBold => get_times_bold_measurer(),
        BuiltinFont::TimesItalic => get_times_italic_measurer(),
        BuiltinFont::TimesBoldItalic => get_times_bold_italic_measurer(),
        BuiltinFont::Helvetica | BuiltinFont::HelveticaOblique => get_helvetica_measurer(),
        BuiltinFont::HelveticaBold | BuiltinFont::HelveticaBoldOblique => {
            get_helvetica_bold_measurer()
        }
        // Courier, Symbol, ZapfDingbats - default to Helvetica metrics
        _ => get_helvetica_measurer(),
    }
}

// =============================================================================
// Adobe AFM Character Width Tables (ASCII subset, in 1000 units per em)
// =============================================================================
//
// These are the standard character widths from Adobe's AFM files for the
// Standard 14 PDF fonts. Only ASCII printable characters (32-126) are included.

/// Times-Roman character widths (indices 0-127, only 32-126 are valid)
#[rustfmt::skip]
static TIMES_ROMAN_WIDTHS: [u16; 128] = [
    // 0-31: Control characters (use 0)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    // 32-47: space ! " # $ % & ' ( ) * + , - . /
    250, 333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278,
    // 48-63: 0 1 2 3 4 5 6 7 8 9 : ; < = > ?
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444,
    // 64-79: @ A B C D E F G H I J K L M N O
    921, 722, 667, 667, 722, 611, 556, 722, 722, 333, 389, 722, 611, 889, 722, 722,
    // 80-95: P Q R S T U V W X Y Z [ \ ] ^ _
    556, 722, 667, 556, 611, 722, 722, 944, 722, 722, 611, 333, 278, 333, 469, 500,
    // 96-111: ` a b c d e f g h i j k l m n o
    333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500, 278, 778, 500, 500,
    // 112-127: p q r s t u v w x y z { | } ~ DEL
    500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541, 0,
];

/// Times-Bold character widths
#[rustfmt::skip]
static TIMES_BOLD_WIDTHS: [u16; 128] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    250, 333, 555, 500, 500, 1000, 833, 278, 333, 333, 500, 570, 250, 333, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500,
    930, 722, 667, 722, 722, 667, 611, 778, 778, 389, 500, 778, 667, 944, 722, 778,
    611, 778, 722, 556, 667, 722, 722, 1000, 722, 722, 667, 333, 278, 333, 581, 500,
    333, 500, 556, 444, 556, 444, 333, 500, 556, 278, 333, 556, 278, 833, 556, 500,
    556, 556, 444, 389, 333, 556, 500, 722, 500, 500, 444, 394, 220, 394, 520, 0,
];

/// Times-Italic character widths
#[rustfmt::skip]
static TIMES_ITALIC_WIDTHS: [u16; 128] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    250, 333, 420, 500, 500, 833, 778, 214, 333, 333, 500, 675, 250, 333, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 333, 333, 675, 675, 675, 500,
    920, 611, 611, 667, 722, 611, 611, 722, 722, 333, 444, 667, 556, 833, 667, 722,
    611, 722, 611, 500, 556, 722, 611, 833, 611, 556, 556, 389, 278, 389, 422, 500,
    333, 500, 500, 444, 500, 444, 278, 500, 500, 278, 278, 444, 278, 722, 500, 500,
    500, 500, 389, 389, 278, 500, 444, 667, 444, 444, 389, 400, 275, 400, 541, 0,
];

/// Times-BoldItalic character widths
#[rustfmt::skip]
static TIMES_BOLD_ITALIC_WIDTHS: [u16; 128] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    250, 389, 555, 500, 500, 833, 778, 278, 333, 333, 500, 570, 250, 333, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500,
    832, 667, 667, 667, 722, 667, 667, 722, 778, 389, 500, 667, 611, 889, 722, 722,
    611, 722, 667, 556, 611, 722, 667, 889, 667, 611, 611, 333, 278, 333, 570, 500,
    333, 500, 500, 444, 500, 444, 333, 500, 556, 278, 278, 500, 278, 778, 556, 500,
    500, 500, 389, 389, 278, 556, 444, 667, 500, 444, 389, 348, 220, 348, 570, 0,
];

/// Helvetica character widths
#[rustfmt::skip]
static HELVETICA_WIDTHS: [u16; 128] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556,
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556,
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556,
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, 0,
];

/// Helvetica-Bold character widths
#[rustfmt::skip]
static HELVETICA_BOLD_WIDTHS: [u16; 128] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611,
    975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556,
    333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611,
    611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584, 0,
];

// ============================================================================
// Windows-1252 upper half
// ============================================================================
//
// The builtin fonts are drawn through WinAnsiEncoding, which is Windows-1252:
// every character below can be printed as itself. The widths come from the
// Adobe AFM files for the standard 14 fonts.

/// The character a builtin font draws for `c`, or `None` when it draws nothing.
///
/// Anything Windows-1252 can encode is drawn as itself -- curly quotes, dashes,
/// bullets, the ellipsis, accented letters. The few common characters outside it
/// fall back to their nearest ASCII look-alike, suit symbols are dropped (they
/// are drawn in the symbol font instead), and anything else becomes `?`.
pub(crate) fn winansi_char(c: char) -> Option<char> {
    if c.is_ascii() || CP1252_UPPER.contains(&c) {
        return Some(c);
    }
    match c {
        '\u{2015}' | '\u{2027}' | '\u{2212}' => Some('-'), // horizontal bar, hyphenation point, minus
        '\u{2023}' => Some('>'),                           // triangular bullet
        '\u{25CF}' => Some('\u{2022}'),                    // black circle, used as a bullet
        '\u{27E6}' => Some('['),                           // white square brackets, which
        '\u{27E7}' => Some(']'),                           // Practice-Bidding-Scenarios writes
        // en, em, thin, hair and narrow no-break spaces
        '\u{2002}' | '\u{2003}' | '\u{2009}' | '\u{200A}' | '\u{202F}' => Some(' '),
        // zero-width joiners draw nothing, and neither do suit symbols here
        '\u{200C}' | '\u{200D}' => None,
        '\u{2660}' | '\u{2663}' | '\u{2665}' | '\u{2666}' => None,
        _ => Some('?'),
    }
}

/// The Windows-1252 byte that encodes `c`, if it has one.
///
/// printpdf passes builtin-font text to lopdf as UTF-8 bytes, so anything past
/// ASCII has to be encoded here instead -- see `LayerBuilder::use_text_builtin`.
pub(crate) fn winansi_byte(c: char) -> Option<u8> {
    if c.is_ascii() {
        return Some(c as u8);
    }
    CP1252_UPPER
        .iter()
        .position(|&u| u == c)
        .map(|i| 0x80 + i as u8)
}

/// Windows-1252 bytes 0x80-0xFF as Unicode; `'\0'` marks the five unassigned codes.
const CP1252_UPPER: [char; 128] = [
    '\u{20AC}', '\0', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\0', '\u{017D}', '\0', '\0',
    '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}',
    '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\0', '\u{017E}', '\u{0178}', '\u{00A0}',
    '\u{00A1}', '\u{00A2}', '\u{00A3}', '\u{00A4}', '\u{00A5}', '\u{00A6}', '\u{00A7}', '\u{00A8}',
    '\u{00A9}', '\u{00AA}', '\u{00AB}', '\u{00AC}', '\u{00AD}', '\u{00AE}', '\u{00AF}', '\u{00B0}',
    '\u{00B1}', '\u{00B2}', '\u{00B3}', '\u{00B4}', '\u{00B5}', '\u{00B6}', '\u{00B7}', '\u{00B8}',
    '\u{00B9}', '\u{00BA}', '\u{00BB}', '\u{00BC}', '\u{00BD}', '\u{00BE}', '\u{00BF}', '\u{00C0}',
    '\u{00C1}', '\u{00C2}', '\u{00C3}', '\u{00C4}', '\u{00C5}', '\u{00C6}', '\u{00C7}', '\u{00C8}',
    '\u{00C9}', '\u{00CA}', '\u{00CB}', '\u{00CC}', '\u{00CD}', '\u{00CE}', '\u{00CF}', '\u{00D0}',
    '\u{00D1}', '\u{00D2}', '\u{00D3}', '\u{00D4}', '\u{00D5}', '\u{00D6}', '\u{00D7}', '\u{00D8}',
    '\u{00D9}', '\u{00DA}', '\u{00DB}', '\u{00DC}', '\u{00DD}', '\u{00DE}', '\u{00DF}', '\u{00E0}',
    '\u{00E1}', '\u{00E2}', '\u{00E3}', '\u{00E4}', '\u{00E5}', '\u{00E6}', '\u{00E7}', '\u{00E8}',
    '\u{00E9}', '\u{00EA}', '\u{00EB}', '\u{00EC}', '\u{00ED}', '\u{00EE}', '\u{00EF}', '\u{00F0}',
    '\u{00F1}', '\u{00F2}', '\u{00F3}', '\u{00F4}', '\u{00F5}', '\u{00F6}', '\u{00F7}', '\u{00F8}',
    '\u{00F9}', '\u{00FA}', '\u{00FB}', '\u{00FC}', '\u{00FD}', '\u{00FE}', '\u{00FF}',
];

/// Times-Roman widths for [`CP1252_UPPER`], from its AFM.
const TIMES_ROMAN_UPPER_WIDTHS: [u16; 128] = [
    500, 0, 333, 500, 444, 1000, 500, 500, 333, 1000, 556, 333, 889, 0, 611, 0, 0, 333, 333, 444,
    444, 350, 500, 1000, 333, 980, 389, 333, 722, 0, 444, 722, 250, 333, 500, 500, 500, 500, 200,
    500, 333, 760, 276, 500, 564, 333, 760, 333, 400, 564, 300, 300, 333, 500, 453, 250, 333, 300,
    310, 500, 750, 750, 750, 444, 722, 722, 722, 722, 722, 722, 889, 667, 611, 611, 611, 611, 333,
    333, 333, 333, 722, 722, 722, 722, 722, 722, 722, 564, 722, 722, 722, 722, 722, 722, 556, 500,
    444, 444, 444, 444, 444, 444, 667, 444, 444, 444, 444, 444, 278, 278, 278, 278, 500, 500, 500,
    500, 500, 500, 500, 564, 500, 500, 500, 500, 500, 500, 500, 500,
];

/// Times-Bold widths for [`CP1252_UPPER`], from its AFM.
const TIMES_BOLD_UPPER_WIDTHS: [u16; 128] = [
    500, 0, 333, 500, 500, 1000, 500, 500, 333, 1000, 556, 333, 1000, 0, 667, 0, 0, 333, 333, 500,
    500, 350, 500, 1000, 333, 1000, 389, 333, 722, 0, 444, 722, 250, 333, 500, 500, 500, 500, 220,
    500, 333, 747, 300, 500, 570, 333, 747, 333, 400, 570, 300, 300, 333, 556, 540, 250, 333, 300,
    330, 500, 750, 750, 750, 500, 722, 722, 722, 722, 722, 722, 1000, 722, 667, 667, 667, 667, 389,
    389, 389, 389, 722, 722, 778, 778, 778, 778, 778, 570, 778, 722, 722, 722, 722, 722, 611, 556,
    500, 500, 500, 500, 500, 500, 722, 444, 444, 444, 444, 444, 278, 278, 278, 278, 500, 556, 500,
    500, 500, 500, 500, 570, 500, 556, 556, 556, 556, 500, 556, 500,
];

/// Times-Italic widths for [`CP1252_UPPER`], from its AFM.
const TIMES_ITALIC_UPPER_WIDTHS: [u16; 128] = [
    500, 0, 333, 500, 556, 889, 500, 500, 333, 1000, 500, 333, 944, 0, 556, 0, 0, 333, 333, 556,
    556, 350, 500, 889, 333, 980, 389, 333, 667, 0, 389, 556, 250, 389, 500, 500, 500, 500, 275,
    500, 333, 760, 276, 500, 675, 333, 760, 333, 400, 675, 300, 300, 333, 500, 523, 250, 333, 300,
    310, 500, 750, 750, 750, 500, 611, 611, 611, 611, 611, 611, 889, 667, 611, 611, 611, 611, 333,
    333, 333, 333, 722, 667, 722, 722, 722, 722, 722, 675, 722, 722, 722, 722, 722, 556, 611, 500,
    500, 500, 500, 500, 500, 500, 667, 444, 444, 444, 444, 444, 278, 278, 278, 278, 500, 500, 500,
    500, 500, 500, 500, 675, 500, 500, 500, 500, 500, 444, 500, 444,
];

/// Times-BoldItalic widths for [`CP1252_UPPER`], from its AFM.
const TIMES_BOLD_ITALIC_UPPER_WIDTHS: [u16; 128] = [
    500, 0, 333, 500, 500, 1000, 500, 500, 333, 1000, 556, 333, 944, 0, 611, 0, 0, 333, 333, 500,
    500, 350, 500, 1000, 333, 1000, 389, 333, 722, 0, 389, 611, 250, 389, 500, 500, 500, 500, 220,
    500, 333, 747, 266, 500, 606, 333, 747, 333, 400, 570, 300, 300, 333, 576, 500, 250, 333, 300,
    300, 500, 750, 750, 750, 500, 667, 667, 667, 667, 667, 667, 944, 667, 667, 667, 667, 667, 389,
    389, 389, 389, 722, 722, 722, 722, 722, 722, 722, 570, 722, 722, 722, 722, 722, 611, 611, 500,
    500, 500, 500, 500, 500, 500, 722, 444, 444, 444, 444, 444, 278, 278, 278, 278, 500, 556, 500,
    500, 500, 500, 500, 570, 500, 556, 556, 556, 556, 444, 500, 444,
];

/// Helvetica widths for [`CP1252_UPPER`], from its AFM.
const HELVETICA_UPPER_WIDTHS: [u16; 128] = [
    556, 0, 222, 556, 333, 1000, 556, 556, 333, 1000, 667, 333, 1000, 0, 611, 0, 0, 222, 222, 333,
    333, 350, 556, 1000, 333, 1000, 500, 333, 944, 0, 500, 667, 278, 333, 556, 556, 556, 556, 260,
    556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333, 333, 333, 556, 537, 278, 333, 333,
    365, 556, 834, 834, 834, 611, 667, 667, 667, 667, 667, 667, 1000, 722, 667, 667, 667, 667, 278,
    278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778, 722, 722, 722, 722, 667, 667, 611,
    556, 556, 556, 556, 556, 556, 889, 500, 556, 556, 556, 556, 278, 278, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 584, 611, 556, 556, 556, 556, 500, 556, 500,
];

/// Helvetica-Bold widths for [`CP1252_UPPER`], from its AFM.
const HELVETICA_BOLD_UPPER_WIDTHS: [u16; 128] = [
    556, 0, 278, 556, 500, 1000, 556, 556, 333, 1000, 667, 333, 1000, 0, 611, 0, 0, 278, 278, 500,
    500, 350, 556, 1000, 333, 1000, 556, 333, 944, 0, 500, 667, 278, 333, 556, 556, 556, 556, 280,
    556, 333, 737, 370, 556, 584, 333, 737, 333, 400, 584, 333, 333, 333, 611, 556, 278, 333, 333,
    365, 556, 834, 834, 834, 611, 722, 722, 722, 722, 722, 722, 1000, 722, 667, 667, 667, 667, 278,
    278, 278, 278, 722, 722, 778, 778, 778, 778, 778, 584, 778, 722, 722, 722, 722, 667, 667, 611,
    556, 556, 556, 556, 556, 556, 889, 556, 556, 556, 556, 556, 278, 278, 278, 278, 611, 611, 611,
    611, 611, 611, 611, 584, 611, 611, 611, 611, 611, 556, 611, 556,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_text_measurement() {
        let measurer = get_times_measurer();

        // Test basic text width measurement
        let width = measurer.measure_width_mm("Hello", 11.0);
        assert!(width > 0.0);

        // Longer text should be wider
        let longer_width = measurer.measure_width_mm("Hello World", 11.0);
        assert!(longer_width > width);

        // Larger font should be wider
        let bigger_width = measurer.measure_width_mm("Hello", 22.0);
        assert!((bigger_width - width * 2.0).abs() < 0.1); // Should be ~2x
    }

    #[test]
    fn test_builtin_font_metrics() {
        let measurer = get_times_measurer();

        // At 11pt font size
        let cap_height = measurer.cap_height_mm(11.0);
        let ascender = measurer.ascender_mm(11.0);

        println!("Times-Roman at 11pt:");
        println!("  Cap height: {:.2} mm", cap_height);
        println!("  Ascender: {:.2} mm", ascender);
        println!("  Line height: {:.2} mm", measurer.line_height_mm(11.0));

        // Cap height should be reasonable (roughly 2-3mm at 11pt)
        assert!(cap_height > 1.5 && cap_height < 4.0);
    }

    #[test]
    fn test_helvetica_vs_times() {
        let times = get_times_measurer();
        let helvetica = get_helvetica_measurer();

        // Both should measure text
        let times_width = times.measure_width_mm("Hello", 11.0);
        let helvetica_width = helvetica.measure_width_mm("Hello", 11.0);

        // Widths should be different (different fonts)
        assert!((times_width - helvetica_width).abs() > 0.01);

        // But both should be reasonable
        assert!(times_width > 0.0 && times_width < 50.0);
        assert!(helvetica_width > 0.0 && helvetica_width < 50.0);
    }

    fn width_in_units(measurer: &BuiltinFontMeasurer, text: &str) -> f32 {
        measurer.measure_width_pt(text, 1000.0)
    }

    #[test]
    fn windows_1252_characters_measure_at_their_afm_width() {
        let times = get_times_measurer();
        // Times-Roman.afm: bullet 350, endash 500, quotedblleft 444, eacute 444
        for (text, width) in [
            ("\u{2022}", 350.0),
            ("\u{2013}", 500.0),
            ("\u{201C}", 444.0),
            ("\u{00E9}", 444.0),
        ] {
            assert!(
                (width_in_units(times, text) - width).abs() < 0.01,
                "{text:?}"
            );
        }
    }

    #[test]
    fn characters_outside_windows_1252_measure_as_what_is_drawn() {
        assert_eq!(winansi_char('\u{202F}'), Some(' '));
        assert_eq!(winansi_char('\u{2212}'), Some('-'));
        assert_eq!(winansi_char('\u{4E2D}'), Some('?'));
        let times = get_times_measurer();
        assert_eq!(
            width_in_units(times, "\u{202F}"),
            width_in_units(times, " ")
        );
    }
}
