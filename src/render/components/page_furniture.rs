//! Page furniture: what Bridge Composer prints around the boards rather than
//! for them (issue #28) -- the event heading or page header, and the footer
//! lines `%PageFooter` asks for.
//!
//! Measured against Bridge Composer 5.118.2:
//!
//! - With `PageHeader` in `%BCOptions`, the page's first `[Event]` is a
//!   header centred in the top margin, `%EventSpacing` points above it. It
//!   takes no room from the boards.
//! - Without it, the event heads each column -- the page, one board to a
//!   page -- inside the content area, and pushes that column down a line.
//! - Row 0 of `%PageFooter` prints under each page: column 0 at the left
//!   margin, 1 centred, 2 against the right margin, from just below the
//!   bottom margin. `\\n` breaks a line.
//! - In a footer, `%D` is `%HRTitleDate` spelt out, `%s` the site, `%n` the
//!   page number, `%N` the page count, `%e` `%HRTitleEvent`, `%t` the set id
//!   and `%%` a percent sign; any other letter stands for itself.
//!
//! Bridge Composer fills a missing date with today's and `%P` with the
//! file's folder. Output has to be byte-reproducible, so both come out empty.

use printpdf::{BuiltinFont, Color, Mm};

use crate::config::Settings;
use crate::render::helpers::colors::BLACK;
use crate::render::helpers::fonts::FontManager;
use crate::render::helpers::layer::LayerBuilder;
use crate::render::helpers::text_metrics::BuiltinFontMeasurer;

/// Millimetres per point.
const PT: f32 = 0.3528;

/// Draws one document's page furniture.
pub struct PageFurniture<'a> {
    settings: &'a Settings,
    fonts: &'a FontManager,
}

impl<'a> PageFurniture<'a> {
    pub fn new(settings: &'a Settings, fonts: &'a FontManager) -> Self {
        Self { settings, fonts }
    }

    /// The font and size, in points, events are set in: `%Font:Event`, or
    /// Bridge Composer's 12pt roman when the file gives none.
    fn event_font(&self) -> (BuiltinFont, f32) {
        let spec = self.settings.fonts.event.as_ref();
        let set = self.fonts.builtin_set_for_spec(spec);
        match spec {
            Some(spec) => {
                let font = match (spec.is_bold(), spec.italic) {
                    (true, true) => set.bold_italic,
                    (true, false) => set.bold,
                    (false, true) => set.italic,
                    (false, false) => set.regular,
                };
                (font, spec.size)
            }
            None => (set.regular, 12.0),
        }
    }

    /// The page header: `event` centred in the top margin, its foot
    /// `%EventSpacing` points above the margin. Takes no room from the page.
    pub fn draw_header(&self, layer: &mut LayerBuilder, event: &str) {
        let (font, size) = self.event_font();
        let measurer = BuiltinFontMeasurer::new(font);
        let s = self.settings;
        let width = measurer.measure_width_mm(event, size);
        let x = s.margin_left + (s.content_width() - width) / 2.0;
        let margin_line = s.page_height - s.margin_top;
        let baseline = margin_line + (s.event_spacing_pt + 1.0) * PT + measurer.descender_mm(size);
        layer.set_fill_color(Color::Rgb(BLACK));
        layer.use_text_builtin(event, size, Mm(x), Mm(baseline), font);
    }

    /// A heading atop a column, or a one-board page: `event` centred over
    /// `x..x + width` below `top`. Returns the height it takes from the column.
    pub fn draw_heading(
        &self,
        layer: &mut LayerBuilder,
        event: &str,
        x: f32,
        width: f32,
        top: f32,
    ) -> f32 {
        let (font, size) = self.event_font();
        let measurer = BuiltinFontMeasurer::new(font);
        let text_width = measurer.measure_width_mm(event, size);
        let baseline = top - 0.9 * size * PT;
        layer.set_fill_color(Color::Rgb(BLACK));
        layer.use_text_builtin(
            event,
            size,
            Mm(x + (width - text_width) / 2.0),
            Mm(baseline),
            font,
        );
        1.18 * size * PT
    }

    /// Row 0 of the `%PageFooter` cells, for page `page` of `pages`.
    pub fn draw_footer(&self, layer: &mut LayerBuilder, page: usize, pages: usize) {
        let s = self.settings;
        let font = self
            .fonts
            .builtin_set_for_spec(s.fonts.hand_record.as_ref())
            .regular;
        let size = s.body_font_size;
        let measurer = BuiltinFontMeasurer::new(font);
        let first_baseline = s.margin_bottom - 1.5 * PT - measurer.ascender_mm(size);
        let right = s.page_width - s.margin_right;
        let centre = s.margin_left + s.content_width() / 2.0;

        layer.set_fill_color(Color::Rgb(BLACK));
        for cell in s.page_footers.iter().filter(|c| c.row == 0) {
            let text = expand(&cell.text, page, pages, s);
            for (i, line) in footer_lines(&text).enumerate() {
                if line.is_empty() {
                    continue;
                }
                let width = measurer.measure_width_mm(line, size);
                let x = match cell.column {
                    0 => s.margin_left,
                    1 => centre - width / 2.0,
                    _ => right - width,
                };
                let y = first_baseline - i as f32 * 1.2 * size * PT;
                layer.use_text_builtin(line, size, Mm(x), Mm(y), font);
            }
        }
    }
}

/// A footer cell's lines. The file escapes its line break, so `\\n` -- or a
/// lone `\n` -- separates them.
fn footer_lines(text: &str) -> impl Iterator<Item = &str> {
    text.split("\\\\n").flat_map(|part| part.split("\\n"))
}

/// A footer cell with its `%` tokens filled in.
pub fn expand(text: &str, page: usize, pages: usize, settings: &Settings) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('D') => {
                let date = settings.title_date.as_deref().and_then(spelled_date);
                out.push_str(&date.unwrap_or_default());
            }
            Some('s') => out.push_str(settings.title_site.as_deref().unwrap_or("")),
            Some('n') => out.push_str(&page.to_string()),
            Some('N') => out.push_str(&pages.to_string()),
            Some('e') => out.push_str(settings.title_from_metadata.as_deref().unwrap_or("")),
            Some('t') => out.push_str(settings.title_set_id.as_deref().unwrap_or("")),
            // The file's folder: not something a reproducible build can print
            Some('P') => {}
            Some(other) => out.push(other),
            None => out.push('%'),
        }
    }
    out
}

/// `%HRTitleDate`'s `2017.01.17` as Bridge Composer spells it: "Tuesday,
/// January 17, 2017". `None` for anything that is not a real date, `0`
/// included -- the value Bridge Composer writes when the title has no date.
pub fn spelled_date(date: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    const DAYS: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    let mut parts = date.trim().split('.');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: usize = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Sakamoto's day of the week, 0 = Sunday
    const OFFSETS: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let weekday = (y + y / 4 - y / 100 + y / 400 + OFFSETS[month - 1] + day as i32).rem_euclid(7);
    Some(format!(
        "{}, {} {}, {}",
        DAYS[weekday as usize],
        MONTHS[month - 1],
        day,
        year
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_spelt_as_bridge_composer_spells_them() {
        // Both from Bridge Composer's own footers
        assert_eq!(
            spelled_date("2017.01.17").as_deref(),
            Some("Tuesday, January 17, 2017")
        );
        assert_eq!(
            spelled_date("2023.12.10").as_deref(),
            Some("Sunday, December 10, 2023")
        );
        assert_eq!(spelled_date("0"), None, "no date: never today's");
        assert_eq!(spelled_date("2017.13.01"), None);
    }

    #[test]
    fn footer_tokens_expand() {
        let mut settings = Settings::default();
        settings.title_date = Some("2017.01.17".into());
        settings.title_site = Some("Stoneridge Creek".into());
        settings.title_from_metadata = Some("ABS3-3".into());
        assert_eq!(
            expand("%D|%s|%n of %N|%e|%t|%%|%P|%q", 6, 9, &settings),
            "Tuesday, January 17, 2017|Stoneridge Creek|6 of 9|ABS3-3||%||q"
        );
    }

    #[test]
    fn a_footer_cell_breaks_at_its_escaped_newlines() {
        let lines: Vec<_> = footer_lines("Presented by\\\\nGrant Robinson").collect();
        assert_eq!(lines, ["Presented by", "Grant Robinson"]);
    }
}
