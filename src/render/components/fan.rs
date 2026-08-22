//! Fan card display renderer
//!
//! Renders a hand as a horizontal fan, like cards held in hand.
//! Cards overlap horizontally with only the rightmost card fully visible.
//! Optional arc parameter rotates cards to simulate the natural curve of a held hand.

use printpdf::{CurTransMat, Mm, PaintMode, Rgb};
use std::collections::HashMap;

use crate::model::{Card, Hand, Rank, Suit};
use crate::render::helpers::card_assets::{CardAssets, CardFace, CARD_HEIGHT_MM, CARD_WIDTH_MM};
use crate::render::helpers::colors::RED;
use crate::render::helpers::layer::LayerBuilder;

/// Conversion factor from mm to points
const MM_TO_PT: f32 = 2.834_645_7;

/// Portion of card visible when overlapped (8% of card width)
const DEFAULT_OVERLAP_RATIO: f32 = 0.08;

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

/// Renderer for fan-style card display (horizontal fan)
pub struct FanRenderer<'a> {
    card_assets: &'a CardAssets,
    scale: f32,
    overlap_ratio: f32,
    first_suit: Suit,
    alternate_colors: bool,
    /// Total arc angle in degrees (e.g., 30.0 means cards span from -15° to +15°)
    arc_degrees: f32,
    /// Rotation of the entire fan display in degrees (counter-clockwise)
    /// 90.0 = vertical with cards going up (for East position)
    /// -90.0 = vertical with cards going down (for West position)
    display_rotation: f32,
    /// Whether to draw a debug rectangle showing the bounding box
    show_bounds: bool,
    /// Cards to circle (highlight) with their colors
    circled_cards: HashMap<Card, Rgb>,
}

impl<'a> FanRenderer<'a> {
    /// Create a new fan renderer with the given card assets and scale factor
    ///
    /// Uses default settings: spades first, alternating colors, no arc (flat)
    pub fn new(card_assets: &'a CardAssets, scale: f32) -> Self {
        Self {
            card_assets,
            scale,
            overlap_ratio: DEFAULT_OVERLAP_RATIO,
            first_suit: Suit::Spades,
            alternate_colors: true,
            arc_degrees: 0.0,
            display_rotation: 0.0,
            show_bounds: false,
            circled_cards: HashMap::new(),
        }
    }

    /// Create a new fan renderer with custom overlap ratio
    ///
    /// overlap_ratio is the portion of card visible when overlapped (0.0 to 1.0)
    pub fn with_overlap(card_assets: &'a CardAssets, scale: f32, overlap_ratio: f32) -> Self {
        Self {
            card_assets,
            scale,
            overlap_ratio,
            first_suit: Suit::Spades,
            alternate_colors: true,
            arc_degrees: 0.0,
            display_rotation: 0.0,
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

    /// Set the arc angle in degrees (default: 0.0 = flat)
    ///
    /// The total arc that the fan spans. Cards are rotated so the leftmost card
    /// is rotated counter-clockwise and the rightmost is rotated clockwise,
    /// simulating cards held in hand where lower edges are closer together.
    ///
    /// Typical values: 30-50 degrees for a natural hand appearance.
    pub fn arc(mut self, degrees: f32) -> Self {
        self.arc_degrees = degrees;
        self
    }

    /// Set the rotation of the entire fan display in degrees (counter-clockwise)
    ///
    /// This rotates the entire rendered fan around the origin point.
    /// - 90.0 = cards fan upward (for East position)
    /// - -90.0 = cards fan downward (for West position)
    pub fn rotation(mut self, degrees: f32) -> Self {
        self.display_rotation = degrees;
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
        fan_suit_order(self.first_suit, self.alternate_colors)
    }

    /// The cards this layout draws, left to right, with the rendition each needs.
    ///
    /// Cards overlap horizontally, so every card but the rightmost shows only
    /// a narrow wedge down its left edge -- the index and nothing else. Court
    /// cards would otherwise leak a vertical rail of their portrait frame into
    /// that wedge, which reads as stray linework, so covered cards use the
    /// corner rendition. Only the rightmost card is fully exposed.
    ///
    /// `render_internal` builds the same sequence, so the two stay in step.
    pub fn faces(
        hand: &Hand,
        first_suit: Suit,
        alternate_colors: bool,
    ) -> Vec<(Suit, Rank, CardFace)> {
        let mut cards: Vec<(Suit, Rank, CardFace)> = Vec::new();
        for suit in fan_suit_order(first_suit, alternate_colors) {
            for rank in hand.holding(suit).ranks.iter() {
                cards.push((suit, *rank, CardFace::Corner));
            }
        }
        if let Some(last) = cards.last_mut() {
            last.2 = CardFace::Full;
        }
        cards
    }

    /// Get the scaled card dimensions
    pub fn card_size(&self) -> (f32, f32) {
        self.card_assets.card_size_mm(self.scale)
    }

    /// Calculate the visible width for overlapped cards
    fn visible_width(&self) -> f32 {
        CARD_WIDTH_MM * self.scale * self.overlap_ratio
    }

    /// Calculate the total dimensions needed to render a hand
    ///
    /// Returns (width, height) in mm, accounting for display rotation
    pub fn dimensions(&self, hand: &Hand) -> (f32, f32) {
        let (width, height) = self.base_dimensions(hand);

        // Account for display rotation
        if self.display_rotation.abs() > 0.001 {
            let angle_rad = self.display_rotation.to_radians();
            let cos_a = angle_rad.cos().abs();
            let sin_a = angle_rad.sin().abs();

            // Rotated bounding box
            let rotated_width = width * cos_a + height * sin_a;
            let rotated_height = width * sin_a + height * cos_a;
            (rotated_width, rotated_height)
        } else {
            (width, height)
        }
    }

    /// Calculate the base dimensions (before display rotation)
    ///
    /// This computes the actual bounding box by simulating card placement
    /// to account for arc offsets and rotation effects.
    pub fn base_dimensions(&self, hand: &Hand) -> (f32, f32) {
        let (dims, _offset) = self.base_dimensions_and_offset(hand);
        dims
    }

    /// Calculate base dimensions and the offset needed to align cards with the bounding box
    ///
    /// Returns ((width, height), (offset_x, offset_y)) where offset is subtracted from
    /// card positions to align them with a bounding box starting at (0, 0).
    fn base_dimensions_and_offset(&self, hand: &Hand) -> ((f32, f32), (f32, f32)) {
        let (card_width, card_height) = self.card_size();
        let visible_width = self.visible_width();

        let card_count = hand.card_count();

        if card_count == 0 {
            return ((0.0, 0.0), (0.0, 0.0));
        }

        if self.arc_degrees.abs() < 0.001 {
            // No arc - simple flat layout
            // Cards are placed with top at y=0, extending down to y=-card_height
            let base_width = (card_count - 1) as f32 * visible_width + card_width;
            // Offset: cards start at x=0, top at y=0
            ((base_width, card_height), (0.0, -card_height))
        } else {
            // With arc, we need to simulate card placement to find actual bounds
            let half_arc = self.arc_degrees / 2.0;
            let max_angle_rad = half_arc.to_radians();
            let sin_a = max_angle_rad.abs().sin();
            let cos_a = max_angle_rad.abs().cos();

            // x_offset used in render_internal
            let rotated_card_width = card_width * cos_a + card_height * sin_a;
            let x_offset = (rotated_card_width - card_width) / 2.0;

            // Track min/max extents
            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;
            let mut min_y = f32::MAX;
            let mut max_y = f32::MIN;

            // Simulate each card position (using origin at (0, 0) with top-left convention)
            let origin_y = 0.0; // card_top_y in render_internal

            for i in 0..card_count {
                // Card rotation angle
                let rotation = if card_count > 1 {
                    let t = i as f32 / (card_count - 1) as f32;
                    half_arc - t * self.arc_degrees
                } else {
                    0.0
                };

                // Card X position (bottom-left corner before rotation)
                let base_x = x_offset + i as f32 * visible_width;

                // Y offset from arc (parabolic curve)
                let t = if card_count > 1 {
                    i as f32 / (card_count - 1) as f32
                } else {
                    0.5
                };
                let arc_factor = 4.0 * t * (1.0 - t);
                let max_rise = card_height * (self.arc_degrees / 90.0) * 0.3;
                let y_offset = arc_factor * max_rise;

                // Rotation compensation
                let angle_rad = rotation.to_radians();
                let rotation_compensation = (CARD_WIDTH_MM * self.scale / 2.0) * angle_rad.sin();

                // Card bottom Y position
                let card_bottom_y = origin_y - card_height + y_offset - rotation_compensation;

                // Calculate rotated card corners
                // Rotation is around bottom-left corner (base_x, card_bottom_y)
                let corners = self.rotated_card_corners(
                    base_x,
                    card_bottom_y,
                    card_width,
                    card_height,
                    rotation,
                );

                // Update bounds
                for (cx, cy) in corners {
                    min_x = min_x.min(cx);
                    max_x = max_x.max(cx);
                    min_y = min_y.min(cy);
                    max_y = max_y.max(cy);
                }
            }

            let width = max_x - min_x;
            let height = max_y - min_y;

            ((width, height), (min_x, min_y))
        }
    }

    /// Calculate the four corners of a rotated card
    ///
    /// Rotation is around the bottom-left corner (x, y).
    fn rotated_card_corners(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rotation_degrees: f32,
    ) -> [(f32, f32); 4] {
        let angle_rad = rotation_degrees.to_radians();
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        // Four corners relative to bottom-left (0,0)
        let corners_local = [
            (0.0, 0.0),      // bottom-left
            (width, 0.0),    // bottom-right
            (width, height), // top-right
            (0.0, height),   // top-left
        ];

        // Rotate each corner around origin, then translate to (x, y)
        corners_local.map(|(lx, ly)| {
            let rx = lx * cos_a - ly * sin_a;
            let ry = lx * sin_a + ly * cos_a;
            (x + rx, y + ry)
        })
    }

    /// Render a hand in fan layout
    ///
    /// Origin is the top-left corner of the display area (for unrotated fans).
    /// For rotated fans, origin is the rotation pivot point.
    /// Cards are arranged left to right by suit (based on suit order configuration),
    /// then by rank within each suit (high to low).
    ///
    /// Returns the width used (before rotation).
    pub fn render(&self, layer: &mut LayerBuilder, hand: &Hand, origin: (Mm, Mm)) -> f32 {
        let has_display_rotation = self.display_rotation.abs() > 0.001;

        if has_display_rotation {
            // Get actual bounding box dimensions and offset
            let ((base_width, base_height), (offset_x, offset_y)) =
                self.base_dimensions_and_offset(hand);

            // The origin passed in is where the CENTER of the rotated fan should be
            let dest_center_x = origin.0 .0 * MM_TO_PT;
            let dest_center_y = origin.1 .0 * MM_TO_PT;

            // After applying the offset, the fan's bounding box is:
            // - Left edge at x=0, right edge at x=base_width
            // - Bottom edge at y=0, top edge at y=base_height
            // So the center is at (base_width/2, base_height/2)
            let fan_center_x = base_width / 2.0 * MM_TO_PT;
            let fan_center_y = base_height / 2.0 * MM_TO_PT;

            // Compute the combined transformation matrix directly.
            // We want to: translate fan center to origin, rotate, translate to destination.
            //
            // printpdf's matrix format [a b c d e f] transforms points as:
            //   x' = a*x + c*y + e
            //   y' = b*x + d*y + f
            //
            // printpdf's Rotate uses: [cos, -sin, sin, cos, 0, 0] with angle = 360 - θ
            // This is equivalent to clockwise rotation by θ, or CCW by -θ.
            //
            // To match printpdf's convention, we use the same formula:
            //   rad = (360 - display_rotation).to_radians()
            //   a = cos(rad), b = -sin(rad), c = sin(rad), d = cos(rad)
            //
            // For rotation around center C and translation to D:
            //   e = dx - (a*cx + c*cy) = dx - cos*cx - sin*cy
            //   f = dy - (b*cx + d*cy) = dy + sin*cx - cos*cy

            let rad = (360.0 - self.display_rotation).to_radians();
            let cos_a = rad.cos();
            let sin_a = rad.sin();

            // Matrix coefficients (matching printpdf's convention)
            let a = cos_a;
            let b = -sin_a;
            let c = sin_a;
            let d = cos_a;

            // Translation: D - R*C where R*C uses matrix multiplication
            let e = dest_center_x - (a * fan_center_x + c * fan_center_y);
            let f = dest_center_y - (b * fan_center_x + d * fan_center_y);

            layer.save_graphics_state();
            layer.set_transform(CurTransMat::Raw([a, b, c, d, e, f]));

            // Draw bounding box if requested
            if self.show_bounds {
                layer.set_outline_color(printpdf::Color::Rgb(printpdf::Rgb::new(
                    1.0, 0.0, 0.0, None,
                )));
                layer.set_outline_thickness(1.0);
                layer.add_rect(
                    Mm(0.0),
                    Mm(0.0),
                    Mm(base_width),
                    Mm(base_height),
                    printpdf::PaintMode::Stroke,
                );
            }

            // Render the fan with offset adjustment so cards align with bounding box
            // The offset tells us where the min corner of the actual bounds is
            // We pass origin that shifts cards so min corner is at (0, 0)
            self.render_internal(layer, hand, (Mm(-offset_x), Mm(-offset_y)));

            layer.restore_graphics_state();
        } else {
            // No display rotation - but still need to account for arc offsets
            let ((base_width, base_height), (offset_x, offset_y)) =
                self.base_dimensions_and_offset(hand);

            // Draw bounding box if requested
            // The bounding box should be positioned at origin (top-left convention)
            // with the computed dimensions
            if self.show_bounds {
                layer.set_outline_color(printpdf::Color::Rgb(printpdf::Rgb::new(
                    1.0, 0.0, 0.0, None,
                )));
                layer.set_outline_thickness(1.0);
                layer.add_rect(
                    origin.0,
                    Mm(origin.1 .0 - base_height),
                    Mm(origin.0 .0 + base_width),
                    origin.1,
                    printpdf::PaintMode::Stroke,
                );
            }

            // Render with offset adjustment so cards align with the bounding box
            // offset = (min_x, min_y) from the bounds calculation where origin_y=0
            // min_y is typically negative (cards extend below y=0)
            // max_y is near 0 or positive (top of tallest card)
            //
            // We want the top of the bounding box at origin.1
            // The bounding box top is at max_y = min_y + base_height
            // So we need: rendered_max_y = origin.1
            //             rendered_max_y = card_top_y + (max_y - 0) where card_top_y is what we pass
            // But render_internal uses card_top_y as the reference for the top of unrotated cards
            //
            // Actually simpler: offset_y = min_y, and max_y = min_y + base_height
            // We want max_y to map to origin.1
            // So we pass card_top_y such that: max_y_rendered = origin.1
            // The cards render relative to card_top_y, with the arc causing variations
            // In the simulation, we used origin_y = 0, and got min_y and max_y
            // To shift so max_y lands at origin.1: new_origin_y = origin.1 - max_y
            //                                                   = origin.1 - (min_y + base_height)
            //                                                   = origin.1 - offset_y - base_height
            let max_y = offset_y + base_height;
            let render_origin_x = origin.0 .0 - offset_x;
            let render_origin_y = origin.1 .0 - max_y;

            self.render_internal(layer, hand, (Mm(render_origin_x), Mm(render_origin_y)));
        }

        // Return the width used
        let (width, _) = self.dimensions(hand);
        width
    }

    /// Internal render method that draws cards at the specified origin
    fn render_internal(&self, layer: &mut LayerBuilder, hand: &Hand, origin: (Mm, Mm)) {
        let visible_width = self.visible_width();

        // Collect all cards in order: by suit (based on configuration), then by rank (high to low)
        let mut cards: Vec<(Suit, Rank)> = Vec::new();
        for suit in self.suit_order() {
            let holding = hand.holding(suit);
            for rank in holding.ranks.iter() {
                cards.push((suit, *rank));
            }
        }

        let num_cards = cards.len();
        if num_cards == 0 {
            return;
        }

        // Calculate arc parameters
        let has_arc = self.arc_degrees.abs() > 0.001;
        let half_arc = self.arc_degrees / 2.0;

        // When there's an arc, cards extend beyond the base layout due to rotation.
        // We need to offset the starting X position to account for this.
        let (card_width, card_height) = self.card_size();
        let x_offset = if has_arc {
            let max_angle_rad = (self.arc_degrees / 2.0).to_radians();
            let sin_a = max_angle_rad.abs().sin();
            let cos_a = max_angle_rad.abs().cos();
            let rotated_width = card_width * cos_a + card_height * sin_a;
            (rotated_width - card_width) / 2.0
        } else {
            0.0
        };

        let card_top_y = origin.1 .0;

        // Render cards from left to right
        // Each card overlays the previous one, so the rightmost card (rendered last) is fully visible
        let last_index = num_cards - 1;
        for (i, (suit, rank)) in cards.iter().enumerate() {
            // Only the rightmost card is fully exposed; the rest show just
            // their left-edge wedge, which the corner rendition covers.
            let face = if i == last_index {
                CardFace::Full
            } else {
                CardFace::Corner
            };

            // Calculate rotation for this card
            // Leftmost card: +half_arc (counter-clockwise, tilts left)
            // Rightmost card: -half_arc (clockwise, tilts right)
            // Middle card: 0
            let rotation = if has_arc && num_cards > 1 {
                let t = i as f32 / (num_cards - 1) as f32; // 0.0 to 1.0
                half_arc - t * self.arc_degrees // goes from +half_arc to -half_arc
            } else {
                0.0
            };

            let base_x = origin.0 .0 + x_offset + i as f32 * visible_width;

            if has_arc {
                // With rotation, we rotate around the bottom-left corner of the card
                // Cards form a concave arc - center cards are higher, outer cards are lower

                // Calculate vertical offset based on position in fan
                // t goes from 0 (left) to 1 (right), with 0.5 being center
                let t = if num_cards > 1 {
                    i as f32 / (num_cards - 1) as f32
                } else {
                    0.5
                };
                // Parabolic curve: 0 at edges (t=0, t=1), maximum at center (t=0.5)
                // 4 * t * (1 - t) gives 0 at edges and 1 at center
                let arc_factor = 4.0 * t * (1.0 - t);

                // Maximum rise at center based on arc angle and card height
                let max_rise = card_height * (self.arc_degrees / 90.0) * 0.3;
                let y_offset = arc_factor * max_rise;

                // Rotation compensation: when rotating around bottom-left, the card's
                // visual center shifts. For positive rotation (CCW), the bottom-left stays
                // put but the card appears to rise. For negative rotation (CW), it appears
                // to drop. We compensate so the visual bottom-center stays at a consistent height.
                let angle_rad = rotation.to_radians();
                // The bottom-center point shifts vertically by (card_width/2) * sin(angle)
                let rotation_compensation = (CARD_WIDTH_MM * self.scale / 2.0) * angle_rad.sin();

                // Base bottom y position (flat layout)
                let base_bottom_y = card_top_y - card_height;

                // Apply arc offset and rotation compensation
                // - y_offset raises center cards
                // - rotation_compensation keeps the visual bottom-center level
                let card_bottom_y = base_bottom_y + y_offset - rotation_compensation;

                let transform = self.card_assets.transform_at_rotated(
                    base_x,
                    card_bottom_y,
                    self.scale,
                    rotation,
                );
                layer.use_xobject(
                    self.card_assets.get_face(*suit, *rank, face).clone(),
                    transform,
                );

                // Draw ellipse if this card is in the circled set
                if let Some(color) = self.circled_cards.get(&Card::new(*suit, *rank)) {
                    self.draw_card_ellipse(layer, base_x, card_bottom_y, rotation, color);
                }
            } else {
                // No rotation - simple flat layout
                let card_bottom_y = card_top_y - card_height;
                let transform = self
                    .card_assets
                    .transform_at(base_x, card_bottom_y, self.scale);
                layer.use_xobject(
                    self.card_assets.get_face(*suit, *rank, face).clone(),
                    transform,
                );

                // Draw ellipse if this card is in the circled set
                if let Some(color) = self.circled_cards.get(&Card::new(*suit, *rank)) {
                    self.draw_card_ellipse(layer, base_x, card_bottom_y, 0.0, color);
                }
            }
        }
    }

    /// Draw a rotated ellipse around the rank/suit indicator in the top-left corner of a card
    ///
    /// card_bottom_left_x, card_bottom_left_y: position of card's bottom-left corner in mm
    /// rotation: card rotation in degrees (counter-clockwise)
    /// color: the color to draw the ellipse outline
    fn draw_card_ellipse(
        &self,
        layer: &mut LayerBuilder,
        card_bottom_left_x: f32,
        card_bottom_left_y: f32,
        rotation: f32,
        color: &Rgb,
    ) {
        let card_width = CARD_WIDTH_MM * self.scale;
        let card_height = CARD_HEIGHT_MM * self.scale;

        // Ellipse center position relative to card's bottom-left corner (unrotated)
        // The ellipse should be in the top-left area of the card
        let ellipse_local_x = card_width * CIRCLE_CENTER_X_RATIO;
        let ellipse_local_y = card_height * (1.0 - CIRCLE_CENTER_Y_RATIO); // from top
        let ellipse_radius_y = card_width * CIRCLE_RADIUS_RATIO; // vertical radius
        let ellipse_radius_x = ellipse_radius_y * ELLIPSE_WIDTH_RATIO; // horizontal radius (30% narrower)

        // Calculate rotated center position
        let (ellipse_x, ellipse_y) = if rotation.abs() > 0.001 {
            let angle_rad = rotation.to_radians();
            let cos_a = angle_rad.cos();
            let sin_a = angle_rad.sin();
            // Rotate point around origin (bottom-left of card)
            let rotated_x = ellipse_local_x * cos_a - ellipse_local_y * sin_a;
            let rotated_y = ellipse_local_x * sin_a + ellipse_local_y * cos_a;
            (
                card_bottom_left_x + rotated_x,
                card_bottom_left_y + rotated_y,
            )
        } else {
            (
                card_bottom_left_x + ellipse_local_x,
                card_bottom_left_y + ellipse_local_y,
            )
        };

        // Draw the rotated ellipse with the specified color
        layer.set_outline_color(printpdf::Color::Rgb(color.clone()));
        layer.set_outline_thickness(1.5);
        layer.add_rotated_ellipse(
            Mm(ellipse_x),
            Mm(ellipse_y),
            Mm(ellipse_radius_x),
            Mm(ellipse_radius_y),
            rotation,
            PaintMode::Stroke,
        );
    }
}

/// Rotate the configured base order so `first_suit` leads.
fn fan_suit_order(first_suit: Suit, alternate_colors: bool) -> [Suit; 4] {
    let base_order = if alternate_colors {
        ALTERNATING_SUIT_ORDER
    } else {
        STANDARD_SUIT_ORDER
    };

    let start_idx = base_order
        .iter()
        .position(|&s| s == first_suit)
        .unwrap_or(0);

    [
        base_order[start_idx],
        base_order[(start_idx + 1) % 4],
        base_order[(start_idx + 2) % 4],
        base_order[(start_idx + 3) % 4],
    ]
}
