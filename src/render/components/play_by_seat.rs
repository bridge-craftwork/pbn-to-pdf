//! The play record by seat: one row per trick and one column per player,
//! as bridge-solver's play trace (`web/src/components/PlayTrace.vue`) draws
//! it. `--play-by-seat` puts it in place of BridgeComposer's table.
//!
//! ```text
//!         W     Dummy    E     Declarer   Tricks
//!    1   ▶♥K    ♥3      ♥7    [♥A]         1–0
//!    2    ♦2    ♦3     ▶♦7     ♦6 (-1)     1–1
//! ```
//!
//! BridgeComposer's columns run in play order, so a seat wanders from column
//! to column as the lead changes, and following one player's cards down the
//! hand means finding them again on every row. Here each seat keeps its
//! column, and the play order comes back as marks: the leader's cell is
//! tinted blue with a bar and arrow down its left edge, the winner's green.
//! The columns run clockwise from the opening leader, which puts declarer
//! last and dummy between the defenders.
//!
//! A `[DoubleDummyPlay]` section adds each card's cost. A card that gave
//! tricks away carries a badge, signed from declarer's side as the solver
//! signs it: `-1` red for a trick declarer lost, `+1` green for one the
//! defence gave back. The card itself is tinted amber when another card of
//! the same suit was right, and red when the suit was wrong. A summary line
//! above the table totals the errors; a legend below says what the marks
//! mean.

use printpdf::{BuiltinFont, Color, FontId, Mm, PaintMode, Rgb};

use crate::config::Settings;
use crate::model::card::RankExt;
use crate::model::{Board, Card, Direction, Trick};
use crate::render::components::play_record::PlayRecordRenderer;
use crate::render::helpers::colors::{SuitColors, BLACK};
use crate::render::helpers::layer::LayerBuilder;
use crate::render::helpers::text_metrics;
use bridge_types::{CardCosts, DoubleDummyPlay};

/// Points to millimetres.
const PT_TO_MM: f32 = 25.4 / 72.0;

/// Column widths in ems of the body font: the trick number, each seat at
/// most, and the running score.
const NUMBER_WIDTH: f32 = 2.0;
const SEAT_WIDTH: f32 = 4.4;
const RUNNING_WIDTH: f32 = 3.6;
/// The narrowest a seat column gets in a tight column of the page.
const MIN_SEAT_WIDTH: f32 = 3.4;
/// Row pitch as a multiple of the line height, for room between the tints.
const ROW_PITCH: f32 = 1.15;
/// Headings, scores and the legend, as a fraction of the body font.
const SMALL: f32 = 0.78;
/// The cost badge's text, likewise.
const BADGE: f32 = 0.62;

// The solver's colours, its translucent ones laid over white.
const LEAD_TINT: Rgb = rgb(232, 238, 245);
const LEAD_MARK: Rgb = rgb(91, 134, 181);
const WON_TINT: Rgb = rgb(212, 237, 218);
const WRONG_CARD: Rgb = rgb(255, 229, 122);
const WRONG_SUIT: Rgb = rgb(244, 204, 204);
const WRONG_SUIT_SEVERE: Rgb = rgb(231, 165, 165);
const LOST: Rgb = rgb(198, 40, 40);
const GAINED: Rgb = rgb(29, 122, 63);
const CLEAN: Rgb = rgb(29, 158, 117);
const MUTED: Rgb = rgb(102, 102, 102);
const WHITE: Rgb = rgb(255, 255, 255);

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        icc_profile: None,
    }
}

/// What kind of mistake a costly card was, as the solver judges it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// Another card of the same suit kept the best result.
    Card,
    /// No card of that suit did.
    Suit,
}

/// A played card's double-dummy cost and what kind of mistake it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cost {
    tricks: u8,
    verdict: Verdict,
}

fn cost_of(costs: &CardCosts, card: Card) -> Option<Cost> {
    let tricks = costs.cost_of(card)?;
    let in_suit_was_right = costs
        .cards
        .iter()
        .any(|(c, cost)| c.suit == card.suit && *cost == Some(0));
    Some(Cost {
        tricks,
        verdict: if in_suit_was_right {
            Verdict::Card
        } else {
            Verdict::Suit
        },
    })
}

pub struct SeatPlayRenderer<'a> {
    font: BuiltinFont,
    symbol_font: &'a FontId,
    colors: SuitColors,
    settings: &'a Settings,
}

impl<'a> SeatPlayRenderer<'a> {
    pub fn new(font: BuiltinFont, symbol_font: &'a FontId, settings: &'a Settings) -> Self {
        Self {
            font,
            symbol_font,
            colors: SuitColors::new(settings.black_color, settings.red_color),
            settings,
        }
    }

    /// The height `board`'s record will take in `width`: a summary line when
    /// there are costs, the headings, the tricks and the legend's lines.
    pub fn height(settings: &Settings, board: &Board, width: f32) -> f32 {
        let Some(play) = board.play.as_ref() else {
            return 0.0;
        };
        let rows = PlayRecordRenderer::rows(play).len();
        if rows == 0 {
            return 0.0;
        }
        let summary = if board.double_dummy_play.is_some() {
            1.0
        } else {
            0.0
        };
        let legend =
            legend_rows(settings, &legend_widths(settings, with_costs(board)), width).len();
        (summary + 1.0 + legend as f32 + rows as f32 * ROW_PITCH) * settings.line_height
    }

    fn em(&self) -> f32 {
        self.settings.body_font_size * PT_TO_MM
    }

    /// Draw `board`'s record with its first line's baseline at `y`, within
    /// `width`. Returns the height used, which is what `height` predicted.
    pub fn render(
        &self,
        layer: &mut LayerBuilder,
        board: &Board,
        x: f32,
        y: f32,
        width: f32,
    ) -> f32 {
        let Some(play) = board.play.as_ref() else {
            return 0.0;
        };
        let dd = board.double_dummy_play.as_ref();
        let declarer = board.declarer;
        let rows = PlayRecordRenderer::rows(play);
        if rows.is_empty() {
            return 0.0;
        }
        let em = self.em();
        let line = self.settings.line_height;
        let pitch = ROW_PITCH * line;
        let seat_width = ((width / em - NUMBER_WIDTH - RUNNING_WIDTH) / 4.0)
            .clamp(MIN_SEAT_WIDTH, SEAT_WIDTH)
            * em;
        let seats_x = x + NUMBER_WIDTH * em;
        let running_x = seats_x + 4.0 * seat_width + RUNNING_WIDTH * em / 2.0;
        let seats: Vec<Direction> = (0..4)
            .scan(play.opening_leader, |seat, _| {
                let this = *seat;
                *seat = seat.next();
                Some(this)
            })
            .collect();
        let declaring =
            |seat: Direction| declarer.is_some_and(|d| seat == d || seat == d.partner());

        let mut baseline = y;
        if let Some(dd) = dd {
            self.summary(layer, rows, dd, x, baseline);
            baseline -= line;
        }

        // Headings
        let small = self.settings.body_font_size * SMALL;
        layer.set_fill_color(Color::Rgb(MUTED));
        for (i, seat) in seats.iter().enumerate() {
            let label = match declarer {
                Some(d) if *seat == d => "Declarer".to_string(),
                Some(d) if *seat == d.partner() => "Dummy".to_string(),
                _ => seat.to_char().to_string(),
            };
            let centre = seats_x + (i as f32 + 0.5) * seat_width;
            self.centred(layer, &label, small, centre, baseline);
        }
        let score_heading = if declarer.is_some() {
            "Tricks"
        } else {
            "NS–EW"
        };
        self.centred(layer, score_heading, small, running_x, baseline);

        let (mut ours, mut theirs) = (0, 0);
        for (i, trick) in rows.iter().enumerate() {
            let row_y = baseline - (i + 1) as f32 * pitch;
            // The text's middle, which the tints and badges centre on
            let middle = row_y + 0.33 * em;
            let band = (middle - pitch / 2.0 + 0.3, middle + pitch / 2.0 - 0.3);

            layer.set_fill_color(Color::Rgb(MUTED));
            let number = format!("{}", i + 1);
            let number_width = self.measure(&number, small);
            layer.use_text_builtin(
                number,
                small,
                Mm(x + NUMBER_WIDTH * em * 0.6 - number_width),
                Mm(row_y),
                self.font,
            );

            for (column, seat) in seats.iter().enumerate() {
                let left = seats_x + column as f32 * seat_width + 0.35;
                let right = seats_x + (column + 1) as f32 * seat_width - 0.35;
                self.cell_marks(layer, trick, *seat, left, right, band, middle);

                let Some(card) = trick.card_by(*seat) else {
                    continue;
                };
                let cost = dd
                    .and_then(|dd| dd.turn(i, *seat))
                    .and_then(|costs| cost_of(costs, card));
                self.card_with_cost(
                    layer,
                    card,
                    cost.filter(|c| c.tricks > 0),
                    declarer.map(|_| declaring(*seat)),
                    (left + right) / 2.0,
                    row_y,
                    middle,
                );
            }

            if let Some(winner) = trick.winner {
                let first_side = match declarer {
                    Some(_) => declaring(winner),
                    None => matches!(winner, Direction::North | Direction::South),
                };
                if first_side {
                    ours += 1;
                } else {
                    theirs += 1;
                }
                layer.set_fill_color(Color::Rgb(MUTED));
                self.centred(layer, &format!("{ours}–{theirs}"), small, running_x, row_y);
            }
        }

        let legend_y = baseline - rows.len() as f32 * pitch - line;
        self.legend(layer, with_costs(board), x, legend_y, width);

        Self::height(self.settings, board, width)
    }

    /// "2 costed errors, 3 tricks given away", or "No costed errors".
    fn summary(
        &self,
        layer: &mut LayerBuilder,
        rows: &[Trick],
        dd: &DoubleDummyPlay,
        x: f32,
        y: f32,
    ) {
        let mut errors = 0;
        let mut cost = 0u32;
        for (i, trick) in rows.iter().enumerate() {
            for seat in Direction::ALL {
                let lost = trick
                    .card_by(seat)
                    .zip(dd.turn(i, seat))
                    .and_then(|(card, costs)| costs.cost_of(card))
                    .unwrap_or(0);
                if lost > 0 {
                    errors += 1;
                    cost += u32::from(lost);
                }
            }
        }
        let size = self.settings.body_font_size * SMALL;
        let (text, color) = if errors == 0 {
            ("No costed errors".to_string(), CLEAN)
        } else {
            let s = |n: u32| if n == 1 { "" } else { "s" };
            (
                format!(
                    "{errors} costed error{}, {cost} trick{} given away",
                    s(errors),
                    s(cost)
                ),
                LOST,
            )
        };
        layer.set_fill_color(Color::Rgb(color));
        layer.use_text_builtin(text, size, Mm(x), Mm(y), self.font);
    }

    /// The leader's tint, bar and arrow, or the winner's tint.
    ///
    /// Whoever wins a trick leads the next, so leading and winning the same
    /// trick is common. The winner's tint takes the cell; the bar and arrow
    /// still say who led.
    #[allow(clippy::too_many_arguments)]
    fn cell_marks(
        &self,
        layer: &mut LayerBuilder,
        trick: &Trick,
        seat: Direction,
        left: f32,
        right: f32,
        (bottom, top): (f32, f32),
        middle: f32,
    ) {
        let led = trick.leader == seat;
        let won = trick.winner == Some(seat);
        if !led && !won {
            return;
        }
        let tint = if won { WON_TINT } else { LEAD_TINT };
        layer.set_fill_color(Color::Rgb(tint));
        layer.add_rect(Mm(left), Mm(bottom), Mm(right), Mm(top), PaintMode::Fill);
        if led {
            self.lead_mark(layer, left, bottom, top, middle);
        }
    }

    /// A bar down the cell's left edge and an arrow pointing in.
    fn lead_mark(&self, layer: &mut LayerBuilder, left: f32, bottom: f32, top: f32, middle: f32) {
        let bar = 0.55;
        let arrow_half = ((top - bottom) * 0.3).min(1.6);
        layer.set_fill_color(Color::Rgb(LEAD_MARK));
        layer.add_rect(
            Mm(left),
            Mm(bottom),
            Mm(left + bar),
            Mm(top),
            PaintMode::Fill,
        );
        layer.add_polygon(
            &[
                (Mm(left + bar), Mm(middle + arrow_half)),
                (Mm(left + bar), Mm(middle - arrow_half)),
                (Mm(left + bar + arrow_half * 0.65), Mm(middle)),
            ],
            PaintMode::Fill,
        );
    }

    /// A card centred on `centre`, with its mistake tint and cost badge.
    ///
    /// `declaring` is whether the player is on declarer's side, which signs
    /// the badge; `None` when there is no declarer to sign it from.
    #[allow(clippy::too_many_arguments)]
    fn card_with_cost(
        &self,
        layer: &mut LayerBuilder,
        card: Card,
        cost: Option<Cost>,
        declaring: Option<bool>,
        centre: f32,
        baseline: f32,
        middle: f32,
    ) {
        let size = self.settings.body_font_size;
        let em = self.em();
        let symbol = card.suit.symbol().to_string();
        let rank = card.rank.display_str().to_string();
        let card_width = self.measure(&symbol, size) + self.measure(&rank, size);

        let badge = cost.map(|cost| {
            let text = match declaring {
                Some(true) => format!("-{}", cost.tricks),
                Some(false) => format!("+{}", cost.tricks),
                None => cost.tricks.to_string(),
            };
            let color = if declaring == Some(false) {
                GAINED
            } else {
                LOST
            };
            let badge_size = size * BADGE;
            let text_width =
                text_metrics::get_helvetica_bold_measurer().measure_width_mm(&text, badge_size);
            let height = 0.82 * em;
            let width = (text_width + 0.5 * height).max(height);
            (text, color, badge_size, text_width, width, height)
        });
        let gap = 0.2 * em;
        let total = card_width + badge.as_ref().map_or(0.0, |b| gap + b.4);
        let card_x = centre - total / 2.0;

        if let Some(cost) = cost {
            let tint = match cost.verdict {
                Verdict::Card => WRONG_CARD,
                Verdict::Suit if cost.tricks >= 2 => WRONG_SUIT_SEVERE,
                Verdict::Suit => WRONG_SUIT,
            };
            let pad = 0.18 * em;
            layer.set_fill_color(Color::Rgb(tint));
            layer.add_rect(
                Mm(card_x - pad),
                Mm(middle - 0.5 * em),
                Mm(card_x + card_width + pad),
                Mm(middle + 0.5 * em),
                PaintMode::Fill,
            );
        }

        layer.set_fill_color(Color::Rgb(self.colors.for_suit(&card.suit)));
        layer.use_text(&symbol, size, Mm(card_x), Mm(baseline), self.symbol_font);
        layer.set_fill_color(Color::Rgb(BLACK));
        layer.use_text_builtin(
            &rank,
            size,
            Mm(card_x + self.measure(&symbol, size)),
            Mm(baseline),
            self.font,
        );

        if let Some((text, color, badge_size, text_width, width, height)) = badge {
            let left = card_x + card_width + gap;
            self.pill(
                layer, &text, color, badge_size, text_width, left, width, height, middle,
            );
        }
    }

    /// White text on a rounded badge.
    #[allow(clippy::too_many_arguments)]
    fn pill(
        &self,
        layer: &mut LayerBuilder,
        text: &str,
        color: Rgb,
        size: f32,
        text_width: f32,
        left: f32,
        width: f32,
        height: f32,
        middle: f32,
    ) {
        let r = height / 2.0;
        layer.set_fill_color(Color::Rgb(color));
        layer.add_circle(Mm(left + r), Mm(middle), Mm(r), PaintMode::Fill);
        layer.add_circle(Mm(left + width - r), Mm(middle), Mm(r), PaintMode::Fill);
        layer.add_rect(
            Mm(left + r),
            Mm(middle - r),
            Mm(left + width - r),
            Mm(middle + r),
            PaintMode::Fill,
        );
        let cap = text_metrics::get_helvetica_bold_measurer().cap_height_mm(size);
        layer.set_fill_color(Color::Rgb(WHITE));
        layer.use_text_builtin(
            text,
            size,
            Mm(left + (width - text_width) / 2.0),
            Mm(middle - cap / 2.0),
            BuiltinFont::HelveticaBold,
        );
    }

    /// What the marks mean, since colour alone would not say, wrapped to
    /// `width`.
    fn legend(&self, layer: &mut LayerBuilder, with_costs: bool, x: f32, y: f32, width: f32) {
        let size = self.settings.body_font_size * SMALL;
        let em = size * PT_TO_MM;
        let swatch = 0.85 * em;
        let badge_size = self.settings.body_font_size * BADGE;
        let badge_height = 0.82 * self.em();
        let items = legend_items(with_costs);
        let widths = legend_widths(self.settings, with_costs);

        for (row, range) in legend_rows(self.settings, &widths, width)
            .into_iter()
            .enumerate()
        {
            let baseline = y - row as f32 * self.settings.line_height;
            let middle = baseline + 0.33 * em;
            let (bottom, top) = (middle - swatch / 2.0, middle + swatch / 2.0);
            let mut cursor = x;
            for (mark, text) in &items[range] {
                match mark {
                    Mark::Led | Mark::Won => {
                        let tint = if *mark == Mark::Led {
                            LEAD_TINT
                        } else {
                            WON_TINT
                        };
                        layer.set_fill_color(Color::Rgb(tint));
                        layer.add_rect(
                            Mm(cursor),
                            Mm(bottom),
                            Mm(cursor + swatch),
                            Mm(top),
                            PaintMode::Fill,
                        );
                        if *mark == Mark::Led {
                            self.lead_mark(layer, cursor, bottom, top, middle);
                        }
                        cursor += swatch;
                    }
                    Mark::Badge(badge, color) => {
                        let text_width = text_metrics::get_helvetica_bold_measurer()
                            .measure_width_mm(badge, badge_size);
                        let pill = (text_width + 0.5 * badge_height).max(badge_height);
                        self.pill(
                            layer,
                            badge,
                            color.clone(),
                            badge_size,
                            text_width,
                            cursor,
                            pill,
                            badge_height,
                            middle,
                        );
                        cursor += pill;
                    }
                }
                cursor += LEGEND_MARK_GAP * em;
                layer.set_fill_color(Color::Rgb(MUTED));
                layer.use_text_builtin(*text, size, Mm(cursor), Mm(baseline), self.font);
                cursor += self.measure(text, size) + LEGEND_GAP * em;
            }
        }
    }

    fn centred(&self, layer: &mut LayerBuilder, text: &str, size: f32, centre: f32, y: f32) {
        let width = self.measure(text, size);
        layer.use_text_builtin(text, size, Mm(centre - width / 2.0), Mm(y), self.font);
    }

    fn measure(&self, text: &str, size: f32) -> f32 {
        text_metrics::get_times_measurer().measure_width_mm(text, size)
    }
}

/// Whether the legend explains the cost badges: only when there are costs,
/// and a declarer to sign them from.
fn with_costs(board: &Board) -> bool {
    board.double_dummy_play.is_some() && board.declarer.is_some()
}

/// A legend entry's mark.
#[derive(Debug, Clone, PartialEq)]
enum Mark {
    Led,
    Won,
    Badge(&'static str, Rgb),
}

/// Space between a legend mark and its text, and between entries, in ems of
/// the legend's font.
const LEGEND_MARK_GAP: f32 = 0.3;
const LEGEND_GAP: f32 = 1.1;

fn legend_items(with_costs: bool) -> Vec<(Mark, &'static str)> {
    let mut items = vec![(Mark::Led, "led"), (Mark::Won, "won the trick")];
    if with_costs {
        items.push((Mark::Badge("-1", LOST), "declarer lost a trick"));
        items.push((Mark::Badge("+1", GAINED), "defence gave one back"));
    }
    items
}

/// Each legend entry's width, mark and text, without the gap after it.
fn legend_widths(settings: &Settings, with_costs: bool) -> Vec<f32> {
    let size = settings.body_font_size * SMALL;
    let em = size * PT_TO_MM;
    let badge_height = 0.82 * settings.body_font_size * PT_TO_MM;
    let times = text_metrics::get_times_measurer();
    legend_items(with_costs)
        .iter()
        .map(|(mark, text)| {
            let mark = match mark {
                Mark::Led | Mark::Won => 0.85 * em,
                Mark::Badge(badge, _) => {
                    let w = text_metrics::get_helvetica_bold_measurer()
                        .measure_width_mm(badge, settings.body_font_size * BADGE);
                    (w + 0.5 * badge_height).max(badge_height)
                }
            };
            mark + LEGEND_MARK_GAP * em + times.measure_width_mm(text, size)
        })
        .collect()
}

/// The legend entries on each line, filling lines of `width` in turn.
fn legend_rows(settings: &Settings, widths: &[f32], width: f32) -> Vec<std::ops::Range<usize>> {
    let gap = LEGEND_GAP * settings.body_font_size * SMALL * PT_TO_MM;
    let mut rows = Vec::new();
    let mut start = 0;
    let mut used = 0.0;
    for (i, w) in widths.iter().enumerate() {
        if i > start && used + w > width {
            rows.push(start..i);
            start = i;
            used = 0.0;
        }
        used += w + gap;
    }
    rows.push(start..widths.len());
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Rank, Suit};

    fn card(suit: Suit, rank: Rank) -> Card {
        Card::new(suit, rank)
    }

    #[test]
    fn a_wrong_card_in_the_right_suit_is_a_card_mistake() {
        let costs = CardCosts {
            cards: vec![
                (card(Suit::Hearts, Rank::Ace), Some(0)),
                (card(Suit::Hearts, Rank::Two), Some(1)),
                (card(Suit::Clubs, Rank::Three), Some(2)),
            ],
        };
        assert_eq!(
            cost_of(&costs, card(Suit::Hearts, Rank::Two)),
            Some(Cost {
                tricks: 1,
                verdict: Verdict::Card
            })
        );
        assert_eq!(
            cost_of(&costs, card(Suit::Clubs, Rank::Three)),
            Some(Cost {
                tricks: 2,
                verdict: Verdict::Suit
            })
        );
        assert_eq!(cost_of(&costs, card(Suit::Spades, Rank::Two)), None);
    }

    #[test]
    fn the_legend_wraps_to_the_column() {
        let settings = Settings::default();
        let widths = legend_widths(&settings, true);
        assert_eq!(legend_rows(&settings, &widths, 200.0), vec![0..4]);
        // Just short of all four with no gaps: the last goes to a second line
        let total: f32 = widths.iter().sum();
        assert_eq!(
            legend_rows(&settings, &widths, total - 0.1),
            vec![0..3, 3..4]
        );
        // An entry wider than the column still gets a line of its own
        assert_eq!(legend_rows(&settings, &widths, 1.0).len(), 4);
    }
}
