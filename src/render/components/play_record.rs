//! The play-record table BridgeComposer draws for a board's `[Play]` section.
//!
//! Probed against BridgeComposer 5.118.2 (issue #42). The table is plain text,
//! with no rules and no shading:
//!
//! ```text
//! Trick   Lead    2nd   3rd   4th
//! 1. S    ♥ K     7     3     A
//! 2. E    ♦ 7     6     3     2
//! 3. E    ♠ A     2     5     7
//! 4. E    -       ♠ 9   -     -
//! ```
//!
//! Four things in that are load-bearing:
//!
//! - The seat beside the trick number is the trick's **leader**, not its
//!   winner. Rows two and three both read `E` because East won trick one and
//!   kept the lead.
//! - The winning card is **underlined** — `A` in row one, the lead itself in
//!   rows two and three.
//! - A card that follows the led suit shows a bare rank; one that does not,
//!   a discard or a ruff, keeps its suit symbol. That is why `♠ 9` carries one
//!   in row four, where no lead is on record to follow.
//! - A card not on record is a dash.

use printpdf::{BuiltinFont, Color, FontId, Mm, PaintMode};

use crate::config::Settings;
use crate::model::card::RankExt;
use crate::model::{Card, Direction, PlaySequence};
use crate::render::helpers::colors::{SuitColors, BLACK};
use crate::render::helpers::layer::LayerBuilder;
use crate::render::helpers::text_metrics;

/// Column pitch, as a multiple of the body font size. BridgeComposer sets a
/// 12pt record on a 48pt pitch.
const COLUMN_PITCH: f32 = 4.0;
/// Gap from the trick number to its leader's letter, likewise 12pt at 12pt.
const LEADER_OFFSET: f32 = 1.0;
/// Points to millimetres.
const PT_TO_MM: f32 = 25.4 / 72.0;

/// The column headings, the first of which carries the trick number and leader.
const HEADINGS: [&str; 5] = ["Trick", "Lead", "2nd", "3rd", "4th"];

pub struct PlayRecordRenderer<'a> {
    font: BuiltinFont,
    symbol_font: &'a FontId,
    colors: SuitColors,
    settings: &'a Settings,
}

impl<'a> PlayRecordRenderer<'a> {
    pub fn new(font: BuiltinFont, symbol_font: &'a FontId, settings: &'a Settings) -> Self {
        Self {
            font,
            symbol_font,
            colors: SuitColors::new(settings.black_color, settings.red_color),
            settings,
        }
    }

    /// The tricks worth tabulating: those with a card actually on record.
    ///
    /// A section closes with `*`, and a trick may hold nothing but placeholders,
    /// so the tail of `tricks` is often empty.
    fn rows(play: &PlaySequence) -> &[crate::model::Trick] {
        let last = play
            .tricks
            .iter()
            .rposition(|t| t.cards.iter().any(Option::is_some));
        match last {
            Some(i) => &play.tricks[..=i],
            None => &[],
        }
    }

    /// The height the table will take: a heading row and one row per trick.
    ///
    /// It needs no fonts, only the row count, so the layout can measure a board
    /// without building a renderer.
    pub fn height(settings: &Settings, play: &PlaySequence) -> f32 {
        let rows = Self::rows(play).len();
        if rows == 0 {
            return 0.0;
        }
        (rows + 1) as f32 * settings.line_height
    }

    fn column_pitch(&self) -> f32 {
        COLUMN_PITCH * self.settings.body_font_size * PT_TO_MM
    }

    /// Draw the table with its heading row's baseline at `y`. Returns the
    /// height used, which is what `height` predicted.
    pub fn render(&self, layer: &mut LayerBuilder, play: &PlaySequence, x: f32, y: f32) -> f32 {
        let rows = Self::rows(play);
        if rows.is_empty() {
            return 0.0;
        }
        let font_size = self.settings.body_font_size;
        let pitch = self.column_pitch();
        let line_height = self.settings.line_height;

        // Heading row
        layer.set_fill_color(Color::Rgb(BLACK));
        for (i, heading) in HEADINGS.iter().enumerate() {
            layer.use_text_builtin(
                *heading,
                font_size,
                Mm(x + i as f32 * pitch),
                Mm(y),
                self.font,
            );
        }

        for (i, trick) in rows.iter().enumerate() {
            let row_y = y - (i + 1) as f32 * line_height;

            // "1." and the leader's initial
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(
                format!("{}.", i + 1),
                font_size,
                Mm(x),
                Mm(row_y),
                self.font,
            );
            layer.use_text_builtin(
                seat_initial(trick.leader),
                font_size,
                Mm(x + LEADER_OFFSET * font_size * PT_TO_MM),
                Mm(row_y),
                self.font,
            );

            // The winner's card is underlined, and sits at its own slot
            let winning_slot = trick
                .winner
                .map(|w| (w.to_index() + 4 - trick.leader.to_index()) % 4);

            for slot in 0..4 {
                let col_x = x + (slot + 1) as f32 * pitch;
                let width = match trick.cards[slot] {
                    None => {
                        layer.set_fill_color(Color::Rgb(BLACK));
                        layer.use_text_builtin("-", font_size, Mm(col_x), Mm(row_y), self.font);
                        self.measure("-")
                    }
                    Some(card) => {
                        // The lead always shows its suit; a later card shows one
                        // only when it does not follow
                        let with_suit = slot == 0 || Some(card.suit) != trick.lead_suit;
                        self.draw_card(layer, &card, col_x, row_y, with_suit)
                    }
                };
                if winning_slot == Some(slot) {
                    self.underline(layer, col_x, row_y, width);
                }
            }
        }

        Self::height(self.settings, play)
    }

    fn measure(&self, text: &str) -> f32 {
        text_metrics::get_times_measurer().measure_width_mm(text, self.settings.body_font_size)
    }

    /// Draw a card, optionally with its suit symbol, and return its width.
    fn draw_card(
        &self,
        layer: &mut LayerBuilder,
        card: &Card,
        x: f32,
        y: f32,
        with_suit: bool,
    ) -> f32 {
        let font_size = self.settings.body_font_size;
        let mut cursor = x;
        if with_suit {
            let symbol = card.suit.symbol().to_string();
            layer.set_fill_color(Color::Rgb(self.colors.for_suit(&card.suit)));
            layer.use_text(&symbol, font_size, Mm(cursor), Mm(y), self.symbol_font);
            cursor += self.measure(&symbol);
        }
        let rank = card.rank.display_str().to_string();
        layer.set_fill_color(Color::Rgb(BLACK));
        layer.use_text_builtin(&rank, font_size, Mm(cursor), Mm(y), self.font);
        cursor + self.measure(&rank) - x
    }

    /// A rule just under the baseline, the width of the card above it.
    fn underline(&self, layer: &mut LayerBuilder, x: f32, baseline: f32, width: f32) {
        let drop = self.settings.body_font_size * PT_TO_MM * 0.18;
        layer.set_outline_color(Color::Rgb(BLACK));
        layer.set_outline_thickness(0.3);
        layer.add_rect(
            Mm(x),
            Mm(baseline - drop),
            Mm(x + width),
            Mm(baseline - drop),
            PaintMode::Stroke,
        );
    }
}

/// The letter BridgeComposer writes beside a trick number.
fn seat_initial(direction: Direction) -> &'static str {
    match direction {
        Direction::North => "N",
        Direction::East => "E",
        Direction::South => "S",
        Direction::West => "W",
    }
}
