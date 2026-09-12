use crate::config::Settings;
use crate::model::card::RankExt;
use crate::model::{Deal, Direction, Hand, HiddenHands, Suit, SUITS_DISPLAY_ORDER};
use printpdf::{BuiltinFont, Color, FontId, Mm, PaintMode, Rgb};

use crate::render::helpers::colors::{self, SuitColors};
use crate::render::helpers::layer::LayerBuilder;
use crate::render::helpers::text_metrics;

/// Light gray color for debug boxes
const DEBUG_BOX_COLOR: Rgb = Rgb {
    r: 0.7,
    g: 0.7,
    b: 0.7,
    icc_profile: None,
};

/// Display options for diagram rendering, computed by the layout layer
/// This centralizes all visibility decisions in one place
#[derive(Debug, Clone, Default)]
pub struct DiagramDisplayOptions {
    /// Which hands are hidden (from [Hidden] PBN tag)
    pub hidden: HiddenHands,
    /// Hide the compass box (implied when only one hand is visible)
    pub hide_compass: bool,
    /// Which single hand is visible (when hide_compass is true)
    pub single_visible_hand: Option<Direction>,
    /// Deal is a fragment (not all 4 suits present)
    pub is_fragment: bool,
    /// Which suits are present in the deal (for fragments)
    pub suits_present: Vec<Suit>,
    /// Whether to show suit symbols (false for single-suit fragments)
    pub show_suit_symbols: bool,
}

impl DiagramDisplayOptions {
    /// Compute display options from a deal and hidden hands
    /// This applies the implied hiding rules documented in docs/BCFlags.md
    pub fn from_deal(deal: &Deal, hidden: &HiddenHands) -> Self {
        let suits_present = deal.suits_present();
        let is_fragment = suits_present.len() < 4;

        // Determine which hands are visible
        let visible: Vec<Direction> = [
            (!hidden.north, Direction::North),
            (!hidden.east, Direction::East),
            (!hidden.south, Direction::South),
            (!hidden.west, Direction::West),
        ]
        .iter()
        .filter(|(v, _)| *v)
        .map(|(_, d)| *d)
        .collect();

        // Hide compass when only one hand is visible
        let hide_compass = visible.len() == 1;
        let single_visible_hand = if hide_compass {
            visible.first().copied()
        } else {
            None
        };
        let show_suit_symbols = suits_present.len() > 1;

        Self {
            hidden: *hidden,
            hide_compass,
            single_visible_hand,
            is_fragment,
            suits_present,
            show_suit_symbols,
        }
    }
}

/// Renderer for hand diagrams
pub struct HandDiagramRenderer<'a> {
    font: BuiltinFont,
    bold_font: BuiltinFont,
    compass_font: BuiltinFont,
    symbol_font: &'a FontId, // Font with Unicode suit symbols (DejaVu Sans)
    colors: SuitColors,
    settings: &'a Settings,
    debug_boxes: bool,
}

impl<'a> HandDiagramRenderer<'a> {
    pub fn new(
        font: BuiltinFont,
        bold_font: BuiltinFont,
        compass_font: BuiltinFont,
        symbol_font: &'a FontId,
        settings: &'a Settings,
    ) -> Self {
        Self {
            font,
            bold_font,
            compass_font,
            symbol_font,
            colors: SuitColors::new(settings.black_color, settings.red_color),
            settings,
            debug_boxes: false, // Disable debug boxes for production
        }
    }

    /// Draw a debug outline box
    fn draw_debug_box(&self, layer: &mut LayerBuilder, x: f32, y: f32, w: f32, h: f32) {
        if !self.debug_boxes {
            return;
        }
        // y is top of box, draw from bottom-left to top-right
        layer.set_outline_color(Color::Rgb(DEBUG_BOX_COLOR));
        layer.set_outline_thickness(0.25);
        layer.add_rect(Mm(x), Mm(y - h), Mm(x + w), Mm(y), PaintMode::Stroke);
    }

    /// Calculate the actual height of a hand block based on font metrics
    fn actual_hand_height(&self) -> f32 {
        self.hand_height_for_suits(4)
    }

    /// Calculate hand height for a specific number of suits
    fn hand_height_for_suits(&self, num_suits: usize) -> f32 {
        let measurer = text_metrics::get_times_measurer();
        let line_height = self.settings.line_height;
        let cap_height = measurer.cap_height_mm(self.settings.card_font_size);
        let descender = measurer.descender_mm(self.settings.card_font_size);

        // N lines of text:
        // - cap_height: from top of box to first baseline
        // - (N-1) * line_height: gaps between the N baselines
        // - descender: from last baseline to bottom of descenders
        let n = num_suits.max(1) as f32;
        cap_height + (n - 1.0) * line_height + descender
    }

    /// Calculate the actual width of a hand by measuring all suit lines
    fn actual_hand_width(&self, hand: &Hand) -> f32 {
        let measurer = text_metrics::get_times_measurer();
        let font_size = self.settings.card_font_size;

        SUITS_DISPLAY_ORDER
            .iter()
            .map(|suit| {
                let holding = hand.holding(*suit);
                let cards_str = if holding.is_void() {
                    "-".to_string()
                } else {
                    holding
                        .ranks
                        .iter()
                        .map(|r| r.display_str().to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                // Full line: "♠ A K Q J T 9 8 7 6 5" (symbol + space + spaced cards)
                let line = format!("{} {}", suit.symbol(), cards_str);
                measurer.measure_width_mm(&line, font_size)
            })
            .fold(0.0_f32, |max, w| max.max(w))
    }

    /// Measure the height of a deal diagram without rendering
    pub fn measure_deal_height(&self, _deal: &Deal, options: &DiagramDisplayOptions) -> f32 {
        // North-only is just the hand height (always 4 suits, even with voids)
        // Check this BEFORE is_fragment so single hands with voids measure correctly
        if options.hide_compass {
            return self.actual_hand_height();
        }

        // Use fragment-aware height if only some suits are present
        if options.is_fragment {
            return self.measure_fragment_height(options);
        }

        // Full deal: 3 rows of hands with compass in middle row
        let hand_h = self.actual_hand_height();
        // north_y = oy.0
        // row2_y = north_y - hand_h
        // south_y = row2_y - hand_h - 2.0
        // height = oy.0 - (south_y - hand_h)
        // = oy.0 - (north_y - hand_h - hand_h - 2.0 - hand_h)
        // = oy.0 - oy.0 + 3*hand_h + 2.0
        // = 3*hand_h + 2.0
        3.0 * hand_h + 2.0
    }

    /// Measure the height of a fragment deal diagram
    fn measure_fragment_height(&self, options: &DiagramDisplayOptions) -> f32 {
        let num_suits = options.suits_present.len();
        let hand_h = self.hand_height_for_suits(num_suits);

        // North-only fragment is just the hand height
        if options.hide_compass {
            return hand_h;
        }

        // Full fragment: similar to full deal but with potentially shorter hands
        let compass_size = self.compass_box_size();
        let compass_center_offset = (compass_size - hand_h) / 2.0;

        // Small gap between compass and South hand
        let compass_hand_gap = 1.5;

        // north_y = oy.0
        // row2_y = north_y - hand_h
        // west_y = row2_y - compass_center_offset
        // south_y = west_y - hand_h - compass_center_offset - compass_hand_gap
        // height = oy.0 - (south_y - hand_h)
        // = 3*hand_h + 2*compass_center_offset + compass_hand_gap
        3.0 * hand_h + 2.0 * compass_center_offset + compass_hand_gap
    }

    /// Render a complete deal with compass rose - Bridge Composer style
    /// Returns the height used by the diagram
    pub fn render_deal(&self, layer: &mut LayerBuilder, deal: &Deal, origin: (Mm, Mm)) -> f32 {
        let options = DiagramDisplayOptions::from_deal(deal, &HiddenHands::default());
        self.render_deal_with_options(layer, deal, origin, &options)
    }

    /// Render a complete deal with compass rose, respecting hidden hands
    /// Returns the height used by the diagram
    ///
    /// DEPRECATED: Use render_deal_with_options instead for new code
    pub fn render_deal_with_hidden(
        &self,
        layer: &mut LayerBuilder,
        deal: &Deal,
        origin: (Mm, Mm),
        hidden: &HiddenHands,
    ) -> f32 {
        let options = DiagramDisplayOptions::from_deal(deal, hidden);
        self.render_deal_with_options(layer, deal, origin, &options)
    }

    /// Render a complete deal with pre-computed display options
    /// All visibility decisions should be made in the layout layer and passed here
    /// Returns the height used by the diagram
    pub fn render_deal_with_options(
        &self,
        layer: &mut LayerBuilder,
        deal: &Deal,
        origin: (Mm, Mm),
        options: &DiagramDisplayOptions,
    ) -> f32 {
        let (ox, oy) = origin;

        // Render without compass if compass is hidden (single hand visible)
        // Check this BEFORE is_fragment so single hands with voids show all 4 suits
        if options.hide_compass {
            return self.render_single_hand(layer, deal, origin, options);
        }

        // Use fragment-aware rendering if only some suits are present
        if options.is_fragment {
            return self.render_deal_fragment_with_options(layer, deal, origin, options);
        }

        // Layout constants for full deals
        let hand_w = self.settings.hand_width; // Used for positioning
        let hand_h = self.actual_hand_height(); // Use actual calculated height
        let compass_size = self.compass_box_size(); // Dynamic size based on font

        // Calculate actual widths for each hand
        let north_w = self.actual_hand_width(&deal.north);
        let south_w = self.actual_hand_width(&deal.south);
        let east_w = self.actual_hand_width(&deal.east);
        let west_w = self.actual_hand_width(&deal.west);

        // Row 1: North hand (centered above compass)
        let north_x = ox.0 + hand_w + (compass_size - hand_w) / 2.0;
        let north_y = oy.0;
        if !options.hidden.north {
            self.draw_debug_box(layer, north_x, north_y, north_w, hand_h);
            self.render_hand_cards(layer, &deal.north, (Mm(north_x), Mm(north_y)));
        }

        // Row 2: West hand | Compass | East hand (immediately below North)
        let row2_y = north_y - hand_h; // No extra gap

        // West hand - left side
        let west_x = ox.0;
        if !options.hidden.west {
            self.draw_debug_box(layer, west_x, row2_y, west_w, hand_h);
            self.render_hand_cards(layer, &deal.west, (Mm(west_x), Mm(row2_y)));
        }

        // Compass rose - vertically centered with West/East hands
        // Left edge of compass aligns with right edge of suit symbols (suit symbols are ~5mm wide)
        let suit_symbol_width = 5.0;
        let half_char_adjust = 1.5; // Fine-tune alignment
        let compass_center_x = north_x + suit_symbol_width + compass_size / 2.0 - half_char_adjust;
        let compass_y = row2_y - hand_h / 2.0; // Center vertically with West/East
                                               // Debug box for compass (centered)
        self.draw_debug_box(
            layer,
            compass_center_x - compass_size / 2.0,
            compass_y + compass_size / 2.0,
            compass_size,
            compass_size,
        );
        self.render_compass(layer, (Mm(compass_center_x), Mm(compass_y)));

        // East hand - to the right of compass
        let east_x = compass_center_x + compass_size / 2.0 + 3.5;
        if !options.hidden.east {
            self.draw_debug_box(layer, east_x, row2_y, east_w, hand_h);
            self.render_hand_cards(layer, &deal.east, (Mm(east_x), Mm(row2_y)));
        }

        // Row 3: HCP box (below West) and South hand (next to HCP box)
        let hcp_box_size = compass_size;
        let hcp_box_x = west_x;
        let hcp_box_y = row2_y - hand_h - 2.0; // Small gap below West hand

        if self.settings.show_hcp {
            self.render_hcp_box(layer, deal, (Mm(hcp_box_x), Mm(hcp_box_y)), hcp_box_size);
        }

        // South hand - positioned next to HCP box, at same Y level
        let south_y = hcp_box_y;
        if !options.hidden.south {
            self.draw_debug_box(layer, north_x, south_y, south_w, hand_h);
            self.render_hand_cards(layer, &deal.south, (Mm(north_x), Mm(south_y)));
        }

        // Return total height used
        oy.0 - (south_y - hand_h)
    }

    /// Render a deal fragment with pre-computed display options
    fn render_deal_fragment_with_options(
        &self,
        layer: &mut LayerBuilder,
        deal: &Deal,
        origin: (Mm, Mm),
        options: &DiagramDisplayOptions,
    ) -> f32 {
        // Render without compass if compass is hidden (single hand visible)
        if options.hide_compass {
            return self.render_single_hand_fragment(layer, deal, origin, options);
        }

        let (ox, oy) = origin;
        let suits_present = &options.suits_present;
        let num_suits = suits_present.len();
        let show_suit_symbol = options.show_suit_symbols;

        // Layout constants
        let hand_w = self.settings.hand_width;
        let hand_h = self.hand_height_for_suits(num_suits);
        let compass_size = self.compass_box_size();

        // Calculate actual widths for fragment hands
        let north_w = self.actual_fragment_width(&deal.north, suits_present, show_suit_symbol);
        let south_w = self.actual_fragment_width(&deal.south, suits_present, show_suit_symbol);
        let east_w = self.actual_fragment_width(&deal.east, suits_present, show_suit_symbol);
        let west_w = self.actual_fragment_width(&deal.west, suits_present, show_suit_symbol);

        // Calculate vertical offset to center hands with compass
        // Compass is vertically centered with West/East row
        // We want the hand content centered with the compass center
        let compass_center_offset = (compass_size - hand_h) / 2.0;

        // Small gap between compass and South hand
        let compass_hand_gap = 1.5;

        // Calculate compass center position (needed for centering N/S when no suit symbols)
        let north_base_x = ox.0 + hand_w + (compass_size - hand_w) / 2.0;
        let suit_symbol_width = if show_suit_symbol { 5.0 } else { 0.0 };
        let half_char_adjust = if show_suit_symbol { 1.5 } else { 0.0 };
        let compass_center_x =
            north_base_x + suit_symbol_width + compass_size / 2.0 - half_char_adjust;

        // Row 1: North hand (centered above compass)
        // When no suit symbol, center the cards over the compass
        let north_x = if show_suit_symbol {
            north_base_x
        } else {
            compass_center_x - north_w / 2.0
        };
        let north_y = oy.0;
        if !options.hidden.north {
            self.draw_debug_box(layer, north_x, north_y, north_w, hand_h);
            self.render_fragment_hand(
                layer,
                &deal.north,
                (Mm(north_x), Mm(north_y)),
                suits_present,
                show_suit_symbol,
            );
        }

        // Row 2: West hand | Compass | East hand
        let row2_y = north_y - hand_h;

        // Compass positioning
        let compass_y = row2_y - hand_h / 2.0 - compass_center_offset;
        let compass_left = compass_center_x - compass_size / 2.0;
        let compass_right = compass_center_x + compass_size / 2.0;

        // Gap between hands and compass
        let hand_compass_gap = 3.5;

        // West hand - right-justified so right edge is near compass left edge
        let west_y = row2_y - compass_center_offset;
        let west_x = compass_left - hand_compass_gap - west_w;
        if !options.hidden.west {
            self.draw_debug_box(layer, west_x, west_y, west_w, hand_h);
            self.render_fragment_hand(
                layer,
                &deal.west,
                (Mm(west_x), Mm(west_y)),
                suits_present,
                show_suit_symbol,
            );
        }

        // Render compass
        self.draw_debug_box(
            layer,
            compass_left,
            compass_y + compass_size / 2.0,
            compass_size,
            compass_size,
        );
        self.render_compass(layer, (Mm(compass_center_x), Mm(compass_y)));

        // East hand - left edge near compass right edge
        let east_x = compass_right + hand_compass_gap;
        if !options.hidden.east {
            self.draw_debug_box(layer, east_x, west_y, east_w, hand_h);
            self.render_fragment_hand(
                layer,
                &deal.east,
                (Mm(east_x), Mm(west_y)),
                suits_present,
                show_suit_symbol,
            );
        }

        // Row 3: South hand (below compass, centered)
        // Add small gap between compass and South
        let south_y = west_y - hand_h - compass_center_offset - compass_hand_gap;
        // When no suit symbol, center the cards over the compass
        let south_x = if show_suit_symbol {
            north_base_x
        } else {
            compass_center_x - south_w / 2.0
        };
        if !options.hidden.south {
            self.draw_debug_box(layer, south_x, south_y, south_w, hand_h);
            self.render_fragment_hand(
                layer,
                &deal.south,
                (Mm(south_x), Mm(south_y)),
                suits_present,
                show_suit_symbol,
            );
        }

        // Return total height used
        oy.0 - (south_y - hand_h)
    }

    /// Calculate the width of a hand for fragment display
    fn actual_fragment_width(
        &self,
        hand: &Hand,
        suits_present: &[Suit],
        show_suit_symbol: bool,
    ) -> f32 {
        let measurer = text_metrics::get_times_measurer();
        let font_size = self.settings.card_font_size;

        suits_present
            .iter()
            .map(|suit| {
                let holding = hand.holding(*suit);
                let cards_str = if holding.is_void() {
                    "-".to_string()
                } else {
                    holding
                        .ranks
                        .iter()
                        .map(|r| r.display_str().to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                if show_suit_symbol {
                    let line = format!("{} {}", suit.symbol(), cards_str);
                    measurer.measure_width_mm(&line, font_size)
                } else {
                    measurer.measure_width_mm(&cards_str, font_size)
                }
            })
            .fold(0.0_f32, |max, w| max.max(w))
    }

    /// Render a hand showing only the specified suits
    fn render_fragment_hand(
        &self,
        layer: &mut LayerBuilder,
        hand: &Hand,
        origin: (Mm, Mm),
        suits_present: &[Suit],
        show_suit_symbol: bool,
    ) {
        let (ox, oy) = origin;
        let line_height = self.settings.line_height;

        let measurer = text_metrics::get_times_measurer();
        let cap_height = measurer.cap_height_mm(self.settings.card_font_size);

        let first_baseline = oy.0 - cap_height;

        for (i, suit) in suits_present.iter().enumerate() {
            let y = first_baseline - (i as f32 * line_height);
            if show_suit_symbol {
                self.render_suit_line(layer, *suit, hand.holding(*suit), (Mm(ox.0), Mm(y)));
            } else {
                self.render_cards_only(layer, hand.holding(*suit), (Mm(ox.0), Mm(y)));
            }
        }
    }

    /// Render just the cards without a suit symbol (for single-suit fragments)
    fn render_cards_only(
        &self,
        layer: &mut LayerBuilder,
        holding: &crate::model::Holding,
        origin: (Mm, Mm),
    ) {
        let (ox, oy) = origin;

        layer.set_fill_color(Color::Rgb(colors::BLACK));

        let cards_str = if holding.is_void() {
            "-".to_string()
        } else {
            holding
                .ranks
                .iter()
                .map(|r| r.display_str().to_string())
                .collect::<Vec<_>>()
                .join(" ")
        };

        layer.use_text_builtin(&cards_str, self.settings.card_font_size, ox, oy, self.font);
    }

    /// Render a single hand without compass (when only one hand is visible)
    /// Used for full deals with only one hand visible (any direction)
    /// Position matches where North hand would be in full compass layout
    fn render_single_hand(
        &self,
        layer: &mut LayerBuilder,
        deal: &Deal,
        origin: (Mm, Mm),
        options: &DiagramDisplayOptions,
    ) -> f32 {
        let (ox, oy) = origin;
        let hand_w = self.settings.hand_width;
        let hand_h = self.actual_hand_height();
        let compass_size = self.compass_box_size();

        // Get the visible hand based on which direction is visible
        let hand = match options.single_visible_hand {
            Some(Direction::North) => &deal.north,
            Some(Direction::East) => &deal.east,
            Some(Direction::South) => &deal.south,
            Some(Direction::West) => &deal.west,
            None => &deal.north, // Fallback (shouldn't happen)
        };
        let hand_width = self.actual_hand_width(hand);

        // Position at North hand location (same as full compass layout)
        let hand_x = ox.0 + hand_w + (compass_size - hand_w) / 2.0;

        self.draw_debug_box(layer, hand_x, oy.0, hand_width, hand_h);
        self.render_hand_cards(layer, hand, (Mm(hand_x), oy));

        // Return just the height used - layout handles spacing
        hand_h
    }

    /// Render a single hand without compass for fragments (when only one hand is visible)
    /// Used for fragment deals with only one hand visible (any direction)
    /// Cards are centered at the compass center position
    fn render_single_hand_fragment(
        &self,
        layer: &mut LayerBuilder,
        deal: &Deal,
        origin: (Mm, Mm),
        options: &DiagramDisplayOptions,
    ) -> f32 {
        let (ox, oy) = origin;
        let hand_w = self.settings.hand_width;
        let compass_size = self.compass_box_size();
        let suits_present = &options.suits_present;
        let num_suits = suits_present.len();
        let show_suit_symbol = num_suits > 1;
        let hand_h = self.hand_height_for_suits(num_suits);

        // Get the visible hand based on which direction is visible
        let hand = match options.single_visible_hand {
            Some(Direction::North) => &deal.north,
            Some(Direction::East) => &deal.east,
            Some(Direction::South) => &deal.south,
            Some(Direction::West) => &deal.west,
            None => &deal.north, // Fallback (shouldn't happen)
        };
        let hand_width = self.actual_fragment_width(hand, suits_present, show_suit_symbol);

        // Calculate compass center (same formula as in render_deal_fragment)
        let north_base_x = ox.0 + hand_w + (compass_size - hand_w) / 2.0;
        let suit_symbol_width = if show_suit_symbol { 5.0 } else { 0.0 };
        let half_char_adjust = if show_suit_symbol { 1.5 } else { 0.0 };
        let compass_center_x =
            north_base_x + suit_symbol_width + compass_size / 2.0 - half_char_adjust;

        // Center the cards at the compass center position
        let hand_x = compass_center_x - hand_width / 2.0;

        self.draw_debug_box(layer, hand_x, oy.0, hand_width, hand_h);
        self.render_fragment_hand(
            layer,
            hand,
            (Mm(hand_x), oy),
            suits_present,
            show_suit_symbol,
        );

        // Return just the height used - layout handles spacing
        hand_h
    }

    /// Render a single hand (used for backward compatibility)
    pub fn render_hand(
        &self,
        layer: &mut LayerBuilder,
        hand: &Hand,
        origin: (Mm, Mm),
        _show_hcp: bool,
    ) {
        self.render_hand_cards(layer, hand, origin);
    }

    /// Render hand cards only (no HCP)
    /// Origin is the top-left of the visual bounding box
    fn render_hand_cards(&self, layer: &mut LayerBuilder, hand: &Hand, origin: (Mm, Mm)) {
        let (ox, oy) = origin;
        let line_height = self.settings.line_height;

        // Use actual font metrics to get cap-height
        let measurer = text_metrics::get_times_measurer();
        let cap_height = measurer.cap_height_mm(self.settings.card_font_size);

        // First baseline is below the top by cap-height
        // This aligns the top of capital letters with the bounding box top
        let first_baseline = oy.0 - cap_height;

        // Render each suit
        for (i, suit) in SUITS_DISPLAY_ORDER.iter().enumerate() {
            let y = first_baseline - (i as f32 * line_height);
            self.render_suit_line(layer, *suit, hand.holding(*suit), (Mm(ox.0), Mm(y)));
        }
    }

    /// Render a single suit line (symbol + cards)
    fn render_suit_line(
        &self,
        layer: &mut LayerBuilder,
        suit: Suit,
        holding: &crate::model::Holding,
        origin: (Mm, Mm),
    ) {
        let (ox, oy) = origin;

        // Set color based on suit
        let color = self.colors.for_suit(&suit);
        layer.set_fill_color(Color::Rgb(color.clone()));

        // Render suit symbol using symbol font (DejaVu Sans has suit glyphs)
        let symbol = suit.symbol().to_string();
        layer.use_text(
            &symbol,
            self.settings.card_font_size,
            ox,
            oy,
            self.symbol_font,
        );

        // Render cards (in black) using regular font
        layer.set_fill_color(Color::Rgb(colors::BLACK));

        let cards_str = if holding.is_void() {
            "-".to_string()
        } else {
            holding
                .ranks
                .iter()
                .map(|r| r.display_str().to_string())
                .collect::<Vec<_>>()
                .join(" ")
        };

        // Offset for cards (after suit symbol)
        let cards_x = Mm(ox.0 + 5.0);
        layer.use_text_builtin(
            &cards_str,
            self.settings.card_font_size,
            cards_x,
            oy,
            self.font,
        );
    }

    /// Calculate compass box size based on font metrics
    fn compass_box_size(&self) -> f32 {
        let measurer = text_metrics::get_times_measurer();
        let font_size = self.settings.compass_font_size;

        // Measure the widest letter (W is typically widest)
        let w_width = measurer.measure_width_mm("W", font_size);
        let cap_height = measurer.cap_height_mm(font_size);

        // Box needs to fit: letter on each side + padding
        // Width: W on left + gap + W on right + padding on edges
        // Height: N on top + gap + S on bottom + padding on edges
        let letter_size = w_width.max(cap_height);
        let padding = 1.5; // Small padding around letters at edges
        let inner_gap = letter_size * 1.6; // Gap between letters - proportional to letter size

        // Total: padding + letter + gap + letter + padding
        (padding * 2.0) + (letter_size * 2.0) + inner_gap
    }

    /// Render compass rose with green filled box and white letters
    fn render_compass(&self, layer: &mut LayerBuilder, center: (Mm, Mm)) {
        // `%ShowCardTable 0` leaves the space and draws nothing in it
        if !self.settings.show_card_table {
            return;
        }
        let (cx, cy) = center;
        let measurer = text_metrics::get_times_measurer();
        let font_size = self.settings.compass_font_size;

        let box_size = self.compass_box_size();
        let half_box = box_size / 2.0;

        // Get font metrics for positioning
        let cap_height = measurer.cap_height_mm(font_size);
        let n_width = measurer.measure_width_mm("N", font_size);
        let s_width = measurer.measure_width_mm("S", font_size);
        let e_width = measurer.measure_width_mm("E", font_size);

        // Draw filled green rectangle
        layer.set_fill_color(Color::Rgb(colors::GREEN));
        layer.add_rect(
            Mm(cx.0 - half_box),
            Mm(cy.0 - half_box),
            Mm(cx.0 + half_box),
            Mm(cy.0 + half_box),
            PaintMode::Fill,
        );

        // Draw white letters using compass font size
        layer.set_fill_color(Color::Rgb(colors::WHITE));

        let padding = 1.5;

        // N (top center) - baseline positioned so cap-height reaches near top edge
        layer.use_text_builtin(
            "N",
            font_size,
            Mm(cx.0 - n_width / 2.0),
            Mm(cy.0 + half_box - padding - cap_height),
            self.compass_font,
        );

        // S (bottom center) - baseline near bottom edge
        layer.use_text_builtin(
            "S",
            font_size,
            Mm(cx.0 - s_width / 2.0),
            Mm(cy.0 - half_box + padding),
            self.compass_font,
        );

        // W (left center) - vertically centered
        layer.use_text_builtin(
            "W",
            font_size,
            Mm(cx.0 - half_box + padding),
            Mm(cy.0 - cap_height / 2.0),
            self.compass_font,
        );

        // E (right center) - vertically centered
        layer.use_text_builtin(
            "E",
            font_size,
            Mm(cx.0 + half_box - padding - e_width),
            Mm(cy.0 - cap_height / 2.0),
            self.compass_font,
        );
    }

    /// Render HCP box with all four hands' point counts
    /// Origin is top-left of the box
    fn render_hcp_box(
        &self,
        layer: &mut LayerBuilder,
        deal: &Deal,
        origin: (Mm, Mm),
        box_size: f32,
    ) {
        let (ox, oy) = origin;
        let half_box = box_size / 2.0;
        let center_x = ox.0 + half_box;
        let center_y = oy.0 - half_box;

        // Draw debug box (same style as hands)
        self.draw_debug_box(layer, ox.0, oy.0, box_size, box_size);

        // Draw HCP values in compass positions
        layer.set_fill_color(Color::Rgb(colors::BLACK));
        let font_size = self.settings.card_font_size - 1.0;

        // Get HCP values
        let north_hcp = deal.north.total_hcp();
        let south_hcp = deal.south.total_hcp();
        let east_hcp = deal.east.total_hcp();
        let west_hcp = deal.west.total_hcp();

        // Use bold measurer for HCP values
        let bold_measurer = text_metrics::get_times_bold_measurer();

        // N (top center)
        let n_text = format!("{}", north_hcp);
        let n_width = bold_measurer.measure_width_mm(&n_text, font_size);
        layer.use_text_builtin(
            &n_text,
            font_size,
            Mm(center_x - n_width / 2.0),
            Mm(center_y + half_box - 5.0),
            self.bold_font,
        );

        // S (bottom center)
        let s_text = format!("{}", south_hcp);
        let s_width = bold_measurer.measure_width_mm(&s_text, font_size);
        layer.use_text_builtin(
            &s_text,
            font_size,
            Mm(center_x - s_width / 2.0),
            Mm(center_y - half_box + 2.0),
            self.bold_font,
        );

        // W (left center)
        let w_text = format!("{}", west_hcp);
        layer.use_text_builtin(
            &w_text,
            font_size,
            Mm(ox.0 + 2.0),
            Mm(center_y - 1.5),
            self.bold_font,
        );

        // E (right center)
        let e_text = format!("{}", east_hcp);
        let e_width = bold_measurer.measure_width_mm(&e_text, font_size);
        layer.use_text_builtin(
            &e_text,
            font_size,
            Mm(ox.0 + box_size - e_width - 2.0),
            Mm(center_y - 1.5),
            self.bold_font,
        );
    }
}
