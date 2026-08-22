//! Declarer's Plan Layout Renderers (1-up, 2-up, 4-up)
//!
//! Generates PDF documents for declarer play practice.
//! Three layout variants:
//! - **1-up**: One deal per page at full size
//! - **2-up**: Two deals side by side on a landscape page
//! - **4-up**: Four deals per page in a 2x2 grid (original layout)

use printpdf::{Color, Mm, PdfDocument, PdfPage, PdfSaveOptions, Rgb};
use std::collections::{HashMap, HashSet};

use crate::config::Settings;
use crate::error::RenderError;
use crate::model::analysis::{find_length_winners, find_promotable_winners, find_sure_winners};
use crate::model::{BidSuit, Board, Card, Deal, Direction, Hand};

use crate::render::components::DeclarersPlanSmallRenderer;
use crate::render::helpers::card_assets::{CardAssets, CardFace};
use crate::render::helpers::colors::{SuitColors, BLUE, GREEN, RED};
use crate::render::helpers::compress::compress_pdf;
use crate::render::helpers::fonts::FontManager;
use crate::render::helpers::layer::LayerBuilder;

/// Separator line thickness
const SEPARATOR_THICKNESS: f32 = 2.0;

/// Separator line color (dark gray)
const SEPARATOR_COLOR: Rgb = Rgb {
    r: 0.3,
    g: 0.3,
    b: 0.3,
    icc_profile: None,
};

/// Padding inside each panel
const PANEL_PADDING: f32 = 5.0;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Prepared board data ready for rendering
struct PreparedBoard<'a> {
    dummy_hand: Hand,
    declarer_hand: Hand,
    is_nt: bool,
    opening_lead: Option<crate::model::Card>,
    deal_number: Option<u32>,
    contract_str: Option<String>,
    trump: Option<BidSuit>,
    _board: &'a Board,
}

fn prepare_board(board: &Board) -> PreparedBoard<'_> {
    let is_nt = board
        .contract
        .as_ref()
        .map(|c| c.suit == BidSuit::NoTrump)
        .unwrap_or(false);

    let opening_lead = board
        .play
        .as_ref()
        .and_then(|play| play.tricks.first().and_then(|trick| trick.cards[0]));

    let contract_str = board.contract.as_ref().map(|c| {
        let suit_symbol = match c.suit {
            BidSuit::Clubs => "♣",
            BidSuit::Diamonds => "♦",
            BidSuit::Hearts => "♥",
            BidSuit::Spades => "♠",
            BidSuit::NoTrump => "NT",
        };
        format!("{}{}", c.level, suit_symbol)
    });

    let declarer = board
        .contract
        .as_ref()
        .map(|c| c.declarer)
        .unwrap_or(Direction::South);
    let (dummy_hand, declarer_hand) = rotate_deal_for_declarer(&board.deal, declarer);

    let trump = board.contract.as_ref().map(|c| c.suit);

    PreparedBoard {
        dummy_hand,
        declarer_hand,
        is_nt,
        opening_lead,
        deal_number: board.number,
        contract_str,
        trump,
        _board: board,
    }
}

/// Rotate a deal so that the declarer is always South.
/// Returns (dummy_hand, declarer_hand).
fn rotate_deal_for_declarer(deal: &Deal, declarer: Direction) -> (Hand, Hand) {
    match declarer {
        Direction::South => (deal.north.clone(), deal.south.clone()),
        Direction::North => (deal.south.clone(), deal.north.clone()),
        Direction::East => (deal.west.clone(), deal.east.clone()),
        Direction::West => (deal.east.clone(), deal.west.clone()),
    }
}

/// The card renditions a whole document draws.
///
/// Registering all 52 full-size cards costs ~3.9MB because printpdf writes
/// every XObject a document owns, drawn or not, and the twelve court-card
/// illustrations dominate that. Collecting the set up front lets each document
/// pay only for the renditions its boards actually use -- typically the
/// reduced band and corner variants, with a full court card only on the rare
/// board that exposes one.
fn required_faces(boards: &[Board]) -> HashSet<(crate::model::Suit, crate::model::Rank, CardFace)> {
    let mut needed = HashSet::new();
    for board in boards {
        let prep = prepare_board(board);
        needed.extend(DeclarersPlanSmallRenderer::required_faces(
            &prep.dummy_hand,
            &prep.declarer_hand,
            prep.trump,
        ));
    }
    needed
}

/// Baseline card scale (4-up) — layout_scale is relative to this
const BASELINE_CARD_SCALE: f32 = SCALE_4UP;

/// Create the shared component renderer with given card scale
fn make_renderer<'a>(
    card_assets: &'a CardAssets,
    fonts: &'a FontManager,
    settings: &Settings,
    card_scale: f32,
) -> DeclarersPlanSmallRenderer<'a> {
    let colors = SuitColors::new(settings.black_color, settings.red_color);
    let layout_scale = card_scale / BASELINE_CARD_SCALE;
    DeclarersPlanSmallRenderer::new(
        card_assets,
        fonts.serif.regular,
        fonts.serif.bold,
        fonts.symbol_font(),
        colors,
    )
    .card_scale(card_scale)
    .layout_scale(layout_scale)
    .show_bounds(settings.debug_boxes)
}

/// Compute cards to circle for a board based on CLI flags.
/// Sure winners use red, promotable winners green, length winners blue.
/// If the same card is identified by multiple analyses, the first match wins.
fn circled_cards_for_board(
    settings: &Settings,
    dummy: &Hand,
    declarer: &Hand,
) -> HashMap<Card, Rgb> {
    let mut circled: HashMap<Card, Rgb> = HashMap::new();
    if settings.circle_sure_winners {
        for card in find_sure_winners(dummy, declarer) {
            circled.entry(card).or_insert(RED);
        }
    }
    if settings.circle_promotable_winners {
        let result = find_promotable_winners(dummy, declarer);
        for card in result.winners {
            circled.entry(card).or_insert(GREEN);
        }
    }
    if settings.circle_length_winners {
        let result = find_length_winners(dummy, declarer);
        for card in result.winners {
            circled.entry(card).or_insert(BLUE);
        }
    }
    circled
}

/// Build a renderer for a specific board, applying circled cards based on settings
fn renderer_for_board<'a>(
    card_assets: &'a CardAssets,
    fonts: &'a FontManager,
    settings: &Settings,
    card_scale: f32,
    dummy: &Hand,
    declarer: &Hand,
) -> DeclarersPlanSmallRenderer<'a> {
    let renderer = make_renderer(card_assets, fonts, settings, card_scale);
    let circled = circled_cards_for_board(settings, dummy, declarer);
    if circled.is_empty() {
        renderer
    } else {
        renderer.circled_cards(circled)
    }
}

/// Render a single prepared board into a layer
fn render_prepared(
    renderer: &DeclarersPlanSmallRenderer<'_>,
    layer: &mut LayerBuilder,
    board: &PreparedBoard<'_>,
    origin: (Mm, Mm),
) {
    renderer.render_with_info(
        layer,
        &board.dummy_hand,
        &board.declarer_hand,
        board.is_nt,
        board.opening_lead,
        board.deal_number,
        board.contract_str.as_deref(),
        board.trump,
        origin,
    );
}

/// Draw the divider between two side-by-side panels on a landscape page.
fn draw_vertical_separator(
    layer: &mut LayerBuilder,
    settings: &Settings,
    x: f32,
    page_height: f32,
) {
    layer.set_outline_color(Color::Rgb(SEPARATOR_COLOR));
    layer.set_outline_thickness(SEPARATOR_THICKNESS);
    layer.add_line(
        Mm(x),
        Mm(settings.margin_bottom),
        Mm(x),
        Mm(page_height - settings.margin_top),
    );
}

/// Generate the final PDF bytes from a document
fn finalize_pdf(doc: PdfDocument, pages: Vec<PdfPage>) -> Result<Vec<u8>, RenderError> {
    let mut doc = doc;
    doc.with_pages(pages);
    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    let compressed = compress_pdf(bytes.clone()).unwrap_or(bytes);
    Ok(compressed)
}

// ---------------------------------------------------------------------------
// 1-Up Renderer
// ---------------------------------------------------------------------------

/// Card scale for 1-up layout (one deal fills the page)
const SCALE_1UP: f32 = 0.55;

/// Declarer's plan 1-up renderer — one deal per page
pub struct DeclarersPlan1UpRenderer {
    settings: Settings,
}

impl DeclarersPlan1UpRenderer {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    pub fn render(&self, boards: &[Board]) -> Result<Vec<u8>, RenderError> {
        let title = boards
            .first()
            .and_then(|b| b.event.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("Declarer's Plan");

        let mut doc = PdfDocument::new(title);
        let fonts = FontManager::new(&mut doc)?;
        let card_assets = CardAssets::load_faces(&mut doc, &required_faces(boards))
            .map_err(|e| RenderError::CardAsset(e.to_string()))?;

        let mut pages = Vec::new();

        for board in boards {
            let prep = prepare_board(board);
            let renderer = renderer_for_board(
                &card_assets,
                &fonts,
                &self.settings,
                SCALE_1UP,
                &prep.dummy_hand,
                &prep.declarer_hand,
            );
            let mut layer = LayerBuilder::new();

            // Center the panel on the page
            let content_width =
                self.settings.page_width - self.settings.margin_left - self.settings.margin_right;
            let content_height =
                self.settings.page_height - self.settings.margin_top - self.settings.margin_bottom;

            let (panel_w, panel_h) =
                renderer.dimensions(&prep.dummy_hand, &prep.declarer_hand, prep.is_nt);

            let origin_x = self.settings.margin_left + (content_width - panel_w) / 2.0;
            let origin_y = self.settings.page_height
                - self.settings.margin_top
                - (content_height - panel_h) / 2.0;

            render_prepared(&renderer, &mut layer, &prep, (Mm(origin_x), Mm(origin_y)));

            pages.push(PdfPage::new(
                Mm(self.settings.page_width),
                Mm(self.settings.page_height),
                layer.into_ops(),
            ));
        }

        finalize_pdf(doc, pages)
    }
}

// ---------------------------------------------------------------------------
// 2-Up Renderer (landscape page)
// ---------------------------------------------------------------------------

/// Card scale for 2-up layout
const SCALE_2UP: f32 = 0.45;

/// Declarer's plan 2-up renderer — two deals side by side on a landscape page
///
/// The tall declarer's-plan panel needs more height than width, so two of them
/// sit naturally side by side across a landscape sheet. This previously drew
/// them on a portrait page with each panel rotated a quarter turn, which put
/// the same ink on the paper but left the page portrait: every reader had to
/// turn the sheet (or the tablet) to use it, and a merged handout gave no way
/// to tell these pages apart from the upright ones.
///
/// Emitting a genuinely landscape page fixes both. It also removes the rotation
/// entirely — on a landscape page the panels are simply drawn upright — and it
/// is what lets `pdf-handouts` recognise the page (it keys on the MediaBox
/// being wider than it is tall) and turn the title and footer a quarter turn
/// onto the short edges, so they still line up with every other page in the
/// stack when printed.
pub struct DeclarersPlan2UpRenderer {
    settings: Settings,
}

impl DeclarersPlan2UpRenderer {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    pub fn render(&self, boards: &[Board]) -> Result<Vec<u8>, RenderError> {
        let title = boards
            .first()
            .and_then(|b| b.event.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("Declarer's Plan");

        let mut doc = PdfDocument::new(title);
        let fonts = FontManager::new(&mut doc)?;
        let card_assets = CardAssets::load_faces(&mut doc, &required_faces(boards))
            .map_err(|e| RenderError::CardAsset(e.to_string()))?;

        // The page is landscape: the settings carry portrait letter, so the two
        // dimensions are swapped here rather than page size being plumbed through
        // every layout. The margins keep their physical meaning (left/right are
        // still the page's left and right).
        let page_width = self.settings.page_height;
        let page_height = self.settings.page_width;

        let content_width = page_width - self.settings.margin_left - self.settings.margin_right;
        let content_height = page_height - self.settings.margin_top - self.settings.margin_bottom;
        let half_width = content_width / 2.0;
        let center_x = self.settings.margin_left + half_width;

        // Slot centers (in page coordinates). The two panels sit side by side,
        // each nudged away from the divider so they are not crowded against it.
        let center_inset = PANEL_PADDING * 2.0;
        let slot_cy = self.settings.margin_bottom + content_height / 2.0;
        let left_slot_cx = center_x - half_width / 2.0 - center_inset / 2.0;
        let right_slot_cx = center_x + half_width / 2.0 + center_inset / 2.0;

        let mut pages = Vec::new();

        for chunk in boards.chunks(2) {
            let mut layer = LayerBuilder::new();

            // Draw vertical separator between the side-by-side panels
            draw_vertical_separator(&mut layer, &self.settings, center_x, page_height);

            let slot_centers = [(left_slot_cx, slot_cy), (right_slot_cx, slot_cy)];

            for (i, board) in chunk.iter().enumerate() {
                let prep = prepare_board(board);
                let renderer = renderer_for_board(
                    &card_assets,
                    &fonts,
                    &self.settings,
                    SCALE_2UP,
                    &prep.dummy_hand,
                    &prep.declarer_hand,
                );
                let (dest_cx, dest_cy) = slot_centers[i];

                // The panel is drawn upright: on a landscape page its slot is
                // already taller than it is wide, which is the shape the
                // declarer's-plan panel wants. No rotation is involved.
                let slot_h = content_height - PANEL_PADDING * 2.0;

                let (panel_w, panel_h) =
                    renderer.dimensions(&prep.dummy_hand, &prep.declarer_hand, prep.is_nt);

                // Centre the panel in its slot. The origin is the panel's top-left,
                // measured from the page's bottom-left like every other coordinate
                // here, so the top edge is the slot centre plus half the height.
                let panel_ox = dest_cx - panel_w / 2.0;
                let panel_oy = dest_cy + panel_h.min(slot_h) / 2.0;

                render_prepared(&renderer, &mut layer, &prep, (Mm(panel_ox), Mm(panel_oy)));
            }

            pages.push(PdfPage::new(
                Mm(page_width),
                Mm(page_height),
                layer.into_ops(),
            ));
        }

        finalize_pdf(doc, pages)
    }
}

// ---------------------------------------------------------------------------
// 4-Up Renderer (original)
// ---------------------------------------------------------------------------

/// Card scale for 4-up layout
const SCALE_4UP: f32 = 0.35;

/// Declarer's plan 4-up renderer — four deals per page in a 2x2 grid
pub struct DeclarersPlanRenderer {
    settings: Settings,
}

impl DeclarersPlanRenderer {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    /// Generate a PDF with declarer's plan practice sheets (4 per page)
    pub fn render(&self, boards: &[Board]) -> Result<Vec<u8>, RenderError> {
        let title = boards
            .first()
            .and_then(|b| b.event.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("Declarer's Plan Practice");

        let mut doc = PdfDocument::new(title);
        let fonts = FontManager::new(&mut doc)?;
        let card_assets = CardAssets::load_faces(&mut doc, &required_faces(boards))
            .map_err(|e| RenderError::CardAsset(e.to_string()))?;

        let mut pages = Vec::new();

        for chunk in boards.chunks(4) {
            let mut layer = LayerBuilder::new();
            self.render_page(&mut layer, chunk, &fonts, &card_assets);
            pages.push(PdfPage::new(
                Mm(self.settings.page_width),
                Mm(self.settings.page_height),
                layer.into_ops(),
            ));
        }

        finalize_pdf(doc, pages)
    }

    /// Render a single page with up to 4 deals
    fn render_page(
        &self,
        layer: &mut LayerBuilder,
        boards: &[Board],
        fonts: &FontManager,
        card_assets: &CardAssets,
    ) {
        let margin_left = self.settings.margin_left;
        let margin_right = self.settings.margin_right;
        let margin_top = self.settings.margin_top;
        let margin_bottom = self.settings.margin_bottom;
        let page_width = self.settings.page_width;
        let page_height = self.settings.page_height;

        let content_width = page_width - margin_left - margin_right;
        let content_height = page_height - margin_top - margin_bottom;

        let half_width = content_width / 2.0;
        let half_height = content_height / 2.0;

        let center_x = margin_left + half_width;
        let center_y = margin_bottom + half_height;

        // Draw separator lines
        self.draw_separator_lines(layer, center_x, center_y);

        // Origins for each quadrant (top-left corner of each, with padding)
        let positions = [
            (margin_left + PANEL_PADDING, page_height - margin_top), // Top-left
            (center_x + PANEL_PADDING, page_height - margin_top),    // Top-right
            (margin_left + PANEL_PADDING, center_y),                 // Bottom-left
            (center_x + PANEL_PADDING, center_y),                    // Bottom-right
        ];

        for (i, board) in boards.iter().enumerate() {
            if i >= 4 {
                break;
            }

            let (x, y) = positions[i];
            let prep = prepare_board(board);
            let renderer = renderer_for_board(
                card_assets,
                fonts,
                &self.settings,
                SCALE_4UP,
                &prep.dummy_hand,
                &prep.declarer_hand,
            );
            render_prepared(&renderer, layer, &prep, (Mm(x), Mm(y)));
        }
    }

    /// Draw horizontal and vertical separator lines between quadrants
    fn draw_separator_lines(&self, layer: &mut LayerBuilder, center_x: f32, center_y: f32) {
        let margin_left = self.settings.margin_left;
        let margin_right = self.settings.margin_right;
        let margin_top = self.settings.margin_top;
        let margin_bottom = self.settings.margin_bottom;
        let page_width = self.settings.page_width;
        let page_height = self.settings.page_height;

        layer.set_outline_color(Color::Rgb(SEPARATOR_COLOR));
        layer.set_outline_thickness(SEPARATOR_THICKNESS);

        // Vertical line
        layer.add_line(
            Mm(center_x),
            Mm(margin_bottom),
            Mm(center_x),
            Mm(page_height - margin_top),
        );

        // Horizontal line
        layer.add_line(
            Mm(margin_left),
            Mm(center_y),
            Mm(page_width - margin_right),
            Mm(center_y),
        );
    }
}
