//! Hand record layout (issue #31): BridgeComposer's *View Hand Record*.
//!
//! Compact boards, eighteen to a page, in a four-by-five grid whose first two
//! cells hold a title block on every page. Each cell carries the dealer and
//! vulnerability, a large board number, the four hands in compass order with
//! no card table, and the HCP compass -- and nothing else: no auction,
//! contract, lead or commentary. Measured against BridgeComposer 5.118.2's
//! rendering of Andrew Rowberg's `%BoardsPerPage 18` file.
//!
//! The title block is page furniture (#28): the omit flag drops it, leaving
//! its two cells empty so the boards keep their places.

use printpdf::{BuiltinFont, Color, Mm, PdfDocument, PdfPage, PdfSaveOptions, Rgb};

use crate::config::Settings;
use crate::error::RenderError;
use crate::model::card::RankExt;
use crate::model::{Board, Direction, Hand, Suit, SUITS_DISPLAY_ORDER};
use crate::render::components::page_furniture::spelled_date;
use crate::render::helpers::colors::{SuitColors, BLACK};
use crate::render::helpers::compress::compress_pdf;
use crate::render::helpers::fonts::FontManager;
use crate::render::helpers::layer::LayerBuilder;
use crate::render::helpers::text_metrics::BuiltinFontMeasurer;

const COLS: usize = 4;
const ROWS: usize = 5;
/// The title block takes the first two cells of every page
const TITLE_CELLS: usize = 2;
/// Boards on a page: the grid less the title block
pub const BOARDS_PER_PAGE: usize = COLS * ROWS - TITLE_CELLS;

/// Grid lines
const RULE_COLOR: Rgb = Rgb {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    icc_profile: None,
};
const RULE_THICKNESS: f32 = 0.5;

/// Millimetres per point
const PT: f32 = 0.3528;
/// Point sizes: the dealer and vulnerability, the board number, the HCP compass
const LABEL_SIZE: f32 = 7.0;
const NUMBER_SIZE: f32 = 20.0;
const HCP_SIZE: f32 = 7.0;
/// Inset of a cell's contents from its edges, in mm
const INSET: f32 = 1.5;

/// Renders the hand record layout
pub struct HandRecordRenderer {
    settings: Settings,
}

/// Where one cell sits on the page: top-left corner and size, in mm
#[derive(Clone, Copy)]
struct Cell {
    x: f32,
    top: f32,
    width: f32,
    height: f32,
}

impl HandRecordRenderer {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    pub fn render(&self, boards: &[Board]) -> Result<Vec<u8>, RenderError> {
        let title = self.title(boards).unwrap_or("Hand Record").to_string();
        let mut doc = PdfDocument::new(&title);
        let fonts = FontManager::new(&mut doc)?;

        let pages: Vec<PdfPage> = boards
            .chunks(BOARDS_PER_PAGE)
            .enumerate()
            .map(|(page, chunk)| {
                let mut layer = LayerBuilder::new();
                self.draw_grid(&mut layer);
                if self.settings.page_furniture {
                    self.draw_title_block(&mut layer, &fonts, &title, page + 1);
                }
                for (i, board) in chunk.iter().enumerate() {
                    self.draw_board(&mut layer, &fonts, board, self.cell(i + TITLE_CELLS));
                }
                PdfPage::new(
                    Mm(self.settings.page_width),
                    Mm(self.settings.page_height),
                    layer.into_ops(),
                )
            })
            .collect();

        doc.with_pages(pages);
        let mut warnings = Vec::new();
        let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
        Ok(compress_pdf(bytes.clone()).unwrap_or(bytes))
    }

    /// The title: `%HRTitleEvent`, else the first board's event
    fn title<'a>(&'a self, boards: &'a [Board]) -> Option<&'a str> {
        self.settings
            .title_from_metadata
            .as_deref()
            .or_else(|| boards.first().and_then(|b| b.event.as_deref()))
            .filter(|t| !t.trim().is_empty())
    }

    /// The `index`th cell of the grid, row by row
    fn cell(&self, index: usize) -> Cell {
        let s = &self.settings;
        let width = s.content_width() / COLS as f32;
        let height = s.content_height() / ROWS as f32;
        Cell {
            x: s.margin_left + (index % COLS) as f32 * width,
            top: s.page_height - s.margin_top - (index / COLS) as f32 * height,
            width,
            height,
        }
    }

    /// The rules between the cells, with none between the two the title
    /// block spans
    fn draw_grid(&self, layer: &mut LayerBuilder) {
        let s = &self.settings;
        let left = s.margin_left;
        let right = s.page_width - s.margin_right;
        let top = s.page_height - s.margin_top;
        let bottom = s.margin_bottom;
        let cell = self.cell(0);

        layer.set_outline_color(Color::Rgb(RULE_COLOR));
        layer.set_outline_thickness(RULE_THICKNESS);
        for row in 0..=ROWS {
            let y = top - row as f32 * cell.height;
            layer.add_line(Mm(left), Mm(y), Mm(right), Mm(y));
        }
        for col in 0..=COLS {
            let x = left + col as f32 * cell.width;
            // The title block spans the first two cells of the top row
            let from = if col == 1 { top - cell.height } else { top };
            layer.add_line(Mm(x), Mm(from), Mm(x), Mm(bottom));
        }
    }

    /// The title and its date, centred in the first two cells, and the page
    /// number in their corner
    fn draw_title_block(
        &self,
        layer: &mut LayerBuilder,
        fonts: &FontManager,
        title: &str,
        page: usize,
    ) {
        let s = &self.settings;
        let font = self.font_set(fonts).regular;
        let measurer = BuiltinFontMeasurer::new(font);
        let first = self.cell(0);
        let width = first.width * TITLE_CELLS as f32;
        let size = s.body_font_size.max(10.0);
        let middle = first.top - first.height / 2.0;

        let date = s.title_date.as_deref().and_then(spelled_date);
        let lines: Vec<&str> = [Some(title), date.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        let line_gap = 1.3 * size * PT;
        layer.set_fill_color(Color::Rgb(BLACK));
        for (i, line) in lines.iter().enumerate() {
            let text_width = measurer.measure_width_mm(line, size);
            let y = middle + (lines.len() as f32 / 2.0 - i as f32 - 0.7) * line_gap;
            layer.use_text_builtin(
                *line,
                size,
                Mm(first.x + (width - text_width) / 2.0),
                Mm(y),
                font,
            );
        }

        let page_label = format!("Page {page}");
        let small = LABEL_SIZE + 1.0;
        let label_width = measurer.measure_width_mm(&page_label, small);
        layer.use_text_builtin(
            page_label,
            small,
            Mm(first.x + width - INSET - label_width),
            Mm(first.top - first.height + INSET + measurer.descender_mm(small)),
            font,
        );
    }

    fn font_set(&self, fonts: &FontManager) -> crate::render::helpers::fonts::BuiltinFontSet {
        fonts.builtin_set_for_spec(self.settings.fonts.hand_record.as_ref())
    }

    /// One board: labels, number, the four hands and the HCP compass
    fn draw_board(&self, layer: &mut LayerBuilder, fonts: &FontManager, board: &Board, cell: Cell) {
        let set = self.font_set(fonts);
        let measurer = BuiltinFontMeasurer::new(set.regular);
        let colors = SuitColors::new(self.settings.black_color, self.settings.red_color);
        let flags = board.bc_flags;

        // Dealer and vulnerability, small, top left
        layer.set_fill_color(Color::Rgb(BLACK));
        let mut label_y = cell.top - INSET - measurer.ascender_mm(LABEL_SIZE);
        if let Some(dealer) = board
            .dealer
            .filter(|_| !flags.is_some_and(|f| f.hide_dealer()))
        {
            let text = format!("{} Deals", dealer.to_char());
            layer.use_text_builtin(
                text,
                LABEL_SIZE,
                Mm(cell.x + INSET),
                Mm(label_y),
                set.regular,
            );
            label_y -= 1.15 * LABEL_SIZE * PT;
        }
        if !flags.is_some_and(|f| f.hide_vulnerable()) {
            layer.use_text_builtin(
                board.vulnerable.to_string(),
                LABEL_SIZE,
                Mm(cell.x + INSET),
                Mm(label_y),
                set.regular,
            );
        }

        // The board number, large, top right
        if let Some(id) = board.board_id.as_deref() {
            let width = measurer.measure_width_mm(id, NUMBER_SIZE);
            layer.use_text_builtin(
                id,
                NUMBER_SIZE,
                Mm(cell.x + cell.width - INSET - width),
                Mm(cell.top - INSET - measurer.cap_height_mm(NUMBER_SIZE)),
                set.regular,
            );
        }

        if board.deal.is_empty() {
            return;
        }

        // Twelve lines of hands -- North, then West and East, then South --
        // fitted to the cell, in the hand record font at most
        let pitch = (cell.height - 2.0 * INSET) / 12.0;
        let size = self.settings.body_font_size.min(pitch / (1.12 * PT));
        let first_baseline = cell.top - INSET - measurer.ascender_mm(size);
        let row_top = |line: usize| first_baseline - line as f32 * pitch;
        // East sits where BridgeComposer puts it, or further left when its
        // longest suit would otherwise run over the cell's edge
        let east_width = SUITS_DISPLAY_ORDER
            .iter()
            .map(|suit| {
                let symbol = suit.symbol().to_string();
                measurer.measure_width_mm(&symbol, size)
                    + 0.4
                    + measurer
                        .measure_width_mm(&ranks(board.deal.hand(Direction::East), *suit), size)
            })
            .fold(0.0_f32, f32::max);
        let east_x = (cell.x + 0.62 * cell.width).min(cell.x + cell.width - INSET - east_width);
        let hands = [
            (Direction::North, cell.x + 0.38 * cell.width, row_top(0)),
            (Direction::West, cell.x + INSET, row_top(4)),
            (Direction::East, east_x, row_top(4)),
            (Direction::South, cell.x + 0.38 * cell.width, row_top(8)),
        ];
        for (seat, x, y) in hands {
            if !board.hidden.is_hidden(seat) {
                let hand = board.deal.hand(seat);
                self.draw_hand(layer, fonts, set.regular, &colors, hand, x, y, pitch, size);
            }
        }

        // The HCP compass, bottom left, level with South. Every hand record
        // carries it, ShowHCP or not, as BridgeComposer's does
        let centre_x = cell.x + 0.16 * cell.width;
        let centre_y = row_top(9) - pitch / 2.0;
        let spread = 2.2 * HCP_SIZE * PT;
        let points = [
            (Direction::North, 0.0, spread * 0.8),
            (Direction::West, -spread, 0.0),
            (Direction::East, spread, 0.0),
            (Direction::South, 0.0, -spread * 0.8),
        ];
        for (seat, dx, dy) in points {
            let hcp = board.deal.hand(seat).total_hcp().to_string();
            let width = measurer.measure_width_mm(&hcp, HCP_SIZE);
            layer.use_text_builtin(
                hcp,
                HCP_SIZE,
                Mm(centre_x + dx - width / 2.0),
                Mm(centre_y + dy),
                set.regular,
            );
        }
    }

    /// One hand: a line a suit, the symbol and then the ranks run together,
    /// `--` for a void
    #[allow(clippy::too_many_arguments)]
    fn draw_hand(
        &self,
        layer: &mut LayerBuilder,
        fonts: &FontManager,
        font: BuiltinFont,
        colors: &SuitColors,
        hand: &Hand,
        x: f32,
        first_baseline: f32,
        pitch: f32,
        size: f32,
    ) {
        let measurer = BuiltinFontMeasurer::new(font);
        for (i, suit) in SUITS_DISPLAY_ORDER.iter().enumerate() {
            let y = first_baseline - i as f32 * pitch;
            let symbol = suit.symbol().to_string();
            layer.set_fill_color(Color::Rgb(colors.for_suit(suit)));
            layer.use_text(&symbol, size, Mm(x), Mm(y), fonts.symbol_font());
            let cards = ranks(hand, *suit);
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(
                cards,
                size,
                Mm(x + measurer.measure_width_mm(&symbol, size) + 0.4),
                Mm(y),
                font,
            );
        }
    }
}

/// A suit's ranks run together, as a hand record prints them; `--` for a void
fn ranks(hand: &Hand, suit: Suit) -> String {
    let holding = hand.holding(suit);
    if holding.is_void() {
        return "--".to_string();
    }
    holding.ranks.iter().map(|r| r.display_str()).collect()
}
