//! Bidding Sheets Layout Renderer
//!
//! Generates PDF documents for face-to-face bidding practice.
//! Each board set produces:
//! 1. North practice page (shows only North's hand)
//! 2. Answers page (shows both hands + auction)
//! 3. South practice page (shows only South's hand)
//! 4. Answers page (repeated for duplex printing)

use printpdf::{BuiltinFont, Color, FontId, Mm, PaintMode, PdfDocument, PdfPage, Rgb};

use crate::config::Settings;
use crate::error::RenderError;
use crate::model::{
    AnnotatedCall, Auction, BidSuit, Board, Call, Direction, DirectionExt, Hand, Suit,
    Vulnerability,
};

use crate::render::helpers::colors::{SuitColors, BLACK, WHITE};
use crate::render::helpers::compress::compress_pdf;
use crate::render::helpers::fonts::FontManager;
use crate::render::helpers::layer::{save_options, LayerBuilder};
use crate::render::helpers::text_metrics::{
    get_helvetica_bold_measurer, get_helvetica_measurer, get_times_measurer, TextMeasure,
};

/// Light gray color for debug boxes
const DEBUG_BOX_COLOR: Rgb = Rgb {
    r: 0.7,
    g: 0.7,
    b: 0.7,
    icc_profile: None,
};
// Debug boxes are now controlled via settings.debug_boxes

/// Font sizes for bidding sheets
const PRACTICE_FONT_SIZE: f32 = 16.0;
const ANSWERS_FONT_SIZE: f32 = 12.0;
const HEADER_FONT_SIZE: f32 = 17.0;

/// Line height multiplier
const LINE_HEIGHT_MULTIPLIER: f32 = 1.4;

/// Superscript ratio relative to body font
const SUPERSCRIPT_RATIO: f32 = 0.65;
/// Superscript vertical rise as fraction of font size
const SUPERSCRIPT_RISE: f32 = 0.4;

/// Banner height in mm
const BANNER_HEIGHT: f32 = 10.0;
/// Gap after banner before content
const AFTER_BANNER_GAP: f32 = 10.0;

/// Column widths (in mm)
const CONTEXT_COLUMN_WIDTH: f32 = 45.0;
const HAND_COLUMN_WIDTH: f32 = 35.0;

/// Row spacing
const ROW_GAP: f32 = 5.0;

/// Separator line thickness for practice pages (thick, in middle of gap)
const PRACTICE_SEPARATOR_THICKNESS: f32 = 2.0;
/// Separator line thickness for answers pages (thin, overlaid on gap)
const ANSWERS_SEPARATOR_THICKNESS: f32 = 1.0;

/// Bidding sheets renderer
pub struct BiddingSheetsRenderer {
    settings: Settings,
}

/// Measured heights for a board on different page types
struct BoardHeights {
    practice: f32,
    answers: f32,
}

/// Blue color for page margin debug box
const DEBUG_MARGIN_COLOR: Rgb = Rgb {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    icc_profile: None,
};

impl BiddingSheetsRenderer {
    /// Draw a debug outline box
    fn draw_debug_box(&self, layer: &mut LayerBuilder, x: f32, y: f32, w: f32, h: f32) {
        if !self.settings.debug_boxes {
            return;
        }
        // y is top of box, draw from bottom-left to top-right
        layer.set_outline_color(Color::Rgb(DEBUG_BOX_COLOR));
        layer.set_outline_thickness(0.25);
        layer.add_rect(Mm(x), Mm(y - h), Mm(x + w), Mm(y), PaintMode::Stroke);
    }

    /// Draw the page margin boundary (content area)
    fn draw_margin_debug_box(&self, layer: &mut LayerBuilder) {
        if !self.settings.debug_boxes {
            return;
        }
        let margin_left = self.settings.margin_left;
        let margin_right = self.settings.margin_right;
        let margin_top = self.settings.margin_top;
        let margin_bottom = self.settings.margin_bottom;
        let page_width = self.settings.page_width;
        let page_height = self.settings.page_height;

        let x = margin_left;
        let y = margin_bottom;
        let w = page_width - margin_left - margin_right;
        let h = page_height - margin_top - margin_bottom;

        layer.set_outline_color(Color::Rgb(DEBUG_MARGIN_COLOR));
        layer.set_outline_thickness(0.5);
        layer.add_rect(Mm(x), Mm(y), Mm(x + w), Mm(y + h), PaintMode::Stroke);
    }

    /// Draw a horizontal separator line from margin to margin
    fn draw_separator_line(&self, layer: &mut LayerBuilder, y: f32, thickness: f32, color: Rgb) {
        let margin_left = self.settings.margin_left;
        let margin_right = self.settings.margin_right;
        let page_width = self.settings.page_width;
        let x_end = page_width - margin_right;

        layer.set_outline_color(Color::Rgb(color));
        layer.set_outline_thickness(thickness);
        layer.add_line(Mm(margin_left), Mm(y), Mm(x_end), Mm(y));
    }

    /// Render the banner with left text and optional right-aligned title
    #[allow(clippy::too_many_arguments)]
    fn render_banner(
        &self,
        layer: &mut LayerBuilder,
        left_text: &str,
        left_text_short: &str, // Shortened version without "(Practice Page)" etc
        title: Option<&str>,
        header_color: Rgb,
        font: BuiltinFont,
        measurer: &dyn TextMeasure,
    ) {
        let margin_left = self.settings.margin_left;
        let content_width = self.settings.content_width();
        let page_top = self.settings.page_height - self.settings.margin_top;
        let banner_padding = 3.0;

        // Draw filled rectangle banner
        layer.set_fill_color(Color::Rgb(header_color.clone()));
        layer.add_rect(
            Mm(margin_left),
            Mm(page_top - BANNER_HEIGHT),
            Mm(margin_left + content_width),
            Mm(page_top),
            PaintMode::Fill,
        );

        // Calculate text position
        let text_y = page_top - banner_padding - measurer.cap_height_mm(HEADER_FONT_SIZE);
        layer.set_fill_color(Color::Rgb(WHITE));

        // Draw left text
        layer.use_text_builtin(
            left_text,
            HEADER_FONT_SIZE,
            Mm(margin_left + banner_padding),
            Mm(text_y),
            font,
        );

        // Draw title on right if present
        if let Some(title) = title {
            let left_text_width = measurer.measure_text(left_text, HEADER_FONT_SIZE);
            let min_gap = measurer.measure_text("     ", HEADER_FONT_SIZE); // 5 char gap
            let available_for_title =
                content_width - left_text_width - min_gap - 2.0 * banner_padding;

            if available_for_title > 0.0 {
                let title_width = measurer.measure_text(title, HEADER_FONT_SIZE);

                let (final_title, final_width) = if title_width <= available_for_title {
                    // Full title fits
                    (title.to_string(), title_width)
                } else {
                    // Try with shorter left text
                    let short_left_width = measurer.measure_text(left_text_short, HEADER_FONT_SIZE);
                    let available_with_short =
                        content_width - short_left_width - min_gap - 2.0 * banner_padding;

                    if title_width <= available_with_short {
                        // Title fits with shorter left text - but we already drew full left text
                        // For simplicity, just truncate the title
                        self.truncate_title(title, available_for_title, measurer)
                    } else {
                        // Truncate title to fit
                        self.truncate_title(title, available_for_title, measurer)
                    }
                };

                if !final_title.is_empty() {
                    let title_x = margin_left + content_width - banner_padding - final_width;
                    layer.use_text_builtin(
                        &final_title,
                        HEADER_FONT_SIZE,
                        Mm(title_x),
                        Mm(text_y),
                        font,
                    );
                }
            }
        }
    }

    /// Truncate title to fit within available width, adding ellipsis
    fn truncate_title(
        &self,
        title: &str,
        available_width: f32,
        measurer: &dyn TextMeasure,
    ) -> (String, f32) {
        let ellipsis = "...";
        let ellipsis_width = measurer.measure_text(ellipsis, HEADER_FONT_SIZE);

        if available_width <= ellipsis_width {
            return (String::new(), 0.0);
        }

        let target_width = available_width - ellipsis_width;
        let mut truncated = String::new();
        let mut width = 0.0;

        for ch in title.chars() {
            let ch_str = ch.to_string();
            let ch_width = measurer.measure_text(&ch_str, HEADER_FONT_SIZE);
            if width + ch_width > target_width {
                break;
            }
            truncated.push(ch);
            width += ch_width;
        }

        if truncated.is_empty() {
            (String::new(), 0.0)
        } else {
            truncated.push_str(ellipsis);
            let final_width = measurer.measure_text(&truncated, HEADER_FONT_SIZE);
            (truncated, final_width)
        }
    }
}

impl BiddingSheetsRenderer {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    /// Generate a PDF with bidding practice sheets
    pub fn render(&self, boards: &[Board]) -> Result<Vec<u8>, RenderError> {
        let title = boards
            .first()
            .and_then(|b| b.event.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("Bidding Practice");

        let mut doc = PdfDocument::new(title);

        // Load fonts - printpdf 0.8 handles subsetting automatically
        let fonts = FontManager::new(&mut doc)?;

        let mut pages = Vec::new();

        // Measure actual board heights by doing a dry-run render
        let board_heights = self.measure_board_heights(boards, &fonts);

        // Group boards into sets that fit on a page using actual measured heights
        let board_sets = self.group_boards_with_heights(boards, &board_heights);

        for board_set in board_sets {
            // North practice page
            let mut layer = LayerBuilder::new();
            self.render_practice_page(&mut layer, board_set, Direction::North, &fonts);
            pages.push(PdfPage::new(
                Mm(self.settings.page_width),
                Mm(self.settings.page_height),
                layer.into_ops(),
            ));

            // Answers page (after North)
            let mut layer = LayerBuilder::new();
            self.render_answers_page(&mut layer, board_set, &fonts);
            pages.push(PdfPage::new(
                Mm(self.settings.page_width),
                Mm(self.settings.page_height),
                layer.into_ops(),
            ));

            // South practice page
            let mut layer = LayerBuilder::new();
            self.render_practice_page(&mut layer, board_set, Direction::South, &fonts);
            pages.push(PdfPage::new(
                Mm(self.settings.page_width),
                Mm(self.settings.page_height),
                layer.into_ops(),
            ));

            // Answers page (after South, for duplex printing)
            let mut layer = LayerBuilder::new();
            self.render_answers_page(&mut layer, board_set, &fonts);
            pages.push(PdfPage::new(
                Mm(self.settings.page_width),
                Mm(self.settings.page_height),
                layer.into_ops(),
            ));
        }

        doc.with_pages(pages);

        let mut warnings = Vec::new();
        let bytes = doc.save(&save_options(), &mut warnings);

        // Compress PDF streams to reduce file size
        let compressed = compress_pdf(bytes.clone()).unwrap_or(bytes);
        Ok(compressed)
    }

    /// Calculate available content height on a page (after banner and gaps)
    fn available_content_height(&self) -> f32 {
        let margin_top = self.settings.margin_top;
        let margin_bottom = self.settings.margin_bottom;
        let page_height = self.settings.page_height;

        page_height - margin_top - margin_bottom - BANNER_HEIGHT - AFTER_BANNER_GAP
    }

    /// Count the number of lines in the auction setup for practice pages
    fn count_auction_setup_lines(&self, board: &Board, player: Direction) -> usize {
        // 1 line for "who bids first" (0 if empty, e.g. when opponent opens)
        let who_first = self.who_bids_first(board, player);
        let who_first_lines = if who_first.is_empty() { 0 } else { 1 };

        // Count opposition bidding lines
        let opp_lines = self.format_opposition_bidding(board, player).len();

        who_first_lines + opp_lines
    }

    /// Measure actual board heights by doing a dry-run render of the auction tables
    fn measure_board_heights(&self, boards: &[Board], fonts: &FontManager) -> Vec<BoardHeights> {
        let line_height = ANSWERS_FONT_SIZE * LINE_HEIGHT_MULTIPLIER * 0.4;
        let practice_line_height = PRACTICE_FONT_SIZE * LINE_HEIGHT_MULTIPLIER * 0.4;

        let text_font = fonts.serif.regular;
        let bold_font = fonts.serif.bold;
        let symbol_font = fonts.symbol_font();
        let colors = SuitColors::new(self.settings.black_color, self.settings.red_color);

        boards
            .iter()
            .map(|board| {
                // Practice page columns:
                // - Context: 4 lines (board#, dealer, vul, HCP)
                // - Hand: 4 lines (4 suits)
                // - Auction setup: variable (who bids first + opponent actions)
                let context_lines: f32 = 4.0;
                let hand_lines: f32 = 4.0;
                // Measure for both North and South, take the max
                let north_setup_lines =
                    self.count_auction_setup_lines(board, Direction::North) as f32;
                let south_setup_lines =
                    self.count_auction_setup_lines(board, Direction::South) as f32;
                let setup_lines = north_setup_lines.max(south_setup_lines);

                let practice_height =
                    context_lines.max(hand_lines).max(setup_lines) * practice_line_height;

                // Answers page: measure actual auction height
                let (auction_height, _) = if let Some(ref auction) = board.auction {
                    // Do a dry-run render to get actual height
                    let mut dummy_layer = LayerBuilder::new();

                    self.render_auction_table(
                        &mut dummy_layer,
                        auction,
                        0.0, // x doesn't matter for height
                        0.0, // y doesn't matter for height calculation
                        ANSWERS_FONT_SIZE,
                        text_font,
                        bold_font,
                        symbol_font,
                        &colors,
                    )
                } else {
                    (line_height, line_height)
                };

                let answers_context_height = 6.0 * line_height;
                let answers_hand_height = 5.0 * line_height;
                let answers_height = answers_context_height
                    .max(answers_hand_height)
                    .max(auction_height);

                eprintln!(
                    "  measure board {}: practice={:.2} (setup_lines={:.0}), answers={:.2}",
                    board.number.unwrap_or(0),
                    practice_height,
                    setup_lines,
                    answers_height,
                );

                BoardHeights {
                    practice: practice_height,
                    answers: answers_height,
                }
            })
            .collect()
    }

    /// Group boards into sets that fit on a page using pre-measured heights
    fn group_boards_with_heights<'a>(
        &self,
        boards: &'a [Board],
        heights: &[BoardHeights],
    ) -> Vec<&'a [Board]> {
        let available_height = self.available_content_height();
        eprintln!("=== Page break calculations ===");
        eprintln!("Page height: {}", self.settings.page_height);
        eprintln!(
            "Margin top: {}, bottom: {}",
            self.settings.margin_top, self.settings.margin_bottom
        );
        eprintln!(
            "Banner height: {}, after banner gap: {}",
            BANNER_HEIGHT, AFTER_BANNER_GAP
        );
        eprintln!("Available content height: {}", available_height);
        eprintln!("ROW_GAP between boards: {}", ROW_GAP);
        eprintln!();

        let mut sets = Vec::new();
        let mut start = 0;

        while start < boards.len() {
            let mut current_height = 0.0;
            let mut end = start;
            eprintln!("--- Starting new page set at board index {} ---", start);

            // Add boards until we run out of space
            while end < boards.len() {
                // Use pre-measured heights
                let practice_height = heights[end].practice;
                let answers_height = heights[end].answers;

                // Use the maximum height (answers page typically needs more space)
                let board_height = practice_height.max(answers_height);

                // Add ROW_GAP between boards (not before the first one)
                let height_needed = if end == start {
                    board_height
                } else {
                    board_height + ROW_GAP
                };

                let would_be_height = current_height + height_needed;
                eprintln!(
                    "Board {}: practice_h={:.2}, answers_h={:.2}, board_h={:.2}, height_needed={:.2}, current={:.2}, would_be={:.2}, available={:.2}, fits={}",
                    boards[end].number.unwrap_or(0),
                    practice_height,
                    answers_height,
                    board_height,
                    height_needed,
                    current_height,
                    would_be_height,
                    available_height,
                    would_be_height <= available_height || end == start
                );

                if current_height + height_needed > available_height && end > start {
                    // This board won't fit, but we have at least one board
                    eprintln!(
                        "  -> Board {} does not fit, breaking page",
                        boards[end].number.unwrap_or(0)
                    );
                    break;
                }

                current_height += height_needed;
                end += 1;
            }

            // Ensure we make progress (at least one board per page)
            if end == start {
                end = start + 1;
            }

            eprintln!(
                "Page set contains boards {} to {} (total height: {:.2})",
                start,
                end - 1,
                current_height
            );
            eprintln!();
            sets.push(&boards[start..end]);
            start = end;
        }

        sets
    }

    /// Render a practice page (shows only one player's hand)
    fn render_practice_page(
        &self,
        layer: &mut LayerBuilder,
        boards: &[Board],
        player: Direction,
        fonts: &FontManager,
    ) {
        // Draw page margin boundary for debugging
        self.draw_margin_debug_box(layer);

        let margin_left = self.settings.margin_left;
        let margin_top = self.settings.margin_top;
        let page_top = self.settings.page_height - margin_top;
        let content_width = self.settings.content_width();
        let measurer = get_helvetica_measurer();
        let sans_bold_measurer = get_helvetica_bold_measurer();

        let text_font = fonts.serif.regular;
        let bold_font = fonts.serif.bold;
        let sans_bold_font = fonts.sans.bold;
        let symbol_font = fonts.symbol_font();
        let colors = SuitColors::new(self.settings.black_color, self.settings.red_color);

        // Color for player identification
        let header_color = match player {
            Direction::North => Rgb::new(0.12, 0.56, 1.0, None), // DodgerBlue
            Direction::South => Rgb::new(1.0, 0.65, 0.0, None),  // Orange
            _ => Rgb::new(0.5, 0.5, 0.5, None),
        };

        // Header banner with title
        // Use short version (without "Practice Page") when title is present to make room
        let title = self.settings.effective_title();
        let header_text = if title.is_some() {
            format!("{} hands", player)
        } else {
            format!("{} hands (Practice Page)", player)
        };
        self.render_banner(
            layer,
            &header_text,
            &header_text, // Same text since we already shortened it when title present
            title,
            header_color.clone(),
            sans_bold_font,
            sans_bold_measurer,
        );

        // Start content below banner
        let mut current_y = page_top - BANNER_HEIGHT - AFTER_BANNER_GAP;

        let line_height = PRACTICE_FONT_SIZE * LINE_HEIGHT_MULTIPLIER * 0.4;

        let board_count = boards.len();
        for (i, board) in boards.iter().enumerate() {
            let row_start_y = current_y;

            // Calculate actual row height based on content
            let context_height = 4.0 * line_height; // 4 lines: Board, Dealer, Vul, HCP
            let hand_height = 4.0 * line_height; // 4 suits
            let setup_lines = self.count_auction_setup_lines(board, player) as f32;
            let setup_height = setup_lines * line_height;
            let row_height = context_height.max(hand_height).max(setup_height);

            // Debug boxes for each column
            let cap_height = measurer.cap_height_mm(PRACTICE_FONT_SIZE);
            let descender = measurer.descender_mm(PRACTICE_FONT_SIZE);
            let box_top = current_y + cap_height;
            let box_height = cap_height + row_height - line_height + descender;
            let setup_col_width = content_width - CONTEXT_COLUMN_WIDTH - HAND_COLUMN_WIDTH;

            self.draw_debug_box(
                layer,
                margin_left,
                box_top,
                CONTEXT_COLUMN_WIDTH,
                box_height,
            );
            self.draw_debug_box(
                layer,
                margin_left + CONTEXT_COLUMN_WIDTH,
                box_top,
                HAND_COLUMN_WIDTH,
                box_height,
            );
            self.draw_debug_box(
                layer,
                margin_left + CONTEXT_COLUMN_WIDTH + HAND_COLUMN_WIDTH,
                box_top,
                setup_col_width,
                box_height,
            );

            // Column 1: Board context
            let col1_x = margin_left;
            self.render_board_context(
                layer,
                board,
                Some(player),
                col1_x,
                current_y,
                PRACTICE_FONT_SIZE,
                text_font,
                bold_font,
            );

            // Column 2: Player's hand (vertical layout)
            let col2_x = margin_left + CONTEXT_COLUMN_WIDTH;
            let hand = board.deal.hand(player);
            self.render_hand_vertical(
                layer,
                hand,
                col2_x,
                current_y,
                PRACTICE_FONT_SIZE,
                text_font,
                symbol_font,
                &colors,
            );

            // Column 3: Opposition bidding + who bids first
            let col3_x = margin_left + CONTEXT_COLUMN_WIDTH + HAND_COLUMN_WIDTH;
            self.render_auction_setup(
                layer,
                board,
                player,
                col3_x,
                current_y,
                PRACTICE_FONT_SIZE,
                text_font,
                symbol_font,
                &colors,
            );

            // Draw separator line in the middle of the gap (except after the last board)
            // The visual bottom of content is at box_top - box_height
            // The gap runs from visual bottom to the next board's baseline
            if i < board_count - 1 {
                let visual_bottom = box_top - box_height;
                let next_board_top = row_start_y - row_height - ROW_GAP + cap_height;
                let line_y = (visual_bottom + next_board_top) / 2.0;
                self.draw_separator_line(
                    layer,
                    line_y,
                    PRACTICE_SEPARATOR_THICKNESS,
                    header_color.clone(),
                );
            }

            current_y = row_start_y - row_height - ROW_GAP;
        }
    }

    /// Render an answers page (shows both hands + auction)
    fn render_answers_page(&self, layer: &mut LayerBuilder, boards: &[Board], fonts: &FontManager) {
        // Draw page margin boundary for debugging
        self.draw_margin_debug_box(layer);

        let margin_left = self.settings.margin_left;
        let margin_top = self.settings.margin_top;
        let page_top = self.settings.page_height - margin_top;
        let content_width = self.settings.content_width();
        let measurer = get_helvetica_measurer();
        let sans_bold_measurer = get_helvetica_bold_measurer();

        let text_font = fonts.serif.regular;
        let bold_font = fonts.serif.bold;
        let sans_bold_font = fonts.sans.bold;
        let symbol_font = fonts.symbol_font();
        let colors = SuitColors::new(self.settings.black_color, self.settings.red_color);

        // Header banner - dark gray for answers (no title on answers page since it's the back of practice page)
        let header_color = Rgb::new(0.3, 0.3, 0.3, None);
        self.render_banner(
            layer,
            "Both hands (Answers Page)",
            "Both hands",
            None, // No title on answers page
            header_color.clone(),
            sans_bold_font,
            sans_bold_measurer,
        );

        // Start content below banner
        let mut current_y = page_top - BANNER_HEIGHT - AFTER_BANNER_GAP;

        let line_height = ANSWERS_FONT_SIZE * LINE_HEIGHT_MULTIPLIER * 0.4;

        let board_count = boards.len();
        for (i, board) in boards.iter().enumerate() {
            let row_start_y = current_y;

            // Column 1: Board context (with both HCPs and contract)
            let col1_x = margin_left;
            self.render_board_context_full(
                layer,
                board,
                col1_x,
                current_y,
                ANSWERS_FONT_SIZE,
                text_font,
                bold_font,
                symbol_font,
                &colors,
            );

            // Column 2: North's hand
            let col2_x = margin_left + CONTEXT_COLUMN_WIDTH;
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(
                "North:",
                ANSWERS_FONT_SIZE,
                Mm(col2_x),
                Mm(current_y),
                bold_font,
            );
            self.render_hand_vertical(
                layer,
                &board.deal.north,
                col2_x,
                current_y - line_height,
                ANSWERS_FONT_SIZE,
                text_font,
                symbol_font,
                &colors,
            );

            // Column 3: South's hand
            let col3_x = margin_left + CONTEXT_COLUMN_WIDTH + HAND_COLUMN_WIDTH;
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(
                "South:",
                ANSWERS_FONT_SIZE,
                Mm(col3_x),
                Mm(current_y),
                bold_font,
            );
            self.render_hand_vertical(
                layer,
                &board.deal.south,
                col3_x,
                current_y - line_height,
                ANSWERS_FONT_SIZE,
                text_font,
                symbol_font,
                &colors,
            );

            // Column 4: Auction table - capture actual height and last line height
            let col4_x = margin_left + CONTEXT_COLUMN_WIDTH + 2.0 * HAND_COLUMN_WIDTH;
            let (auction_height, auction_last_line_height) =
                if let Some(ref auction) = board.auction {
                    self.render_auction_table(
                        layer,
                        auction,
                        col4_x,
                        current_y,
                        ANSWERS_FONT_SIZE,
                        text_font,
                        bold_font,
                        symbol_font,
                        &colors,
                    )
                } else {
                    (line_height, line_height)
                };

            // Calculate row height using actual auction height
            let context_height = 6.0 * line_height; // More lines on answers page
            let hand_height = 5.0 * line_height; // Label + 4 suits
            let row_height = context_height.max(hand_height).max(auction_height);

            // Determine which column is tallest and use its last line height
            let last_line_height =
                if auction_height >= context_height && auction_height >= hand_height {
                    auction_last_line_height
                } else {
                    line_height
                };

            // Draw debug boxes using actual heights
            let cap_height = measurer.cap_height_mm(ANSWERS_FONT_SIZE);
            let descender = measurer.descender_mm(ANSWERS_FONT_SIZE);
            let box_top = row_start_y + cap_height;
            // Box height from top of first line to bottom of descenders on last line
            let box_height = cap_height + row_height - last_line_height + descender;

            self.draw_debug_box(
                layer,
                margin_left,
                box_top,
                CONTEXT_COLUMN_WIDTH,
                box_height,
            );
            self.draw_debug_box(
                layer,
                margin_left + CONTEXT_COLUMN_WIDTH,
                box_top,
                HAND_COLUMN_WIDTH,
                box_height,
            );
            self.draw_debug_box(
                layer,
                margin_left + CONTEXT_COLUMN_WIDTH + HAND_COLUMN_WIDTH,
                box_top,
                HAND_COLUMN_WIDTH,
                box_height,
            );
            // Auction column extends to right margin
            let auction_col_width = content_width - CONTEXT_COLUMN_WIDTH - 2.0 * HAND_COLUMN_WIDTH;
            self.draw_debug_box(
                layer,
                margin_left + CONTEXT_COLUMN_WIDTH + 2.0 * HAND_COLUMN_WIDTH,
                box_top,
                auction_col_width,
                box_height,
            );

            // Draw thin separator line overlaid on center of gap (except after last board)
            // The visual bottom of content is at box_top - box_height
            // The gap runs from visual bottom to the next board's baseline
            if i < board_count - 1 {
                let visual_bottom = box_top - box_height;
                let next_board_top = row_start_y - row_height - ROW_GAP + cap_height;
                let line_y = (visual_bottom + next_board_top) / 2.0;
                self.draw_separator_line(
                    layer,
                    line_y,
                    ANSWERS_SEPARATOR_THICKNESS,
                    header_color.clone(),
                );
            }

            current_y = row_start_y - row_height - ROW_GAP;
        }
    }

    /// Render board context (board number, dealer, vulnerability, HCP)
    #[allow(clippy::too_many_arguments)]
    fn render_board_context(
        &self,
        layer: &mut LayerBuilder,
        board: &Board,
        player: Option<Direction>,
        x: f32,
        y: f32,
        font_size: f32,
        text_font: BuiltinFont,
        bold_font: BuiltinFont,
    ) {
        let line_height = font_size * LINE_HEIGHT_MULTIPLIER * 0.4;
        let mut current_y = y;

        layer.set_fill_color(Color::Rgb(BLACK));

        // Board number
        if let Some(num) = board.number {
            layer.use_text_builtin(
                format!("Board: {}", num),
                font_size,
                Mm(x),
                Mm(current_y),
                bold_font,
            );
            current_y -= line_height;
        }

        // Dealer
        if let Some(dealer) = board.dealer {
            layer.use_text_builtin(
                format!("Dealer: {}", dealer),
                font_size,
                Mm(x),
                Mm(current_y),
                text_font,
            );
            current_y -= line_height;
        }

        // Vulnerability
        let vul_str = match board.vulnerable {
            Vulnerability::None => "None",
            Vulnerability::NorthSouth => "N-S",
            Vulnerability::EastWest => "E-W",
            Vulnerability::Both => "Both",
        };
        layer.use_text_builtin(
            format!("Vul: {}", vul_str),
            font_size,
            Mm(x),
            Mm(current_y),
            text_font,
        );
        current_y -= line_height;

        // HCP (for single player) with length points
        if let Some(player) = player {
            let hand = board.deal.hand(player);
            let hcp = hand.total_hcp();
            let length_pts = hand.length_points();
            let hcp_str = if length_pts > 0 {
                format!("HCP: {}+{}", hcp, length_pts)
            } else {
                format!("HCP: {}", hcp)
            };
            layer.use_text_builtin(hcp_str, font_size, Mm(x), Mm(current_y), text_font);
        }
    }

    /// Render full board context for answers page (includes both HCPs and contract)
    #[allow(clippy::too_many_arguments)]
    fn render_board_context_full(
        &self,
        layer: &mut LayerBuilder,
        board: &Board,
        x: f32,
        y: f32,
        font_size: f32,
        text_font: BuiltinFont,
        bold_font: BuiltinFont,
        symbol_font: &FontId,
        colors: &SuitColors,
    ) {
        let line_height = font_size * LINE_HEIGHT_MULTIPLIER * 0.4;
        let mut current_y = y;
        let measurer = get_helvetica_measurer();

        layer.set_fill_color(Color::Rgb(BLACK));

        // Board number
        if let Some(num) = board.number {
            layer.use_text_builtin(
                format!("Board: {}", num),
                font_size,
                Mm(x),
                Mm(current_y),
                bold_font,
            );
            current_y -= line_height;
        }

        // Dealer
        if let Some(dealer) = board.dealer {
            layer.use_text_builtin(
                format!("Dealer: {}", dealer),
                font_size,
                Mm(x),
                Mm(current_y),
                text_font,
            );
            current_y -= line_height;
        }

        // Vulnerability
        let vul_str = match board.vulnerable {
            Vulnerability::None => "None",
            Vulnerability::NorthSouth => "N-S",
            Vulnerability::EastWest => "E-W",
            Vulnerability::Both => "Both",
        };
        layer.use_text_builtin(
            format!("Vul: {}", vul_str),
            font_size,
            Mm(x),
            Mm(current_y),
            text_font,
        );
        current_y -= line_height;

        // North HCP with length points
        let north_hcp = board.deal.north.total_hcp();
        let north_length = board.deal.north.length_points();
        let north_hcp_str = if north_length > 0 {
            format!("North HCP: {}+{}", north_hcp, north_length)
        } else {
            format!("North HCP: {}", north_hcp)
        };
        layer.use_text_builtin(north_hcp_str, font_size, Mm(x), Mm(current_y), text_font);
        current_y -= line_height;

        // South HCP with length points
        let south_hcp = board.deal.south.total_hcp();
        let south_length = board.deal.south.length_points();
        let south_hcp_str = if south_length > 0 {
            format!("South HCP: {}+{}", south_hcp, south_length)
        } else {
            format!("South HCP: {}", south_hcp)
        };
        layer.use_text_builtin(south_hcp_str, font_size, Mm(x), Mm(current_y), text_font);
        current_y -= line_height;

        // Contract (if available)
        if let Some(ref auction) = board.auction {
            if let Some(contract) = auction.final_contract() {
                let prefix = "Contract: ";
                layer.use_text_builtin(prefix, font_size, Mm(x), Mm(current_y), text_font);

                let prefix_width = measurer.measure_width_mm(prefix, font_size);
                let mut contract_x = x + prefix_width;

                // Level
                let level_str = contract.level.to_string();
                layer.use_text_builtin(
                    &level_str,
                    font_size,
                    Mm(contract_x),
                    Mm(current_y),
                    text_font,
                );
                contract_x += measurer.measure_width_mm(&level_str, font_size);

                // Suit symbol
                let symbol = match contract.strain {
                    BidSuit::Clubs => "\u{2663}",
                    BidSuit::Diamonds => "\u{2666}",
                    BidSuit::Hearts => "\u{2665}",
                    BidSuit::Spades => "\u{2660}",
                    BidSuit::NoTrump => "NT",
                };

                if contract.strain.is_red() {
                    layer.set_fill_color(Color::Rgb(colors.hearts.clone()));
                } else {
                    layer.set_fill_color(Color::Rgb(BLACK));
                }

                if contract.strain == BidSuit::NoTrump {
                    layer.use_text_builtin(
                        symbol,
                        font_size,
                        Mm(contract_x),
                        Mm(current_y),
                        text_font,
                    );
                } else {
                    layer.use_text(
                        symbol,
                        font_size,
                        Mm(contract_x),
                        Mm(current_y),
                        symbol_font,
                    );
                }
                contract_x += measurer.measure_width_mm(symbol, font_size);

                layer.set_fill_color(Color::Rgb(BLACK));

                // Doubled/Redoubled
                if contract.redoubled {
                    layer.use_text_builtin(
                        "XX",
                        font_size,
                        Mm(contract_x),
                        Mm(current_y),
                        text_font,
                    );
                    contract_x += measurer.measure_width_mm("XX", font_size);
                } else if contract.doubled {
                    layer.use_text_builtin(
                        "X",
                        font_size,
                        Mm(contract_x),
                        Mm(current_y),
                        text_font,
                    );
                    contract_x += measurer.measure_width_mm("X", font_size);
                }

                // Declarer
                let declarer_str = format!(" {}", contract.declarer);
                layer.use_text_builtin(
                    &declarer_str,
                    font_size,
                    Mm(contract_x),
                    Mm(current_y),
                    text_font,
                );
            }
        }
    }

    /// Render a hand in vertical layout (suit per line)
    #[allow(clippy::too_many_arguments)]
    fn render_hand_vertical(
        &self,
        layer: &mut LayerBuilder,
        hand: &Hand,
        x: f32,
        y: f32,
        font_size: f32,
        text_font: BuiltinFont,
        symbol_font: &FontId,
        colors: &SuitColors,
    ) {
        let line_height = font_size * LINE_HEIGHT_MULTIPLIER * 0.4;
        let measurer = get_helvetica_measurer();
        let mut current_y = y;

        let suits = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];

        for suit in suits {
            let holding = hand.holding(suit);

            // Suit symbol
            let symbol = suit.symbol().to_string();
            let suit_color = colors.for_suit(
                &crate::model::Card {
                    suit,
                    rank: crate::model::Rank::Ace,
                }
                .suit,
            );
            layer.set_fill_color(Color::Rgb(suit_color));
            layer.use_text(&symbol, font_size, Mm(x), Mm(current_y), symbol_font);

            let symbol_width = measurer.measure_width_mm(&symbol, font_size);

            // Holding (or void dash)
            layer.set_fill_color(Color::Rgb(BLACK));
            let holding_str = if holding.is_void() {
                "\u{2014}".to_string() // Em dash for void
            } else {
                holding.to_string()
            };
            layer.use_text_builtin(
                &holding_str,
                font_size,
                Mm(x + symbol_width + 1.0),
                Mm(current_y),
                text_font,
            );

            current_y -= line_height;
        }
    }

    /// Render auction setup (who bids first + opposition bidding)
    #[allow(clippy::too_many_arguments)]
    fn render_auction_setup(
        &self,
        layer: &mut LayerBuilder,
        board: &Board,
        player: Direction,
        x: f32,
        y: f32,
        font_size: f32,
        text_font: BuiltinFont,
        symbol_font: &FontId,
        colors: &SuitColors,
    ) {
        let line_height = font_size * LINE_HEIGHT_MULTIPLIER * 0.4;
        let mut current_y = y;

        // Who bids first (at the top) - may be empty when opponent opens
        let who_first = self.who_bids_first(board, player);
        if !who_first.is_empty() {
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(&who_first, font_size, Mm(x), Mm(current_y), text_font);
            current_y -= line_height;
        }

        // Opposition bidding
        let opp_lines = self.format_opposition_bidding(board, player);
        for line in &opp_lines {
            self.render_mixed_text(
                layer,
                line,
                x,
                current_y,
                font_size,
                text_font,
                symbol_font,
                colors,
            );
            current_y -= line_height;
        }
    }

    /// Format opposition bidding as text lines with LHO/RHO labels
    fn format_opposition_bidding(&self, board: &Board, player: Direction) -> Vec<MixedText> {
        let mut lines = Vec::new();

        let Some(ref auction) = board.auction else {
            lines.push(MixedText::plain("The opponents are silent."));
            return lines;
        };

        // Determine LHO and RHO relative to player
        let lho = player.next();
        let rho = player.next().next().next(); // 3 steps around = RHO

        let mut current_seat = auction.dealer;
        let mut is_opening_bid = true;
        let mut prev_bid: Option<(u8, BidSuit)> = None;
        let mut found_opp_bid = false;

        for annotated in &auction.calls {
            let is_opponent = current_seat == lho || current_seat == rho;

            if is_opponent {
                // Determine position label (LHO or RHO)
                let position = if current_seat == lho { "LHO" } else { "RHO" };

                match &annotated.call {
                    Call::Bid {
                        level,
                        strain: suit,
                    } => {
                        if is_opening_bid {
                            // Opponent is dealer and opens - state as fact
                            lines.push(MixedText::opening_bid(position, *level, *suit));
                            let who_next = if current_seat == rho {
                                "You bid next."
                            } else {
                                "Partner bids next."
                            };
                            lines.push(MixedText::plain(who_next));
                        } else {
                            lines.push(MixedText::bid_action_with_position(
                                current_seat,
                                position,
                                "bids",
                                *level,
                                *suit,
                            ));
                        }
                        is_opening_bid = false;
                        prev_bid = Some((*level, *suit));
                        found_opp_bid = true;
                    }
                    Call::Double => {
                        if let Some((level, suit)) = prev_bid {
                            lines.push(MixedText::double_action_with_position(
                                current_seat,
                                position,
                                level,
                                suit,
                            ));
                        }
                        found_opp_bid = true;
                    }
                    Call::Redouble => {
                        lines.push(MixedText::plain(&format!(
                            "{} ({}) Redoubles if possible.",
                            current_seat, position
                        )));
                        found_opp_bid = true;
                    }
                    Call::Pass | Call::Continue | Call::Blank => {}
                }
            } else {
                // Track N/S bids for "doubles X" context
                if let Call::Bid {
                    level,
                    strain: suit,
                } = &annotated.call
                {
                    prev_bid = Some((*level, *suit));
                    is_opening_bid = false;
                }
            }

            current_seat = current_seat.next();
        }

        if !found_opp_bid {
            lines.push(MixedText::plain("The opponents are silent."));
        }

        lines
    }

    /// Determine who bids first for a given player
    fn who_bids_first(&self, board: &Board, player: Direction) -> String {
        let Some(dealer) = board.dealer else {
            return String::new();
        };

        let partner = player.partner();
        let lho = player.next();
        let rho = player.next().next().next(); // 3 steps = RHO

        // Check if opponents are silent (only pass)
        let opponents_silent = board
            .auction
            .as_ref()
            .map(|auction| {
                let mut seat = auction.dealer;
                auction.calls.iter().all(|a| {
                    let is_opp = seat == lho || seat == rho;
                    let ok = !is_opp || matches!(a.call, Call::Pass);
                    seat = seat.next();
                    ok
                })
            })
            .unwrap_or(true);

        if dealer == player {
            "You bid first.".to_string()
        } else if dealer == partner {
            "Partner bids first.".to_string()
        } else if opponents_silent {
            // When opponents deal but are silent, just say who bids first
            if dealer == rho {
                "You bid first.".to_string()
            } else {
                // LHO is dealer; after LHO passes, partner bids first
                "Partner bids first.".to_string()
            }
        } else {
            // Opponent opens - format_opposition_bidding handles the details
            String::new()
        }
    }

    /// Render mixed text (text with embedded suit symbols)
    #[allow(clippy::too_many_arguments)]
    fn render_mixed_text(
        &self,
        layer: &mut LayerBuilder,
        text: &MixedText,
        x: f32,
        y: f32,
        font_size: f32,
        text_font: BuiltinFont,
        symbol_font: &FontId,
        colors: &SuitColors,
    ) {
        // Use Times measurer for text (matches text_font which is serif)
        // and Helvetica measurer for symbols
        let text_measurer = get_times_measurer();
        let symbol_measurer = get_helvetica_measurer();
        let mut current_x = x;

        for segment in &text.segments {
            match segment {
                TextSegment::Plain(s) => {
                    layer.set_fill_color(Color::Rgb(BLACK));
                    layer.use_text_builtin(s, font_size, Mm(current_x), Mm(y), text_font);
                    current_x += text_measurer.measure_width_mm(s, font_size);
                }
                TextSegment::Suit(suit) => {
                    let suit_color = colors.for_bid_suit(suit);
                    layer.set_fill_color(Color::Rgb(suit_color));
                    let symbol = suit.symbol();
                    // NoTrump returns "NT" which is regular text, not a symbol glyph
                    if *suit == BidSuit::NoTrump {
                        layer.use_text_builtin(symbol, font_size, Mm(current_x), Mm(y), text_font);
                        current_x += text_measurer.measure_width_mm(symbol, font_size);
                    } else {
                        layer.use_text(symbol, font_size, Mm(current_x), Mm(y), symbol_font);
                        current_x += symbol_measurer.measure_width_mm(symbol, font_size);
                    }
                }
            }
        }
    }

    /// Render auction table in W/N/E/S columns
    /// Returns (total_height, last_line_height) so caller can correctly compute box bounds
    #[allow(clippy::too_many_arguments)]
    fn render_auction_table(
        &self,
        layer: &mut LayerBuilder,
        auction: &Auction,
        x: f32,
        y: f32,
        font_size: f32,
        text_font: BuiltinFont,
        bold_font: BuiltinFont,
        symbol_font: &FontId,
        colors: &SuitColors,
    ) -> (f32, f32) {
        let line_height = font_size * LINE_HEIGHT_MULTIPLIER * 0.4;
        let col_width = 12.0; // Column width for each seat
        let mut current_y = y;

        // Header row
        layer.set_fill_color(Color::Rgb(BLACK));
        layer.use_text_builtin("W", font_size, Mm(x), Mm(current_y), bold_font);
        layer.use_text_builtin("N", font_size, Mm(x + col_width), Mm(current_y), bold_font);
        layer.use_text_builtin(
            "E",
            font_size,
            Mm(x + 2.0 * col_width),
            Mm(current_y),
            bold_font,
        );
        layer.use_text_builtin(
            "S",
            font_size,
            Mm(x + 3.0 * col_width),
            Mm(current_y),
            bold_font,
        );
        current_y -= line_height;

        // Determine starting column based on dealer
        let start_col = auction.dealer.table_position();

        // Render calls
        let mut col = start_col;
        let mut row_y = current_y;

        // Leave positions before dealer blank (no dashes)

        // Check for special cases: passed out (4 passes) or ending with "All Pass" (3+ passes)
        let all_passes = auction.calls.iter().all(|c| matches!(c.call, Call::Pass));
        let is_passed_out = all_passes && auction.calls.len() == 4;

        // Count trailing passes to detect "All Pass" ending
        let trailing_passes = auction
            .calls
            .iter()
            .rev()
            .take_while(|c| matches!(c.call, Call::Pass))
            .count();
        let has_all_pass_ending = trailing_passes >= 3 && !is_passed_out;

        // Calculate how many calls to render normally (excluding the 3 trailing passes if All Pass)
        let calls_to_render = if has_all_pass_ending {
            auction.calls.len() - 3
        } else if is_passed_out {
            0 // We'll render "Pass Out" specially
        } else {
            auction.calls.len()
        };

        // Render normal calls
        for annotated in auction.calls.iter().take(calls_to_render) {
            let col_x = x + col as f32 * col_width;

            self.render_annotated_call(
                layer,
                annotated,
                col_x,
                row_y,
                font_size,
                text_font,
                symbol_font,
                colors,
            );

            col += 1;
            if col >= 4 {
                col = 0;
                row_y -= line_height;
            }
        }

        // Handle special endings
        if is_passed_out {
            // Four passes: show "Pass Out" in dealer's column
            let col_x = x + start_col as f32 * col_width;
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin("Pass Out", font_size, Mm(col_x), Mm(row_y), text_font);
            col = start_col + 1;
            if col >= 4 {
                col = 0;
                row_y -= line_height;
            }
        } else if has_all_pass_ending {
            // Show "All Pass" in the position of the first of the three passes
            let col_x = x + col as f32 * col_width;
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin("All Pass", font_size, Mm(col_x), Mm(row_y), text_font);
            col += 1;
            if col >= 4 {
                col = 0;
                row_y -= line_height;
            }
            // Leave the remaining two pass positions blank
        }

        // Track the last line height used (for correct box calculation)
        let mut last_line_height = line_height;

        // Render notes if present
        if !auction.notes.is_empty() {
            // Move to a new row after the auction
            if col > 0 {
                row_y -= line_height;
            }
            row_y -= line_height * 0.5; // Gap before notes

            let note_font_size = font_size * 0.90;
            let note_line_height = note_font_size * LINE_HEIGHT_MULTIPLIER * 0.4;

            // Get sorted note numbers
            let mut note_nums: Vec<&u8> = auction.notes.keys().collect();
            note_nums.sort();

            for num in note_nums {
                if let Some(text) = auction.notes.get(num) {
                    let note_text = format!("{}. {}", num, text);
                    layer.set_fill_color(Color::Rgb(BLACK));
                    layer.use_text_builtin(&note_text, note_font_size, Mm(x), Mm(row_y), text_font);
                    row_y -= note_line_height;
                }
            }
            // Notes use smaller line height
            last_line_height = note_line_height;
        }

        // Return (total height, last line height used)
        (y - row_y, last_line_height)
    }

    /// Render an annotated call (call with optional superscript annotation)
    #[allow(clippy::too_many_arguments)]
    fn render_annotated_call(
        &self,
        layer: &mut LayerBuilder,
        annotated: &AnnotatedCall,
        x: f32,
        y: f32,
        font_size: f32,
        text_font: BuiltinFont,
        symbol_font: &FontId,
        colors: &SuitColors,
    ) {
        let call_width = self.render_call(
            layer,
            &annotated.call,
            x,
            y,
            font_size,
            text_font,
            symbol_font,
            colors,
        );

        // If there's an annotation, render it as superscript
        if let Some(ref annotation) = annotated.annotation {
            let sup_x = x + call_width;
            let sup_y = y + (font_size * SUPERSCRIPT_RISE * 0.352778); // Convert pt to mm
            let sup_size = font_size * SUPERSCRIPT_RATIO;

            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(annotation, sup_size, Mm(sup_x), Mm(sup_y), text_font);
        }
    }

    /// Render a single call and return the width used
    #[allow(clippy::too_many_arguments)]
    fn render_call(
        &self,
        layer: &mut LayerBuilder,
        call: &Call,
        x: f32,
        y: f32,
        font_size: f32,
        text_font: BuiltinFont,
        symbol_font: &FontId,
        colors: &SuitColors,
    ) -> f32 {
        let measurer = get_helvetica_measurer();

        match call {
            Call::Pass => {
                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin("Pass", font_size, Mm(x), Mm(y), text_font);
                measurer.measure_width_mm("Pass", font_size)
            }
            Call::Double => {
                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin("X", font_size, Mm(x), Mm(y), text_font);
                measurer.measure_width_mm("X", font_size)
            }
            Call::Redouble => {
                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin("XX", font_size, Mm(x), Mm(y), text_font);
                measurer.measure_width_mm("XX", font_size)
            }
            Call::Bid {
                level,
                strain: suit,
            } => {
                // Level
                let level_str = level.to_string();
                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin(&level_str, font_size, Mm(x), Mm(y), text_font);

                let level_width = measurer.measure_width_mm(&level_str, font_size);

                // Suit
                let symbol = match suit {
                    BidSuit::Clubs => "\u{2663}",
                    BidSuit::Diamonds => "\u{2666}",
                    BidSuit::Hearts => "\u{2665}",
                    BidSuit::Spades => "\u{2660}",
                    BidSuit::NoTrump => "NT",
                };

                if suit.is_red() {
                    layer.set_fill_color(Color::Rgb(colors.hearts.clone()));
                } else {
                    layer.set_fill_color(Color::Rgb(BLACK));
                }

                if *suit == BidSuit::NoTrump {
                    layer.use_text_builtin(
                        symbol,
                        font_size,
                        Mm(x + level_width),
                        Mm(y),
                        text_font,
                    );
                } else {
                    layer.use_text(symbol, font_size, Mm(x + level_width), Mm(y), symbol_font);
                }

                let symbol_width = measurer.measure_width_mm(symbol, font_size);
                level_width + symbol_width
            }
            Call::Continue => {
                // "+" in PBN becomes "?" in display
                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin("?", font_size, Mm(x), Mm(y), text_font);
                measurer.measure_width_mm("?", font_size)
            }
            Call::Blank => {
                // Underscore sequences become a horizontal line for fill-in exercises
                let line_width = 8.0; // mm
                let line_thickness = 0.3; // mm
                let baseline_offset = font_size * 0.08 * 0.352778; // Slightly below baseline

                layer.set_outline_color(Color::Rgb(BLACK));
                layer.set_outline_thickness(line_thickness);
                layer.add_line(
                    Mm(x),
                    Mm(y - baseline_offset),
                    Mm(x + line_width),
                    Mm(y - baseline_offset),
                );
                line_width
            }
        }
    }
}

/// Mixed text with plain text and suit symbols
struct MixedText {
    segments: Vec<TextSegment>,
}

enum TextSegment {
    Plain(String),
    Suit(BidSuit),
}

impl MixedText {
    fn plain(s: &str) -> Self {
        Self {
            segments: vec![TextSegment::Plain(s.to_string())],
        }
    }

    /// Format: "RHO opens the bidding 1♦."
    fn opening_bid(position: &str, level: u8, suit: BidSuit) -> Self {
        Self {
            segments: vec![
                TextSegment::Plain(format!("{} opens the bidding ", position)),
                TextSegment::Plain(format!("{}", level)),
                TextSegment::Suit(suit),
                TextSegment::Plain(".".to_string()),
            ],
        }
    }

    /// Format: "East (LHO) bids 1♠ if possible."
    fn bid_action_with_position(
        seat: Direction,
        position: &str,
        action: &str,
        level: u8,
        suit: BidSuit,
    ) -> Self {
        Self {
            segments: vec![
                TextSegment::Plain(format!("{} ({}) {} ", seat, position, action)),
                TextSegment::Plain(format!("{}", level)),
                TextSegment::Suit(suit),
                TextSegment::Plain(" if possible.".to_string()),
            ],
        }
    }

    /// Format: "East (LHO) Doubles 1♠ if possible."
    fn double_action_with_position(
        seat: Direction,
        position: &str,
        level: u8,
        suit: BidSuit,
    ) -> Self {
        Self {
            segments: vec![
                TextSegment::Plain(format!("{} ({}) Doubles ", seat, position)),
                TextSegment::Plain(format!("{}", level)),
                TextSegment::Suit(suit),
                TextSegment::Plain(" if possible.".to_string()),
            ],
        }
    }
}

/// Extension trait for SuitColors to handle BidSuit
trait SuitColorsExt {
    fn for_bid_suit(&self, suit: &BidSuit) -> Rgb;
}

impl SuitColorsExt for SuitColors {
    fn for_bid_suit(&self, suit: &BidSuit) -> Rgb {
        match suit {
            BidSuit::Hearts | BidSuit::Diamonds => self.hearts.clone(),
            BidSuit::Spades | BidSuit::Clubs | BidSuit::NoTrump => self.spades.clone(),
        }
    }
}
