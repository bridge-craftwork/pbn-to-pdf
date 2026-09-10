//! Dealer Summary Layout Renderer
//!
//! Generates a simple summary PDF showing dealer, contract, declarer, and opening lead
//! for each board. Displays 6 boards per page in a 2x3 grid with separate boxes.
//!
//! Based on Bridge Composer's DealerSummary.wsf script.

use printpdf::{Color, Mm, PdfDocument, PdfPage, PdfSaveOptions, Rgb};

use crate::config::Settings;
use crate::error::RenderError;
use crate::model::card::RankExt;
use crate::model::Board;

use crate::render::helpers::colors::{SuitColors, BLACK};
use crate::render::helpers::compress::compress_pdf;
use crate::render::helpers::fonts::FontManager;
use crate::render::helpers::layer::LayerBuilder;
use crate::render::helpers::text_metrics::get_helvetica_measurer;

/// Border color for cells
const BORDER_COLOR: Rgb = Rgb {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    icc_profile: None,
};

/// Border thickness
const BORDER_THICKNESS: f32 = 0.5;

/// Fixed box dimensions
const BOX_WIDTH: f32 = 61.0; // mm
const BOX_HEIGHT: f32 = 55.5; // mm

/// Gap between boxes
const BOX_GAP_H: f32 = 15.0; // Horizontal gap between columns
const BOX_GAP_V: f32 = 12.0; // Vertical gap between rows

/// Text start position within box (from top-left corner of box)
const TEXT_OFFSET_X: f32 = 5.0;
const TEXT_OFFSET_Y: f32 = 10.0; // Distance from top of box to first baseline

/// Font size for text
const FONT_SIZE: f32 = 18.0;

/// Line height multiplier
const LINE_HEIGHT_MULT: f32 = 1.4;

/// Boards per page (2 columns × 3 rows)
const BOARDS_PER_PAGE: usize = 6;
const COLS: usize = 2;
const ROWS: usize = 3;

/// Dealer summary renderer
pub struct DealerSummaryRenderer {
    settings: Settings,
}

impl DealerSummaryRenderer {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    /// Generate a PDF with dealer summary (6 boards per page)
    pub fn render(&self, boards: &[Board]) -> Result<Vec<u8>, RenderError> {
        let title = boards
            .first()
            .and_then(|b| b.event.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("Dealer Summary");

        let mut doc = PdfDocument::new(title);

        // Load fonts
        let fonts = FontManager::new(&mut doc)?;

        let mut pages = Vec::new();

        // Process boards in groups of 6
        for chunk in boards.chunks(BOARDS_PER_PAGE) {
            let mut layer = LayerBuilder::new();
            self.render_page(&mut layer, chunk, &fonts);
            pages.push(PdfPage::new(
                Mm(self.settings.page_width),
                Mm(self.settings.page_height),
                layer.into_ops(),
            ));
        }

        doc.with_pages(pages);

        let mut warnings = Vec::new();
        let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

        // Compress PDF streams to reduce file size
        let compressed = compress_pdf(bytes.clone()).unwrap_or(bytes);
        Ok(compressed)
    }

    /// Render a single page with up to 6 boards
    fn render_page(&self, layer: &mut LayerBuilder, boards: &[Board], fonts: &FontManager) {
        let colors = SuitColors::new(self.settings.black_color, self.settings.red_color);

        // Page layout
        let page_width = self.settings.page_width;
        let page_height = self.settings.page_height;

        // Calculate total grid dimensions
        let grid_width = COLS as f32 * BOX_WIDTH + (COLS - 1) as f32 * BOX_GAP_H;
        let grid_height = ROWS as f32 * BOX_HEIGHT + (ROWS - 1) as f32 * BOX_GAP_V;

        // Center the grid on the page
        let grid_start_x = (page_width - grid_width) / 2.0;
        let grid_start_y = page_height - (page_height - grid_height) / 2.0;

        // Render each board
        for (i, board) in boards.iter().enumerate() {
            if i >= BOARDS_PER_PAGE {
                break;
            }

            let col = i % COLS;
            let row = i / COLS;

            // Box position (top-left corner)
            let box_x = grid_start_x + col as f32 * (BOX_WIDTH + BOX_GAP_H);
            let box_y = grid_start_y - row as f32 * (BOX_HEIGHT + BOX_GAP_V);

            // Draw box border
            self.draw_cell_border(layer, box_x, box_y, BOX_WIDTH, BOX_HEIGHT);

            // Render board content (text starts at offset from top-left)
            self.render_board_cell(
                layer,
                board,
                fonts,
                &colors,
                box_x + TEXT_OFFSET_X,
                box_y - TEXT_OFFSET_Y,
            );
        }
    }

    /// Draw border around a cell
    fn draw_cell_border(&self, layer: &mut LayerBuilder, x: f32, y: f32, width: f32, height: f32) {
        layer.set_outline_color(Color::Rgb(BORDER_COLOR));
        layer.set_outline_thickness(BORDER_THICKNESS);

        // Draw rectangle (4 lines)
        let bottom = y - height;
        let right = x + width;

        layer.add_line(Mm(x), Mm(y), Mm(right), Mm(y)); // Top
        layer.add_line(Mm(right), Mm(y), Mm(right), Mm(bottom)); // Right
        layer.add_line(Mm(right), Mm(bottom), Mm(x), Mm(bottom)); // Bottom
        layer.add_line(Mm(x), Mm(bottom), Mm(x), Mm(y)); // Left
    }

    /// Render content for a single board cell
    fn render_board_cell(
        &self,
        layer: &mut LayerBuilder,
        board: &Board,
        fonts: &FontManager,
        colors: &SuitColors,
        x: f32,
        y: f32,
    ) {
        let font_size = FONT_SIZE;
        let line_height = font_size * LINE_HEIGHT_MULT * 0.352778; // Convert pt to mm
        let mut current_y = y;
        let measurer = get_helvetica_measurer(); // Sans-serif measurer for Helvetica

        // Board number
        if let Some(ref board_id) = board.board_id {
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(
                format!("Board: {}", board_id),
                font_size,
                Mm(x),
                Mm(current_y),
                fonts.sans.regular,
            );
            current_y -= line_height;
        }

        // Dealer (bold)
        if let Some(dealer) = board.dealer {
            // "Dealer: " in regular
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(
                "Dealer: ",
                font_size,
                Mm(x),
                Mm(current_y),
                fonts.sans.regular,
            );

            // Measure "Dealer: " width to position the bold name
            let dealer_label_width = measurer.measure_width_mm("Dealer: ", font_size);

            // Direction name in bold
            layer.use_text_builtin(
                format!("{}", dealer),
                font_size,
                Mm(x + dealer_label_width),
                Mm(current_y),
                fonts.sans.bold,
            );
            current_y -= line_height;
        }

        // Extra spacing before contract info
        current_y -= line_height * 0.3;

        // Contract with suit symbol
        if let Some(ref contract) = board.contract {
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(
                "Contract: ",
                font_size,
                Mm(x),
                Mm(current_y),
                fonts.sans.regular,
            );

            let contract_label_width = measurer.measure_width_mm("Contract: ", font_size);
            let mut contract_x = x + contract_label_width;

            // Level
            let level_str = format!("{}", contract.level);
            layer.use_text_builtin(
                &level_str,
                font_size,
                Mm(contract_x),
                Mm(current_y),
                fonts.sans.regular,
            );
            contract_x += measurer.measure_width_mm(&level_str, font_size);

            // Suit symbol (colored) - use symbol font for suits, builtin for NT
            let suit_color = if contract.strain.is_red() {
                colors.hearts.clone()
            } else {
                BLACK
            };
            layer.set_fill_color(Color::Rgb(suit_color));
            let suit_str = contract.strain.symbol();
            if contract.strain == crate::model::BidSuit::NoTrump {
                layer.use_text_builtin(
                    suit_str,
                    font_size,
                    Mm(contract_x),
                    Mm(current_y),
                    fonts.sans.regular,
                );
            } else {
                layer.use_text(
                    suit_str,
                    font_size,
                    Mm(contract_x),
                    Mm(current_y),
                    fonts.symbol_font(),
                );
            }
            contract_x += measurer.measure_width_mm(suit_str, font_size);

            // Doubled/Redoubled indicator
            layer.set_fill_color(Color::Rgb(BLACK));
            if contract.redoubled {
                layer.use_text_builtin(
                    "XX",
                    font_size,
                    Mm(contract_x),
                    Mm(current_y),
                    fonts.sans.regular,
                );
            } else if contract.doubled {
                layer.use_text_builtin(
                    "X",
                    font_size,
                    Mm(contract_x),
                    Mm(current_y),
                    fonts.sans.regular,
                );
            }

            current_y -= line_height;
        }

        // Declarer
        if let Some(ref contract) = board.contract {
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(
                format!("Declarer: {}", contract.declarer),
                font_size,
                Mm(x),
                Mm(current_y),
                fonts.sans.regular,
            );
            current_y -= line_height;
        }

        // Opening lead (only show if we have lead data)
        if let Some(ref play) = board.play {
            if let Some(first_trick) = play.tricks.first() {
                if let Some(lead_card) = first_trick.cards[0] {
                    layer.set_fill_color(Color::Rgb(BLACK));
                    layer.use_text_builtin(
                        "Lead: ",
                        font_size,
                        Mm(x),
                        Mm(current_y),
                        fonts.sans.regular,
                    );

                    let lead_label_width = measurer.measure_width_mm("Lead: ", font_size);
                    let mut lead_x = x + lead_label_width;

                    // Suit symbol (colored) - use symbol font
                    let suit_color = if lead_card.suit.is_red() {
                        colors.hearts.clone()
                    } else {
                        BLACK
                    };
                    layer.set_fill_color(Color::Rgb(suit_color));
                    let suit_str = lead_card.suit.symbol().to_string();
                    layer.use_text(
                        &suit_str,
                        font_size,
                        Mm(lead_x),
                        Mm(current_y),
                        fonts.symbol_font(),
                    );
                    lead_x += measurer.measure_width_mm(&suit_str, font_size);

                    // Rank
                    layer.set_fill_color(Color::Rgb(BLACK));
                    layer.use_text_builtin(
                        lead_card.rank.display_str().to_string(),
                        font_size,
                        Mm(lead_x),
                        Mm(current_y),
                        fonts.sans.regular,
                    );
                }
            }
        }
    }
}
