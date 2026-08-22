//! Dummy card display renderer
//!
//! Renders a hand as stacked cards by suit, like dummy's hand laid out on the table.
//! Each suit is a vertical stack with only the bottom card fully visible.

use printpdf::{Mm, PaintMode, Rgb};
use std::collections::HashMap;

use crate::model::{Card, Hand, Rank, Suit};
use crate::render::helpers::card_assets::{CardAssets, CardFace, CARD_HEIGHT_MM, CARD_WIDTH_MM};
use crate::render::helpers::colors::RED;
use crate::render::helpers::layer::LayerBuilder;

/// Gap between suit stacks in mm
const SUIT_GAP_MM: f32 = 2.0;

/// Default portion of card visible when overlapped (15% of card height)
const DEFAULT_OVERLAP_RATIO: f32 = 0.15;

/// Default suit order with alternating colors: Spades (black), Hearts (red), Clubs (black), Diamonds (red)
const ALTERNATING_SUIT_ORDER: [Suit; 4] = [Suit::Spades, Suit::Hearts, Suit::Clubs, Suit::Diamonds];

/// Standard suit order: Spades, Hearts, Diamonds, Clubs
const STANDARD_SUIT_ORDER: [Suit; 4] = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];

/// Circle/ellipse position relative to the card's top-left corner
/// Based on SVG analysis: rank text at x=1.16pt, y=28pt; suit symbol at x=11pt, y=40pt
/// Card dimensions: 167.09pt × 242.67pt
const CIRCLE_CENTER_X_RATIO: f32 = 0.06; // ~10pt / 167pt = 6% from left edge
const CIRCLE_CENTER_Y_RATIO: f32 = 0.115; // ~28pt / 243pt = 11.5% from top edge
const CIRCLE_RADIUS_RATIO: f32 = 0.17; // 17% of card width as radius (vertical)
const ELLIPSE_WIDTH_RATIO: f32 = 0.65; // Ellipse is 65% as wide as it is tall

/// Renderer for dummy-style card display (vertical stacks by suit)
pub struct DummyRenderer<'a> {
    card_assets: &'a CardAssets,
    scale: f32,
    overlap_ratio: f32,
    first_suit: Suit,
    alternate_colors: bool,
    /// Whether to draw a debug rectangle showing the bounding box
    show_bounds: bool,
    /// Cards to circle (highlight) with their colors
    circled_cards: HashMap<Card, Rgb>,
}

impl<'a> DummyRenderer<'a> {
    /// Create a new dummy renderer with the given card assets and scale factor
    ///
    /// Uses default settings: spades first, alternating colors
    pub fn new(card_assets: &'a CardAssets, scale: f32) -> Self {
        Self {
            card_assets,
            scale,
            overlap_ratio: DEFAULT_OVERLAP_RATIO,
            first_suit: Suit::Spades,
            alternate_colors: true,
            show_bounds: false,
            circled_cards: HashMap::new(),
        }
    }

    /// Create a new dummy renderer with custom overlap ratio
    ///
    /// overlap_ratio is the portion of card visible when overlapped (0.0 to 1.0)
    pub fn with_overlap(card_assets: &'a CardAssets, scale: f32, overlap_ratio: f32) -> Self {
        Self {
            card_assets,
            scale,
            overlap_ratio,
            first_suit: Suit::Spades,
            alternate_colors: true,
            show_bounds: false,
            circled_cards: HashMap::new(),
        }
    }

    /// Set the first suit to display (rotates the suit order)
    pub fn first_suit(mut self, suit: Suit) -> Self {
        self.first_suit = suit;
        self
    }

    /// Set whether to alternate suit colors (default: true)
    ///
    /// When true: uses order like S-H-C-D (black-red-black-red)
    /// When false: uses order like S-H-D-C
    pub fn alternate_colors(mut self, alternate: bool) -> Self {
        self.alternate_colors = alternate;
        self
    }

    /// Set whether to show a debug bounding box rectangle (default: false)
    pub fn show_bounds(mut self, show: bool) -> Self {
        self.show_bounds = show;
        self
    }

    /// Set which cards should be circled (highlighted) with their colors
    ///
    /// The ellipse appears around the rank/suit indicator in the top-left corner of the card.
    pub fn circled_cards(mut self, cards: HashMap<Card, Rgb>) -> Self {
        self.circled_cards = cards;
        self
    }

    /// Add a single card to circle with the default color (red)
    pub fn circle_card(mut self, card: Card) -> Self {
        self.circled_cards.insert(card, RED);
        self
    }

    /// Add a single card to circle with a specific color
    pub fn circle_card_with_color(mut self, card: Card, color: Rgb) -> Self {
        self.circled_cards.insert(card, color);
        self
    }

    /// Get the suit order based on configuration
    fn suit_order(&self) -> [Suit; 4] {
        suit_order_for(self.first_suit, self.alternate_colors)
    }

    /// Which rendition of each card this layout draws for a hand.
    ///
    /// Cards are stacked highest-first and drawn in that order, so every card
    /// but the lowest in a suit is covered down to `overlap_ratio` of its
    /// height -- only that band is ever seen. The lowest card is fully
    /// exposed and needs the complete illustration.
    ///
    /// `render` walks the same order, so the two cannot drift apart.
    pub fn faces(
        hand: &Hand,
        first_suit: Suit,
        alternate_colors: bool,
    ) -> Vec<(Suit, Rank, CardFace)> {
        let mut out = Vec::new();
        for suit in suit_order_for(first_suit, alternate_colors) {
            let ranks = hand.holding(suit).ranks.to_vec();
            let last = ranks.len().saturating_sub(1);
            for (i, rank) in ranks.iter().enumerate() {
                let face = if i == last {
                    CardFace::Full
                } else {
                    CardFace::Band
                };
                out.push((suit, *rank, face));
            }
        }
        out
    }
}

/// Rotate the configured base order so `first_suit` leads.
fn suit_order_for(first_suit: Suit, alternate_colors: bool) -> [Suit; 4] {
    let base_order = if alternate_colors {
        ALTERNATING_SUIT_ORDER
    } else {
        STANDARD_SUIT_ORDER
    };

    // Find the index of the first suit in the base order
    let start_idx = base_order
        .iter()
        .position(|&s| s == first_suit)
        .unwrap_or(0);

    // Rotate the order so first_suit is first
    [
        base_order[start_idx],
        base_order[(start_idx + 1) % 4],
        base_order[(start_idx + 2) % 4],
        base_order[(start_idx + 3) % 4],
    ]
}

impl<'a> DummyRenderer<'a> {
    /// Get the scaled card dimensions
    pub fn card_size(&self) -> (f32, f32) {
        self.card_assets.card_size_mm(self.scale)
    }

    /// Calculate the visible height for overlapped cards
    fn visible_height(&self) -> f32 {
        CARD_HEIGHT_MM * self.scale * self.overlap_ratio
    }

    /// Calculate the total dimensions needed to render a hand
    ///
    /// Returns (width, height) in mm
    pub fn dimensions(&self, hand: &Hand) -> (f32, f32) {
        let (card_width, card_height) = self.card_size();
        let visible_height = self.visible_height();

        // Width: 4 suit stacks with gaps between them
        let width = 4.0 * card_width + 3.0 * SUIT_GAP_MM;

        // Height: find the tallest stack
        let suits = self.suit_order();
        let max_cards = suits
            .iter()
            .map(|suit| hand.holding(*suit).len())
            .max()
            .unwrap_or(0);

        // Height: one full card + overlapped portions for remaining cards
        let height = if max_cards == 0 {
            0.0
        } else {
            card_height + (max_cards - 1) as f32 * visible_height
        };

        (width, height)
    }

    /// Render a hand in dummy layout
    ///
    /// Origin is the top-left corner of the display area.
    /// Cards are arranged in 4 columns based on suit order configuration.
    /// Within each column, cards are stacked vertically with highest rank at top.
    ///
    /// Returns the height used.
    pub fn render(&self, layer: &mut LayerBuilder, hand: &Hand, origin: (Mm, Mm)) -> f32 {
        let (card_width, card_height) = self.card_size();
        let visible_height = self.visible_height();

        // Draw bounding box if requested
        if self.show_bounds {
            let (width, height) = self.dimensions(hand);
            layer.set_outline_color(printpdf::Color::Rgb(printpdf::Rgb::new(
                1.0, 0.0, 0.0, None,
            )));
            layer.set_outline_thickness(1.0);
            layer.add_rect(
                origin.0,
                Mm(origin.1 .0 - height),
                Mm(origin.0 .0 + width),
                origin.1,
                printpdf::PaintMode::Stroke,
            );
        }

        let suits = self.suit_order();

        for (col, suit) in suits.iter().enumerate() {
            let col_x = origin.0 .0 + col as f32 * (card_width + SUIT_GAP_MM);
            let holding = hand.holding(*suit);

            if holding.is_empty() {
                continue;
            }

            // Get ranks sorted high to low (BTreeSet already gives this order)
            let ranks: Vec<Rank> = holding.ranks.to_vec();

            // Render cards from top (highest rank) to bottom (lowest rank)
            // so that lower cards render on top and naturally cover the cards above.
            // The bottom card (lowest rank, last rendered) will be fully visible.
            let last_index = ranks.len().saturating_sub(1);
            for (i, rank) in ranks.iter().enumerate() {
                let card_top_y = origin.1 .0 - i as f32 * visible_height;
                // Every card but the lowest is covered down to `visible_height`.
                let face = if i == last_index {
                    CardFace::Full
                } else {
                    CardFace::Band
                };

                // The bottom of this card
                let card_bottom_y = card_top_y - card_height;

                // Place the card (transform is at bottom-left)
                let transform = self
                    .card_assets
                    .transform_at(col_x, card_bottom_y, self.scale);
                layer.use_xobject(
                    self.card_assets.get_face(*suit, *rank, face).clone(),
                    transform,
                );

                // Draw ellipse if this card is in the circled set
                if let Some(color) = self.circled_cards.get(&Card::new(*suit, *rank)) {
                    self.draw_card_ellipse(layer, col_x, card_bottom_y, color);
                }
            }
        }

        // Return the height used
        let (_, height) = self.dimensions(hand);
        height
    }

    /// Draw an ellipse around the rank/suit indicator in the top-left corner of a card
    ///
    /// card_bottom_left_x, card_bottom_left_y: position of card's bottom-left corner in mm
    /// color: the color to draw the ellipse outline
    fn draw_card_ellipse(
        &self,
        layer: &mut LayerBuilder,
        card_bottom_left_x: f32,
        card_bottom_left_y: f32,
        color: &Rgb,
    ) {
        let card_width = CARD_WIDTH_MM * self.scale;
        let card_height = CARD_HEIGHT_MM * self.scale;

        // Ellipse center position relative to card's bottom-left corner
        // The ellipse should be in the top-left area of the card
        let ellipse_x = card_bottom_left_x + card_width * CIRCLE_CENTER_X_RATIO;
        let ellipse_y = card_bottom_left_y + card_height * (1.0 - CIRCLE_CENTER_Y_RATIO); // from top
        let ellipse_radius_y = card_width * CIRCLE_RADIUS_RATIO; // vertical radius
        let ellipse_radius_x = ellipse_radius_y * ELLIPSE_WIDTH_RATIO; // horizontal radius (30% narrower)

        // Draw the ellipse with the specified color
        layer.set_outline_color(printpdf::Color::Rgb(color.clone()));
        layer.set_outline_thickness(1.5);
        layer.add_ellipse(
            Mm(ellipse_x),
            Mm(ellipse_y),
            Mm(ellipse_radius_x),
            Mm(ellipse_radius_y),
            PaintMode::Stroke,
        );
    }
}
