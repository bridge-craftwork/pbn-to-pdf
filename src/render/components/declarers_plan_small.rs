//! Declarer's plan small component
//!
//! Renders a compact layout for one quadrant of a page showing:
//! - Header line: Deal # (left), Contract (center), Goal (right)
//! - North hand in dummy view
//! - South hand in fan view
//! - Winners or Losers table (below south hand)
//!   - NT contracts: Winners table
//!   - Suit contracts: Losers table

use printpdf::{BuiltinFont, Color, FontId, Mm, PaintMode, Rgb};
use std::collections::HashMap;

use crate::model::card::RankExt;
use crate::model::{BidSuit, Board, Card, Hand, Suit};
use crate::render::components::{
    DummyRenderer, FanRenderer, LosersTableRenderer, WinnersTableRenderer,
};
use crate::render::helpers::card_assets::{CardAssets, CardFace, CARD_HEIGHT_MM};
use crate::render::helpers::colors::{SuitColors, BLACK};
use crate::render::helpers::layer::LayerBuilder;
use crate::render::helpers::text_metrics;

/// Gap between elements in mm
const ELEMENT_GAP: f32 = 4.0;

/// Portion of fan height to display (crop from bottom)
const FAN_CROP_RATIO: f32 = 0.5;

/// Nominal number of cards in a suit for calculating fixed dummy height
const NOMINAL_SUIT_LENGTH: usize = 5;

/// Font size for header text (increased for visibility)
const HEADER_FONT_SIZE: f32 = 14.0;
/// Gap between "Deal N" and the contract that follows it.
const DEAL_CONTRACT_GAP: f32 = 2.0;
/// Smallest gap allowed between the contract and the right-justified goal text.
const HEADER_MIN_GAP: f32 = 2.0;
/// Floor on the header's autofit, as a fraction of HEADER_FONT_SIZE.
const HEADER_MIN_SHRINK: f32 = 0.75;
const CONTRACT_LABEL: &str = "Ctr: ";

/// Font size at which the header's two ends fit `available` with `min_gap`
/// between them, given their widths measured at `base`.
///
/// Text width is proportional to font size, so a single step lands on a fit.
/// The result is floored at `HEADER_MIN_SHRINK` of `base`: a header shrunk far
/// enough to read as a different size is worse than one that touches the edge.
fn fit_header_font_size(
    base: f32,
    left_width: f32,
    goal_width: f32,
    min_gap: f32,
    available: f32,
) -> f32 {
    let needed = left_width + min_gap + goal_width;
    if needed > available && needed > 0.0 && available > 0.0 {
        (base * (available / needed)).max(base * HEADER_MIN_SHRINK)
    } else {
        base
    }
}

/// Height of the header line area
const HEADER_HEIGHT: f32 = 8.0;

/// Extra space to raise dummy (one line height)
const DUMMY_RAISE: f32 = 1.0;

/// Extra space to raise fan (~1 inch minus header savings)
const FAN_RAISE: f32 = 21.4;

/// Extra space to raise table (about 1.5x title row height minus header savings + row height)
const TABLE_RAISE: f32 = 11.5;

/// Font size for opening lead box (larger for visibility)
const LEAD_BOX_FONT_SIZE: f32 = 12.5;

/// Mild yellow background color for opening lead box
const MILD_YELLOW: Rgb = Rgb {
    r: 1.0,
    g: 1.0,
    b: 0.7,
    icc_profile: None,
};

/// Renderer for a small declarer's plan layout (one quadrant of a page)
pub struct DeclarersPlanSmallRenderer<'a> {
    card_assets: &'a CardAssets,
    font: BuiltinFont,
    bold_font: BuiltinFont,
    symbol_font: &'a FontId,
    colors: SuitColors,
    /// Scale factor for card rendering
    card_scale: f32,
    /// Scale factor for non-card elements (text, tables, spacing)
    layout_scale: f32,
    /// Arc degrees for the fan display
    fan_arc: f32,
    /// Overlap ratio for dummy display
    dummy_overlap: f32,
    /// Whether to show debug bounding boxes
    show_bounds: bool,
    /// Cards to circle (highlight) with their colors
    circled_cards: HashMap<Card, Rgb>,
}

impl<'a> DeclarersPlanSmallRenderer<'a> {
    /// Create a new declarer's plan small renderer
    pub fn new(
        card_assets: &'a CardAssets,
        font: BuiltinFont,
        bold_font: BuiltinFont,
        symbol_font: &'a FontId,
        colors: SuitColors,
    ) -> Self {
        Self {
            card_assets,
            font,
            bold_font,
            symbol_font,
            colors,
            card_scale: 0.35,
            layout_scale: 1.0,
            fan_arc: 30.0,
            dummy_overlap: 0.18, // Show some suit symbol on clipped cards
            show_bounds: false,
            circled_cards: HashMap::new(),
        }
    }

    /// Set the card scale factor
    pub fn card_scale(mut self, scale: f32) -> Self {
        self.card_scale = scale;
        self
    }

    /// Set the layout scale factor for non-card elements (text, tables, spacing)
    pub fn layout_scale(mut self, scale: f32) -> Self {
        self.layout_scale = scale;
        self
    }

    /// Set the fan arc in degrees
    pub fn fan_arc(mut self, arc: f32) -> Self {
        self.fan_arc = arc;
        self
    }

    /// Set the dummy overlap ratio
    pub fn dummy_overlap(mut self, overlap: f32) -> Self {
        self.dummy_overlap = overlap;
        self
    }

    /// Set whether to show debug bounding boxes
    pub fn show_bounds(mut self, show: bool) -> Self {
        self.show_bounds = show;
        self
    }

    /// Set which cards should be circled (highlighted) with their colors
    ///
    /// The ellipse appears around the rank/suit indicator in the top-left corner of the card.
    /// Cards can be in either the dummy (north) or declarer (south) hand.
    pub fn circled_cards(mut self, cards: HashMap<Card, Rgb>) -> Self {
        self.circled_cards = cards;
        self
    }

    /// Add a single card to circle with the default color (red)
    pub fn circle_card(mut self, card: Card) -> Self {
        use crate::render::helpers::colors::RED;
        self.circled_cards.insert(card, RED);
        self
    }

    /// Add a single card to circle with a specific color
    pub fn circle_card_with_color(mut self, card: Card, color: Rgb) -> Self {
        self.circled_cards.insert(card, color);
        self
    }

    /// Get the dummy renderer configured for this layout
    ///
    /// Filters circled_cards to only include cards that are in the given hand.
    fn dummy_renderer(&self, trump: Option<BidSuit>, hand: &Hand) -> DummyRenderer<'a> {
        let first_suit = Self::first_suit_for_trump(trump);
        let hand_circled: HashMap<Card, Rgb> = self
            .circled_cards
            .iter()
            .filter(|(card, _)| hand.contains(card.suit, card.rank))
            .map(|(card, color)| (*card, color.clone()))
            .collect();
        DummyRenderer::with_overlap(self.card_assets, self.card_scale, self.dummy_overlap)
            .first_suit(first_suit)
            .show_bounds(self.show_bounds)
            .circled_cards(hand_circled)
    }

    /// Every card rendition this panel draws for one board.
    ///
    /// The layouts use it to register only the XObjects a document actually
    /// needs: printpdf writes every XObject added to a document whether or not
    /// a page invokes it, and the full court-card illustrations are ~3.9MB.
    /// It delegates to the same `faces` helpers the renderers draw from, so
    /// the set can never fall out of step with what is drawn.
    pub fn required_faces(
        dummy: &Hand,
        declarer: &Hand,
        trump: Option<BidSuit>,
    ) -> Vec<(Suit, crate::model::Rank, CardFace)> {
        let first_suit = Self::first_suit_for_trump(trump);
        let mut faces = DummyRenderer::faces(dummy, first_suit, true);
        faces.extend(FanRenderer::faces(declarer, first_suit, true));
        faces
    }

    /// Determine the first suit based on trump suit
    /// - Suit contracts: trump suit first
    /// - NT contracts: Clubs first
    fn first_suit_for_trump(trump: Option<BidSuit>) -> Suit {
        match trump {
            Some(BidSuit::Spades) => Suit::Spades,
            Some(BidSuit::Hearts) => Suit::Hearts,
            Some(BidSuit::Diamonds) => Suit::Diamonds,
            Some(BidSuit::Clubs) => Suit::Clubs,
            Some(BidSuit::NoTrump) | None => Suit::Clubs,
        }
    }

    /// Calculate the nominal dummy height based on a 5-card suit
    /// This provides consistent positioning regardless of actual hand shape
    fn nominal_dummy_height(&self) -> f32 {
        let card_height = CARD_HEIGHT_MM * self.card_scale;
        let visible_height = card_height * self.dummy_overlap;
        // Height: one full card + overlapped portions for (NOMINAL_SUIT_LENGTH - 1) cards
        card_height + (NOMINAL_SUIT_LENGTH - 1) as f32 * visible_height
    }

    /// Get the fan renderer configured for this layout, scaled to match dummy width
    ///
    /// Filters circled_cards to only include cards that are in the given hand.
    fn fan_renderer(
        &self,
        target_width: f32,
        hand: &Hand,
        trump: Option<BidSuit>,
    ) -> FanRenderer<'a> {
        let first_suit = Self::first_suit_for_trump(trump);
        // Calculate scale to match target width
        let temp_renderer = FanRenderer::new(self.card_assets, 1.0)
            .arc(self.fan_arc)
            .first_suit(first_suit);
        let (temp_width, _) = temp_renderer.dimensions(hand);
        let scale = if temp_width > 0.0 {
            target_width / temp_width
        } else {
            self.card_scale
        };

        let hand_circled: HashMap<Card, Rgb> = self
            .circled_cards
            .iter()
            .filter(|(card, _)| hand.contains(card.suit, card.rank))
            .map(|(card, color)| (*card, color.clone()))
            .collect();

        FanRenderer::new(self.card_assets, scale)
            .arc(self.fan_arc)
            .first_suit(first_suit)
            .show_bounds(self.show_bounds)
            .circled_cards(hand_circled)
    }

    /// Calculate dimensions needed for the layout
    ///
    /// Returns (width, height) in mm.
    /// Uses nominal dummy height (5-card suit) for consistent positioning.
    pub fn dimensions(&self, north: &Hand, south: &Hand, is_nt: bool) -> (f32, f32) {
        let s = self.layout_scale;

        // Use None for trump since dimensions don't depend on suit order
        let dummy_renderer = self.dummy_renderer(None, north);
        let (dummy_width, _) = dummy_renderer.dimensions(north);

        // Use nominal height for consistent layout
        let nominal_dummy_height = self.nominal_dummy_height();

        let fan_renderer = self.fan_renderer(dummy_width, south, None);
        let (_, fan_height) = fan_renderer.dimensions(south);
        // Only count the visible (cropped) portion of the fan
        let visible_fan_height = fan_height * FAN_CROP_RATIO;

        // Table dimensions - both tables have the same dimensions
        let (table_width, table_height) = if is_nt {
            self.winners_table_renderer().dimensions()
        } else {
            self.losers_table_renderer().dimensions()
        };

        // Width is just the content area
        let width = dummy_width.max(table_width);

        // Total height: header + gap + dummy + gap + visible fan + gap + table
        let height = HEADER_HEIGHT * s
            + ELEMENT_GAP * s
            + nominal_dummy_height
            + ELEMENT_GAP * s
            + visible_fan_height
            + ELEMENT_GAP * s
            + table_height;

        (width, height)
    }

    /// Create the winners table renderer, scaled by layout_scale
    fn winners_table_renderer(&self) -> WinnersTableRenderer<'a> {
        let s = self.layout_scale;
        WinnersTableRenderer::new(
            self.font,
            self.bold_font,
            self.symbol_font,
            self.colors.clone(),
        )
        .font_sizes(14.0 * s, 12.0 * s)
        .col_width(16.0 * s)
        .row_height(8.0 * s)
        .header_height(6.0 * s)
    }

    /// Create the losers table renderer, scaled by layout_scale
    fn losers_table_renderer(&self) -> LosersTableRenderer<'a> {
        let s = self.layout_scale;
        LosersTableRenderer::new(
            self.font,
            self.bold_font,
            self.symbol_font,
            self.colors.clone(),
        )
        .font_sizes(14.0 * s, 12.0 * s)
        .col_width(16.0 * s)
        .row_height(8.0 * s)
        .header_height(6.0 * s)
    }

    /// Render the declarer's plan layout from a Board
    ///
    /// Origin is the top-left corner of the display area.
    /// Returns the height used.
    pub fn render_board(&self, layer: &mut LayerBuilder, board: &Board, origin: (Mm, Mm)) -> f32 {
        let north = &board.deal.north;
        let south = &board.deal.south;

        // Determine if NT contract
        let is_nt = board
            .contract
            .as_ref()
            .map(|c| c.suit == crate::model::BidSuit::NoTrump)
            .unwrap_or(false);

        // Get opening lead if play sequence exists
        let opening_lead = board
            .play
            .as_ref()
            .and_then(|play| play.tricks.first().and_then(|trick| trick.cards[0]));

        self.render(layer, north, south, is_nt, opening_lead, origin)
    }

    /// Render the declarer's plan layout with explicit parameters
    ///
    /// Origin is the top-left corner of the display area.
    /// The opening lead is rendered between dummy and fan.
    /// Returns the height used.
    pub fn render(
        &self,
        layer: &mut LayerBuilder,
        north: &Hand,
        south: &Hand,
        is_nt: bool,
        opening_lead: Option<Card>,
        origin: (Mm, Mm),
    ) -> f32 {
        self.render_with_info(
            layer,
            north,
            south,
            is_nt,
            opening_lead,
            None,
            None,
            None,
            origin,
        )
    }

    /// Render the declarer's plan layout with deal info
    ///
    /// Origin is the top-left corner of the display area.
    /// deal_number: Optional deal number to display (e.g., "1", "2")
    /// contract_str: Optional contract string (e.g., "4♥")
    /// trump: Optional trump suit (used for suit ordering in displays)
    /// Returns the height used.
    #[allow(unused_variables, clippy::too_many_arguments)]
    pub fn render_with_info(
        &self,
        layer: &mut LayerBuilder,
        north: &Hand,
        south: &Hand,
        is_nt: bool,
        opening_lead: Option<Card>,
        deal_number: Option<u32>,
        contract_str: Option<&str>,
        trump: Option<BidSuit>,
        origin: (Mm, Mm),
    ) -> f32 {
        let s = self.layout_scale;
        let (ox, oy) = (origin.0 .0, origin.1 .0);

        let header_height = HEADER_HEIGHT * s;
        let element_gap = ELEMENT_GAP * s;
        let lead_box_font_size = LEAD_BOX_FONT_SIZE * s;

        // Content starts at origin
        let content_x = ox;

        // Get dummy dimensions for layout calculations
        let dummy_renderer = self.dummy_renderer(trump, north);
        let (dummy_width, _) = dummy_renderer.dimensions(north);
        let nominal_dummy_height = self.nominal_dummy_height();

        // Right edge for right-justified text
        let right_edge = content_x + dummy_width;

        // Render header line at the top
        // Header Y position (baseline of text)
        let header_y = oy - header_height + 2.0 * s; // scaled offset from bottom of header area

        layer.set_fill_color(Color::Rgb(BLACK));
        let measurer = text_metrics::get_times_measurer();

        let goal_text = if is_nt {
            "Goal: at least ____ winners"
        } else {
            "Goal: at most ____ losers"
        };

        // The header puts "Deal N  Ctr: X" on the left and the goal on the
        // right, both on one line no wider than the dummy diagram. Nothing
        // stopped the two from meeting: a wide contract and the longer goal
        // wording ran together as "6NTGoal: ...". Measure both ends first and
        // shrink the line just enough to keep a gap between them.
        let header_font_size = {
            let base = HEADER_FONT_SIZE * s;
            let mut left = 0.0;
            if let Some(deal_num) = deal_number {
                left += measurer.measure_width_mm(&format!("Deal {}", deal_num), base)
                    + DEAL_CONTRACT_GAP * s;
            }
            if let Some(contract) = contract_str {
                left += measurer.measure_width_mm(CONTRACT_LABEL, base)
                    + measurer.measure_width_mm(contract, base);
            }
            fit_header_font_size(
                base,
                left,
                measurer.measure_width_mm(goal_text, base),
                HEADER_MIN_GAP * s,
                right_edge - content_x,
            )
        };

        // Left: "Deal #" followed by "Ctr: xx"
        let mut text_x = content_x;
        if let Some(deal_num) = deal_number {
            let deal_text = format!("Deal {}", deal_num);
            let deal_width = measurer.measure_width_mm(&deal_text, header_font_size);
            layer.use_text_builtin(
                &deal_text,
                header_font_size,
                Mm(text_x),
                Mm(header_y),
                self.bold_font,
            );
            text_x += deal_width + DEAL_CONTRACT_GAP * s;
        }

        // Contract right after deal number (abbreviated)
        if let Some(contract) = contract_str {
            let label = CONTRACT_LABEL;
            let label_width = measurer.measure_width_mm(label, header_font_size);
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(label, header_font_size, Mm(text_x), Mm(header_y), self.font);

            // Render contract with colored suit symbol
            let contract_x = text_x + label_width;
            self.render_colored_contract(
                layer,
                contract,
                trump,
                Mm(contract_x),
                Mm(header_y),
                header_font_size,
            );
        }

        // Right: Goal text (right-justified)
        layer.set_fill_color(Color::Rgb(BLACK));
        let goal_width = measurer.measure_width_mm(goal_text, header_font_size);
        let goal_x = right_edge - goal_width;
        layer.use_text_builtin(
            goal_text,
            header_font_size,
            Mm(goal_x),
            Mm(header_y),
            self.font,
        );

        // Dummy (North hand) - positioned below header with gap, raised by DUMMY_RAISE
        let dummy_y = oy - header_height - element_gap + DUMMY_RAISE * s;
        dummy_renderer.render(layer, north, (Mm(content_x), Mm(dummy_y)));

        // Fan (South hand) - positioned below dummy, raised by FAN_RAISE
        let fan_renderer = self.fan_renderer(dummy_width, south, trump);
        let (_, fan_height) = fan_renderer.dimensions(south);
        let visible_fan_height = fan_height * FAN_CROP_RATIO;
        let fan_y = dummy_y - nominal_dummy_height - element_gap + FAN_RAISE * s;
        fan_renderer.render(layer, south, (Mm(content_x), Mm(fan_y)));

        // Opening lead box (e.g., "Lead: ♠4") - positioned at left margin, above fan top
        if let Some(lead_card) = opening_lead {
            self.render_opening_lead_box(layer, lead_card, content_x, fan_y + 9.0 * s);
        }

        // Table below the VISIBLE portion of the fan (centered on dummy width), raised by TABLE_RAISE
        let (table_width, _) = if is_nt {
            self.winners_table_renderer().dimensions()
        } else {
            self.losers_table_renderer().dimensions()
        };
        let table_x = content_x + (dummy_width - table_width) / 2.0;
        let table_y = fan_y - visible_fan_height - element_gap + TABLE_RAISE * s;

        if is_nt {
            let table = self.winners_table_renderer();
            table.render(layer, (Mm(table_x), Mm(table_y)));
        } else {
            let table = self.losers_table_renderer();
            table.render(layer, (Mm(table_x), Mm(table_y)));
        }

        // Calculate total height used
        let (_, total_height) = self.dimensions(north, south, is_nt);
        total_height
    }

    /// Render contract string with colored suit symbol
    /// Hearts and diamonds are rendered in red, spades and clubs in black
    fn render_colored_contract(
        &self,
        layer: &mut LayerBuilder,
        contract: &str,
        trump: Option<BidSuit>,
        x: Mm,
        y: Mm,
        font_size: f32,
    ) {
        // Use serif measurer for text width
        let measurer = text_metrics::get_times_measurer();

        // Determine if trump suit is red
        let is_red = trump.map(|t| t.is_red()).unwrap_or(false);

        // Split contract into level and symbol
        // Contract format is like "4♥" or "3NT"
        if contract.len() >= 2 {
            // First character is the level
            let level = &contract[0..1];
            let symbol = &contract[1..];

            // Render level in black using builtin font
            layer.set_fill_color(Color::Rgb(BLACK));
            let level_width = measurer.measure_width_mm(level, font_size);
            layer.use_text_builtin(level, font_size, x, y, self.bold_font);

            // Render symbol in appropriate color
            if is_red {
                layer.set_fill_color(Color::Rgb(self.colors.hearts.clone()));
            } else {
                layer.set_fill_color(Color::Rgb(BLACK));
            }

            // Use builtin font for NT, symbol font for suit symbols
            let is_nt = trump
                .map(|t| t == BidSuit::NoTrump)
                .unwrap_or(symbol == "NT");
            if is_nt {
                layer.use_text_builtin(symbol, font_size, Mm(x.0 + level_width), y, self.bold_font);
            } else {
                layer.use_text(
                    symbol,
                    font_size,
                    Mm(x.0 + level_width),
                    y,
                    self.symbol_font,
                );
            }
        } else {
            // Fallback: render whole contract using builtin font
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(contract, font_size, x, y, self.bold_font);
        }

        // Reset to black
        layer.set_fill_color(Color::Rgb(BLACK));
    }

    /// Render opening lead box with mild yellow background (e.g., "Lead: ♠4")
    fn render_opening_lead_box(&self, layer: &mut LayerBuilder, card: Card, x: f32, y: f32) {
        let measurer = text_metrics::get_times_measurer();
        let font_size = LEAD_BOX_FONT_SIZE * self.layout_scale;
        let cap_height = measurer.cap_height_mm(font_size);

        // Build the text components: "Lead: " + suit symbol + rank
        let label = "Lead: ";
        let suit_symbol = card.suit.symbol().to_string();
        let rank_str = card.rank.display_str().to_string();

        // Measure widths
        let label_width = measurer.measure_width_mm(label, font_size);
        let suit_width = measurer.measure_width_mm(&suit_symbol, font_size);
        let rank_width = measurer.measure_width_mm(&rank_str, font_size);
        let total_width = label_width + suit_width + rank_width;

        // Box dimensions with padding
        let padding_h = 1.5; // Horizontal padding
        let padding_v = 1.0; // Vertical padding
        let box_width = total_width + 2.0 * padding_h;
        let box_height = cap_height + 2.0 * padding_v;

        // Box position (y is the top of the box)
        let box_x = x;
        let box_y = y;
        let box_bottom = box_y - box_height;

        // Draw mild yellow background rectangle (filled, no stroke)
        // add_rect takes (x1, y1, x2, y2) - lower-left and upper-right corners
        layer.set_fill_color(Color::Rgb(MILD_YELLOW));
        layer.add_rect(
            Mm(box_x),
            Mm(box_bottom),
            Mm(box_x + box_width),
            Mm(box_y),
            PaintMode::Fill,
        );

        // Text baseline position
        let text_x = box_x + padding_h;
        let text_y = box_y - padding_v - cap_height;

        // Render "Lead: " in black
        layer.set_fill_color(Color::Rgb(BLACK));
        layer.use_text_builtin(label, font_size, Mm(text_x), Mm(text_y), self.font);

        // Render suit symbol in appropriate color
        let suit_color = self.colors.for_suit(&card.suit);
        layer.set_fill_color(Color::Rgb(suit_color));
        layer.use_text(
            &suit_symbol,
            font_size,
            Mm(text_x + label_width),
            Mm(text_y),
            self.symbol_font,
        );

        // Render rank in black
        layer.set_fill_color(Color::Rgb(BLACK));
        layer.use_text_builtin(
            &rank_str,
            font_size,
            Mm(text_x + label_width + suit_width),
            Mm(text_y),
            self.bold_font,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::helpers::text_metrics;

    /// The header line as it is actually assembled: "Deal N" + gap + "Ctr: " +
    /// the contract on the left, the goal right-justified.
    fn header_fits(contract: &str, is_nt: bool, available: f32, scale: f32) -> bool {
        let m = text_metrics::get_times_measurer();
        let base = HEADER_FONT_SIZE * scale;
        let goal = if is_nt {
            "Goal: at least ____ winners"
        } else {
            "Goal: at most ____ losers"
        };
        let left = m.measure_width_mm("Deal 8", base)
            + DEAL_CONTRACT_GAP * scale
            + m.measure_width_mm(CONTRACT_LABEL, base)
            + m.measure_width_mm(contract, base);
        let goal_width = m.measure_width_mm(goal, base);
        let min_gap = HEADER_MIN_GAP * scale;

        let fitted = fit_header_font_size(base, left, goal_width, min_gap, available);
        // Widths scale with the font size, so re-scale rather than re-measure.
        let k = fitted / base;
        (left + min_gap + goal_width) * k <= available + 0.01
    }

    /// (layout_scale, width available to the header) as the three declarer's
    /// plan layouts actually call this, measured from a real render.
    const REAL_GEOMETRY: [(f32, f32); 3] = [
        (1.0, 88.5),        // declarers-plan (4-up)
        (1.2857143, 112.1), // declarers-plan-2up
        (1.5714287, 135.7), // declarers-plan-1up
    ];

    #[test]
    fn leaves_a_gap_between_the_contract_and_the_goal() {
        // Regression: a notrump contract is the widest ("6NT" against "6♥"), and
        // it pairs with the longer goal wording. The two ends were positioned
        // independently, which ran them together as "6NTGoal: at least ...".
        for (scale, available) in REAL_GEOMETRY {
            for (contract, is_nt) in [
                ("6NT", true),
                ("7NT", true),
                ("6\u{2665}", false),
                ("1\u{2660}", false),
            ] {
                assert!(
                    header_fits(contract, is_nt, available, scale),
                    "{contract} at scale {scale} in {available}mm overruns the goal text",
                );
            }
        }
    }

    #[test]
    fn the_real_layouts_barely_need_shrinking() {
        // The autofit is a safety net, not a design element: if a change makes
        // it work hard, the header has outgrown its panel and the fix belongs
        // in the layout rather than in a smaller font.
        let m = text_metrics::get_times_measurer();
        for (scale, available) in REAL_GEOMETRY {
            let base = HEADER_FONT_SIZE * scale;
            let left = m.measure_width_mm("Deal 8", base)
                + DEAL_CONTRACT_GAP * scale
                + m.measure_width_mm(CONTRACT_LABEL, base)
                + m.measure_width_mm("7NT", base);
            let goal = m.measure_width_mm("Goal: at least ____ winners", base);
            let fitted = fit_header_font_size(base, left, goal, HEADER_MIN_GAP * scale, available);
            let ratio = fitted / base;
            assert!(
                ratio > 0.9,
                "scale {scale} shrinks the header to {:.0}% -- the panel is too narrow for it",
                ratio * 100.0,
            );
        }
    }

    #[test]
    fn does_not_shrink_a_header_that_already_fits() {
        let base = 14.0;
        assert_eq!(fit_header_font_size(base, 20.0, 30.0, 2.0, 100.0), base);
        // Exactly touching the limit still counts as fitting.
        assert_eq!(fit_header_font_size(base, 20.0, 30.0, 2.0, 52.0), base);
    }

    #[test]
    fn shrinks_only_as_far_as_the_floor() {
        let base = 14.0;
        // Wildly too narrow: clamped rather than reduced to something unreadable.
        let fitted = fit_header_font_size(base, 200.0, 300.0, 2.0, 10.0);
        assert!(
            (fitted - base * HEADER_MIN_SHRINK).abs() < 1e-6,
            "got {fitted}"
        );
    }

    #[test]
    fn a_zero_width_panel_does_not_produce_a_nonsense_size() {
        let fitted = fit_header_font_size(14.0, 20.0, 30.0, 2.0, 0.0);
        assert!(fitted.is_finite() && fitted > 0.0, "got {fitted}");
    }
}
