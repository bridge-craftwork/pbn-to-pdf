use crate::config::Settings;
use crate::error::RenderError;
use crate::model::card::RankExt;
use crate::model::{AuctionExt, BCFlags, BidSuit, Board, CommentaryBlock, CommentarySlot};
use printpdf::{BuiltinFont, Color, FontId, Mm, PaintMode, PdfPage, Rgb};

use crate::render::components::bidding_table::BiddingTableRenderer;
use crate::render::components::commentary::{CommentaryRenderer, FloatLayout};
use crate::render::components::hand_diagram::{DiagramDisplayOptions, HandDiagramRenderer};
use crate::render::components::page_furniture::PageFurniture;
use crate::render::helpers::colors::{SuitColors, BLACK};
use crate::render::helpers::compress::compress_pdf;
use crate::render::helpers::document::new_document;
use crate::render::helpers::fonts::FontManager;
use crate::render::helpers::layer::{save_options, LayerBuilder};
use crate::render::helpers::text_metrics::{self, get_times_measurer};

/// Light gray color for debug boxes (component level)
const DEBUG_BOX_COLOR: Rgb = Rgb {
    r: 0.7,
    g: 0.7,
    b: 0.7,
    icc_profile: None,
};

/// Orange color for board-level debug boxes
const DEBUG_BOARD_BOX_COLOR: Rgb = Rgb {
    r: 1.0,
    g: 0.5,
    b: 0.0,
    icc_profile: None,
};
// Debug boxes are now controlled via settings.debug_boxes

/// Dark gray color for separator lines
const SEPARATOR_COLOR: Rgb = Rgb {
    r: 0.4,
    g: 0.4,
    b: 0.4,
    icc_profile: None,
};

/// Separator line thickness
const SEPARATOR_THICKNESS: f32 = 0.5;

/// Special board name that triggers a column break
const COLUMN_BREAK_NAME: &str = "column-break";
/// Special board name that triggers a page break
const PAGE_BREAK_NAME: &str = "page-break";
/// Legacy spacer name (treated as column-break)
const SPACER_NAME: &str = "spacer";

/// Check if a board is a column break marker
fn is_column_break(board: &Board) -> bool {
    // Check BCFlags bit 25
    if board.bc_flags.as_ref().is_some_and(|f| f.column_break()) {
        return true;
    }
    // Check board name
    board
        .board_id
        .as_ref()
        .map(|id| {
            let id_lower = id.to_lowercase();
            id_lower == COLUMN_BREAK_NAME || id_lower == SPACER_NAME
        })
        .unwrap_or(false)
}

/// Check if a board is a page break marker
fn is_page_break(board: &Board) -> bool {
    // Check BCFlags bit 26
    if board.bc_flags.as_ref().is_some_and(|f| f.page_break()) {
        return true;
    }
    // Check board name
    board
        .board_id
        .as_ref()
        .map(|id| id.to_lowercase() == PAGE_BREAK_NAME)
        .unwrap_or(false)
}

/// Visibility flags for a board, computed once and reused
struct BoardVisibility {
    show_board: bool,
    show_dealer: bool,
    show_vulnerable: bool,
    show_diagram: bool,
    show_auction: bool,
    show_commentary: bool,
}

impl BoardVisibility {
    fn from_board(board: &Board, settings: &Settings) -> Self {
        let flags = board.bc_flags;
        let deal_is_empty = board.deal.is_empty();
        let has_auction = board
            .auction
            .as_ref()
            .map(|a| !a.calls.is_empty())
            .unwrap_or(false);
        // Show board info if deal has cards OR there's an auction (for exercise boards)
        let has_content = !deal_is_empty || has_auction;
        // Board label is tied to diagram - if BCFlags says no diagram, no board label either
        let show_diagram_flag = flags.map(|f| f.show_diagram()).unwrap_or(true);
        Self {
            show_board: has_content
                && settings.show_board_labels
                && flags.map(|f| !f.hide_board()).unwrap_or(true)
                && show_diagram_flag,
            show_dealer: has_content
                && settings.show_board_labels
                && flags.map(|f| !f.hide_dealer()).unwrap_or(true)
                && show_diagram_flag,
            show_vulnerable: has_content
                && settings.show_board_labels
                && flags.map(|f| !f.hide_vulnerable()).unwrap_or(true)
                && show_diagram_flag,
            show_diagram: !deal_is_empty && show_diagram_flag && !board.hidden.all_hidden(),
            show_auction: has_auction
                && flags.map(|f| f.show_auction()).unwrap_or(true)
                && settings.show_bidding,
            show_commentary: ColumnCommentary::of(board, settings).any(),
        }
    }

    fn has_content(&self) -> bool {
        self.show_board
            || self.show_dealer
            || self.show_vulnerable
            || self.show_diagram
            || self.show_auction
            || self.show_commentary
    }
}

/// A board's visible commentary, split by where a column draws it.
///
/// Outside Center mode every block goes below the board, as it always has.
/// Center mode follows Bridge Composer (issue #24): each block shows under its
/// own BCFlags bit and in its own place -- the event commentary above the
/// board, the diagram commentary between diagram and auction, the final
/// commentary last -- and a block between `[Board]` and `[Deal]` not at all.
#[derive(Default)]
struct ColumnCommentary<'a> {
    above: Vec<&'a CommentaryBlock>,
    under_diagram: Vec<&'a CommentaryBlock>,
    below: Vec<&'a CommentaryBlock>,
}

impl<'a> ColumnCommentary<'a> {
    fn of(board: &'a Board, settings: &Settings) -> Self {
        let mut shown = Self::default();
        if !settings.show_commentary {
            return shown;
        }
        let flags = board.bc_flags;
        let blocks = board.commentary.iter().filter(|c| !c.is_blank());
        if !settings.center {
            if flags
                .map(|f| f.show_event_commentary() || f.show_final_commentary())
                .unwrap_or(true)
            {
                shown.below = blocks.collect();
            }
            return shown;
        }
        let allowed = |bit: fn(&BCFlags) -> bool| flags.map(|f| bit(&f)).unwrap_or(true);
        for block in blocks {
            match block.slot {
                CommentarySlot::Event if allowed(BCFlags::show_event_commentary) => {
                    shown.above.push(block)
                }
                CommentarySlot::Diagram if allowed(BCFlags::show_diagram_commentary) => {
                    shown.under_diagram.push(block)
                }
                CommentarySlot::Final if allowed(BCFlags::show_final_commentary) => {
                    shown.below.push(block)
                }
                _ => {}
            }
        }
        shown
    }

    fn any(&self) -> bool {
        !(self.above.is_empty() && self.under_diagram.is_empty() && self.below.is_empty())
    }
}

/// A board's `[Event]`, when it has one worth printing
fn board_event(board: &Board) -> Option<&str> {
    board.event.as_deref().filter(|e| !e.trim().is_empty())
}

/// Main document renderer
pub struct DocumentRenderer {
    settings: Settings,
}

impl DocumentRenderer {
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    /// Measure the height a board would use in a column without rendering
    /// Returns 0.0 for break markers and boards with no content
    fn measure_board_height(&self, board: &Board, column_width: f32) -> f32 {
        // Name-based break markers have zero height (no content)
        // BCFlags-based breaks have real content to measure
        let is_name_break = board
            .board_id
            .as_ref()
            .map(|id| {
                let id_lower = id.to_lowercase();
                id_lower == COLUMN_BREAK_NAME
                    || id_lower == SPACER_NAME
                    || id_lower == PAGE_BREAK_NAME
            })
            .unwrap_or(false);
        if is_name_break {
            return 0.0;
        }

        let visibility = BoardVisibility::from_board(board, &self.settings);
        let commentary = ColumnCommentary::of(board, &self.settings);

        // Empty boards have zero height
        if !visibility.has_content() {
            return 0.0;
        }

        let line_height = self.settings.line_height;
        let measurer = get_times_measurer();
        let cap_height = measurer.cap_height_mm(self.settings.body_font_size);

        // Count title lines (board number, dealer, vulnerability stacked vertically)
        let mut title_lines = 0;
        if visibility.show_board && board.board_id.is_some() {
            title_lines += 1;
        }
        if visibility.show_dealer && board.dealer.is_some() {
            title_lines += 1;
        }
        if visibility.show_vulnerable {
            title_lines += 1;
        }

        // For auction-only boards (no diagram), render board number inline with auction header
        // This saves vertical space by not having the board number on its own line
        // Not in Center mode: Bridge Composer puts the label on a line of its
        // own, top left, and an inline one runs into a centred auction
        let inline_board_label = !self.settings.center
            && !visibility.show_diagram
            && visibility.show_auction
            && board.auction.is_some()
            && visibility.show_board
            && board.board_id.is_some()
            && !visibility.show_dealer
            && !visibility.show_vulnerable;

        // Adjust title_lines if board label will be inline with auction
        let effective_title_lines = if inline_board_label {
            0 // Board label rendered inline with auction, not as separate line
        } else {
            title_lines
        };

        // Initial height depends on what content we have
        let mut height: f32;

        // Diagram height
        if visibility.show_diagram {
            let diagram_options = DiagramDisplayOptions::from_deal(&board.deal, &board.hidden)
                .with_trick(board.bc_flags, board.play.as_ref());

            // Check for single-card deal - renders just a rank number, not a full diagram
            let is_single_card = board.deal.get_single_visible_card(&board.hidden).is_some();

            if is_single_card {
                // Single-card: board label and rank on same line, vertically centered
                // Just need height for one line of text plus padding
                height = cap_height * 2.0 + line_height;
            } else if diagram_options.hide_compass {
                let diagram_height = self.measure_diagram_height(&diagram_options);
                // North-only: title and cards share same top line, height is the taller of the two
                height = diagram_height.max(cap_height + title_lines as f32 * line_height);
            } else {
                let diagram_height = self.measure_diagram_height(&diagram_options);
                // Full compass: diagram starts at top, no extra spacing needed
                height = diagram_height;
            }
        } else if effective_title_lines > 0 {
            // Title lines but no diagram: cap_height for ascenders + title line spacing
            height = cap_height + effective_title_lines as f32 * line_height;
        } else if inline_board_label {
            // Auction-only with inline board label: cap_height for ascenders
            height = cap_height;
        } else {
            // Commentary-only: position first baseline so ascenders reach start_y
            let commentary_ascender = measurer.ascender_mm(self.settings.commentary_font_size);
            height = commentary_ascender;
        }

        // Diagram commentary, between the diagram and the auction (Center mode)
        if !commentary.under_diagram.is_empty() {
            height += line_height;
            self.add_blocks_height(&mut height, &commentary.under_diagram, column_width);
        }

        // Auction height
        if visibility.show_auction {
            if let Some(ref auction) = board.auction {
                // Use narrowed bid column width if 4 columns don't fit
                let effective_bid_col_width =
                    self.settings.bid_column_width.min(column_width / 4.0);
                let narrowed_settings;
                let bid_settings = if effective_bid_col_width < self.settings.bid_column_width {
                    narrowed_settings = {
                        let mut s = self.settings.clone();
                        s.bid_column_width = effective_bid_col_width;
                        s
                    };
                    Some(&narrowed_settings as &Settings)
                } else {
                    None
                };
                let mut auction_height = self.measure_auction_height(
                    auction,
                    &board.players,
                    Some(column_width),
                    bid_settings,
                );

                // For 2-column inline board labels, we skip the spacing row before the header
                let is_two_col =
                    self.settings.two_col_auctions && auction.uncontested_pair().is_some();
                if inline_board_label && is_two_col {
                    auction_height -= self.settings.bid_row_height;
                }
                height += auction_height;

                let has_contract = board.contract.is_some();
                let has_lead = board
                    .play
                    .as_ref()
                    .and_then(|p| p.tricks.first())
                    .and_then(|t| t.cards[0])
                    .is_some();
                let has_more_below = if self.settings.center {
                    !commentary.below.is_empty()
                } else {
                    visibility.show_commentary && !board.commentary.is_empty()
                };

                // Spacing after auction (only if there's contract or lead)
                if has_contract || has_lead {
                    height += line_height;
                }

                // Center mode: with no contract or lead to step past, the
                // commentary under the auction needs its own line's gap
                if self.settings.center && !has_contract && !has_lead && has_more_below {
                    height += line_height;
                }

                // Contract line
                if has_contract {
                    // Only add spacing if there's more content below
                    if has_lead || has_more_below {
                        height += line_height;
                    }
                }

                // Opening lead line
                if has_lead {
                    // Only add spacing if there's more content below
                    if has_more_below {
                        height += line_height;
                    }
                }
            }
        }

        // Commentary below the board
        self.add_blocks_height(&mut height, &commentary.below, column_width);

        // Event commentary above the board (Center mode): its ascender, the
        // blocks, and a line's gap before the board proper, if there is one
        if !commentary.above.is_empty() {
            let has_body = visibility.show_board
                || visibility.show_dealer
                || visibility.show_vulnerable
                || visibility.show_diagram
                || visibility.show_auction
                || !commentary.under_diagram.is_empty()
                || !commentary.below.is_empty();
            let mut above = measurer.ascender_mm(self.settings.commentary_font_size);
            self.add_blocks_height(&mut above, &commentary.above, column_width);
            height = if has_body {
                above + line_height + height
            } else {
                above
            };
        }

        height
    }

    /// Add the height of consecutive commentary blocks, one line apart, to
    /// `height` -- accumulated in place, in the order the renderer steps down.
    fn add_blocks_height(&self, height: &mut f32, blocks: &[&CommentaryBlock], column_width: f32) {
        for (i, block) in blocks.iter().enumerate() {
            *height += self.measure_commentary_height(block, column_width);
            // Add spacing between blocks, but not after the last one
            if i < blocks.len() - 1 {
                *height += self.settings.line_height;
            }
        }
    }

    /// Measure diagram height without rendering
    fn measure_diagram_height(&self, options: &DiagramDisplayOptions) -> f32 {
        let measurer = get_times_measurer();
        let line_height = self.settings.line_height;
        let cap_height = measurer.cap_height_mm(self.settings.card_font_size);
        let descender = measurer.descender_mm(self.settings.card_font_size);

        // Calculate hand height for given number of suits
        let num_suits = if options.is_fragment {
            options.suits_present.len()
        } else {
            4
        };
        let n = num_suits.max(1) as f32;
        let hand_h = cap_height + (n - 1.0) * line_height + descender;

        // North-only is just the hand height
        if options.hide_compass {
            return hand_h;
        }

        // Calculate compass size (same logic as HandDiagramRenderer::compass_box_size)
        let compass_size = measurer.cap_height_mm(self.settings.body_font_size) * 3.5;

        if options.is_fragment {
            // Fragment: 3 rows with compass centering offset
            let compass_center_offset = (compass_size - hand_h) / 2.0;
            3.0 * hand_h + 2.0 * compass_center_offset
        } else {
            // Full deal: 3 rows of hands
            3.0 * hand_h + 2.0
        }
    }

    /// Measure auction height without rendering
    fn measure_auction_height(
        &self,
        auction: &crate::model::Auction,
        players: &crate::model::PlayerNames,
        notes_max_width: Option<f32>,
        settings_override: Option<&Settings>,
    ) -> f32 {
        // Use the bidding table renderer's static measurement to ensure consistency
        BiddingTableRenderer::measure_height_static(
            auction,
            Some(players),
            settings_override.unwrap_or(&self.settings),
            notes_max_width,
        )
    }

    /// Measure commentary height without rendering
    fn measure_commentary_height(
        &self,
        block: &crate::model::CommentaryBlock,
        max_width: f32,
    ) -> f32 {
        use crate::model::TextSpan;

        let font_size = self.settings.commentary_font_size;
        let line_height = self.settings.line_height;

        // Use the default measurer for estimation
        let measurer = get_times_measurer();
        let base_space_width = measurer.measure_width_mm(" ", font_size);

        // Simple line counting based on text width
        // This is a simplified version - for accurate measurement we'd need full tokenization
        let mut total_width = 0.0;
        let mut line_count = 1;

        for span in &block.content.spans {
            match span {
                TextSpan::Plain(text)
                | TextSpan::Bold(text)
                | TextSpan::Italic(text)
                | TextSpan::BoldItalic(text)
                | TextSpan::Underline(text)
                | TextSpan::Colored { text, .. } => {
                    for word in text.split_whitespace() {
                        let word_width = measurer.measure_width_mm(word, font_size);
                        if total_width + word_width + base_space_width > max_width
                            && total_width > 0.0
                        {
                            line_count += 1;
                            total_width = word_width;
                        } else {
                            total_width += word_width + base_space_width;
                        }
                    }
                    // Count newlines in the text
                    line_count += text.matches('\n').count();
                }
                TextSpan::SuitSymbol(_) | TextSpan::CardRef { .. } => {
                    // Suit symbols and card refs are small, just add a bit of width
                    total_width += measurer.measure_width_mm("♠", font_size);
                }
                TextSpan::LineBreak => {
                    line_count += 1;
                    total_width = 0.0;
                }
            }
        }

        // Advancement from first baseline to last baseline (caller handles cap_height positioning)
        (line_count.max(1) - 1) as f32 * line_height
    }

    /// Generate a PDF from a list of boards
    pub fn render(&self, boards: &[Board]) -> Result<Vec<u8>, RenderError> {
        let title = boards
            .first()
            .and_then(|b| b.event.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("Bridge Hands");

        let mut doc = new_document(title);

        // Load fonts - printpdf 0.8 handles subsetting automatically
        let fonts = FontManager::new(&mut doc)?;

        // Page furniture (issue #28), unless the caller adds its own
        let furniture = self
            .settings
            .page_furniture
            .then(|| PageFurniture::new(&self.settings, &fonts));

        let layers = if self.settings.column_count >= 2 {
            // Multi-column layout: fit multiple boards per page
            self.render_multi_column(boards, &fonts, furniture.as_ref())
        } else {
            // Single board per page (original behavior)
            let mut layers = Vec::new();
            for board in boards {
                let mut layer = LayerBuilder::new();
                let mut top = self.settings.page_height - self.settings.margin_top;
                if let (Some(furniture), Some(event)) = (&furniture, board_event(board)) {
                    if self.settings.page_header {
                        furniture.draw_header(&mut layer, event);
                    } else {
                        top -= furniture.draw_heading(
                            &mut layer,
                            event,
                            self.settings.margin_left,
                            self.settings.content_width(),
                            top,
                        );
                    }
                }
                self.render_board(&mut layer, board, &fonts, self.settings.margin_left, top);
                layers.push(layer);
            }
            layers
        };

        // Footers go on last: they need the page count
        let page_count = layers.len();
        let pages = layers
            .into_iter()
            .enumerate()
            .map(|(i, mut layer)| {
                if let Some(furniture) = &furniture {
                    furniture.draw_footer(&mut layer, i + 1, page_count);
                }
                PdfPage::new(
                    Mm(self.settings.page_width),
                    Mm(self.settings.page_height),
                    layer.into_ops(),
                )
            })
            .collect();

        doc.with_pages(pages);

        // Save with auto-subsetting enabled (default)
        let mut warnings = Vec::new();
        let bytes = doc.save(&save_options(), &mut warnings);

        // Compress PDF streams to reduce file size
        let compressed = compress_pdf(bytes.clone()).unwrap_or(bytes);
        Ok(compressed)
    }

    /// Render boards in multi-column layout with multiple boards per page
    fn render_multi_column(
        &self,
        boards: &[Board],
        fonts: &FontManager,
        furniture: Option<&PageFurniture>,
    ) -> Vec<LayerBuilder> {
        let mut pages = Vec::new();

        let page_width = self.settings.page_width;
        let page_height = self.settings.page_height;
        let margin_top = self.settings.margin_top;
        let margin_bottom = self.settings.margin_bottom;
        let num_columns = self.settings.column_count as usize;

        // Minimum column width for readable content (approx 60mm per column)
        const MIN_COLUMN_WIDTH: f32 = 60.0;
        const DEFAULT_MARGIN: f32 = 15.0;
        let gutter = 5.0; // Space between columns

        // Calculate minimum content width needed for N columns
        let min_content_width =
            num_columns as f32 * MIN_COLUMN_WIDTH + (num_columns - 1) as f32 * gutter;

        // Check if specified margins leave enough room for columns
        let specified_content = page_width - self.settings.margin_left - self.settings.margin_right;
        let (margin_left, margin_right) = if specified_content < min_content_width {
            // Margins too large for multi-column layout, use defaults
            (DEFAULT_MARGIN, DEFAULT_MARGIN)
        } else {
            (self.settings.margin_left, self.settings.margin_right)
        };

        let content_width = page_width - margin_left - margin_right;
        let column_width = content_width / num_columns as f32;
        let usable_column_width =
            column_width - gutter * (num_columns - 1) as f32 / num_columns as f32;

        // Calculate column start X positions and separator X positions
        let column_starts: Vec<f32> = (0..num_columns)
            .map(|i| margin_left + i as f32 * column_width + if i > 0 { gutter / 2.0 } else { 0.0 })
            .collect();
        let separator_positions: Vec<f32> = (1..num_columns)
            .map(|i| margin_left + i as f32 * column_width)
            .collect();

        // Spacing between boards (separator line area)
        let board_spacing = 5.0;

        // Process boards dynamically - fill each column until no more space
        let mut board_iter = boards.iter().peekable();

        while board_iter.peek().is_some() {
            let mut layer = LayerBuilder::new();

            // Page header: the event of the page's first board
            if let Some(furniture) = furniture.filter(|_| self.settings.page_header) {
                if let Some(event) = board_iter.peek().and_then(|b| board_event(b)) {
                    furniture.draw_header(&mut layer, event);
                }
            }

            // Draw vertical separator lines
            layer.set_outline_color(Color::Rgb(SEPARATOR_COLOR));
            layer.set_outline_thickness(SEPARATOR_THICKNESS);
            for sep_x in &separator_positions {
                layer.add_line(
                    Mm(*sep_x),
                    Mm(margin_bottom),
                    Mm(*sep_x),
                    Mm(page_height - margin_top),
                );
            }

            // Track Y position and board count for each column
            let mut column_y: Vec<f32> = vec![page_height - margin_top; num_columns];
            let mut column_board_count: Vec<usize> = vec![0; num_columns];

            // Track if we need to force a page break after this page
            let mut force_page_break = false;

            // Fill columns left to right
            for col_idx in 0..num_columns {
                if force_page_break {
                    break;
                }

                let col_x = column_starts[col_idx];
                let col_end_x = if col_idx < num_columns - 1 {
                    separator_positions[col_idx] - gutter / 2.0
                } else {
                    page_width - margin_right
                };

                // Without a page header, the event heads each column: that of
                // the board the column starts with
                if let Some(furniture) = furniture.filter(|_| !self.settings.page_header) {
                    if let Some(event) = board_iter.peek().and_then(|b| board_event(b)) {
                        column_y[col_idx] -= furniture.draw_heading(
                            &mut layer,
                            event,
                            col_x,
                            usable_column_width,
                            column_y[col_idx],
                        );
                    }
                }

                while let Some(&next) = board_iter.peek() {
                    // Page break marker - force new page
                    if is_page_break(next) {
                        // Name-based markers are just markers with no content — consume them
                        // BCFlags-based breaks have content — only break if column has boards
                        // (first board in a new page renders normally, preventing infinite loop)
                        let is_bcflags = next.bc_flags.as_ref().is_some_and(|f| f.page_break());
                        if is_bcflags && column_board_count[col_idx] == 0 {
                            // First board in column — render it, don't break
                        } else {
                            if !is_bcflags {
                                board_iter.next(); // Consume name-based break marker
                            }
                            force_page_break = true;
                            break;
                        }
                    }

                    // Column break marker - move to next column
                    if is_column_break(next) {
                        let is_bcflags = next.bc_flags.as_ref().is_some_and(|f| f.column_break());
                        if is_bcflags && column_board_count[col_idx] == 0 {
                            // First board in column — render it, don't break
                        } else {
                            if !is_bcflags {
                                board_iter.next(); // Consume name-based break marker
                            }
                            break;
                        }
                    }

                    // Measure the board height to check if it fits
                    let board_height = self.measure_board_height(next, usable_column_width);

                    // Skip empty boards (height 0)
                    if board_height == 0.0 {
                        board_iter.next(); // Consume and skip
                        continue;
                    }

                    // Check if board fits in remaining space
                    let available = column_y[col_idx] - margin_bottom;
                    if board_height + board_spacing > available && column_board_count[col_idx] > 0 {
                        // Doesn't fit and we have at least one board - move to next column
                        break;
                    }

                    // Board fits - consume and render it
                    let board = board_iter.next().unwrap();

                    // Draw horizontal separator if not at top
                    if column_board_count[col_idx] > 0 {
                        let sep_y = column_y[col_idx] + board_spacing / 2.0;
                        layer.set_outline_color(Color::Rgb(SEPARATOR_COLOR));
                        layer.set_outline_thickness(SEPARATOR_THICKNESS);
                        layer.add_line(Mm(col_x), Mm(sep_y), Mm(col_end_x), Mm(sep_y));
                    }

                    let rendered_height = self.render_board_in_column(
                        &mut layer,
                        board,
                        fonts,
                        col_x,
                        column_y[col_idx],
                        usable_column_width,
                    );

                    // Draw debug box around the whole board
                    self.draw_board_debug_box(
                        &mut layer,
                        col_x,
                        column_y[col_idx],
                        usable_column_width,
                        rendered_height,
                    );

                    column_y[col_idx] -= rendered_height + board_spacing;
                    column_board_count[col_idx] += 1;
                }
            }

            pages.push(layer);
        }

        pages
    }

    /// Render a board within a column (for multi-column layout)
    fn render_board_in_column(
        &self,
        layer: &mut LayerBuilder,
        board: &Board,
        fonts: &FontManager,
        column_x: f32,
        start_y: f32,
        column_width: f32,
    ) -> f32 {
        let line_height = self.settings.line_height;

        // Get font sets
        let diagram_fonts = fonts.builtin_set_for_spec(self.settings.fonts.diagram.as_ref());
        let card_table_fonts = fonts.builtin_set_for_spec(self.settings.fonts.card_table.as_ref());
        let hand_record_fonts =
            fonts.builtin_set_for_spec(self.settings.fonts.hand_record.as_ref());

        let measurer = get_times_measurer();
        let cap_height = measurer.cap_height_mm(self.settings.body_font_size);

        let mut current_y: f32;

        // Check BCFlags for visibility control
        // Show board info if deal has cards OR there's an auction (for exercise boards)
        // Board label is tied to diagram - if BCFlags says no diagram, no board label either
        let flags = board.bc_flags;
        let deal_is_empty = board.deal.is_empty();
        let has_auction = board
            .auction
            .as_ref()
            .map(|a| !a.calls.is_empty())
            .unwrap_or(false);
        let has_content = !deal_is_empty || has_auction;
        let show_diagram_flag = flags.map(|f| f.show_diagram()).unwrap_or(true);
        let show_board = has_content
            && self.settings.show_board_labels
            && flags.map(|f| !f.hide_board()).unwrap_or(true)
            && show_diagram_flag;
        let show_dealer = has_content
            && self.settings.show_board_labels
            && flags.map(|f| !f.hide_dealer()).unwrap_or(true)
            && show_diagram_flag;
        let show_vulnerable = has_content
            && self.settings.show_board_labels
            && flags.map(|f| !f.hide_vulnerable()).unwrap_or(true)
            && show_diagram_flag;
        let show_diagram = !deal_is_empty && show_diagram_flag && !board.hidden.all_hidden();
        let show_auction = has_auction
            && flags.map(|f| f.show_auction()).unwrap_or(true)
            && self.settings.show_bidding;
        let commentary = ColumnCommentary::of(board, &self.settings);
        let show_commentary = commentary.any();

        // Skip completely empty boards (nothing visible to show)
        if !show_board
            && !show_dealer
            && !show_vulnerable
            && !show_diagram
            && !show_auction
            && !show_commentary
        {
            return 0.0;
        }

        // For auction-only boards (no diagram), render board number inline with auction header
        // This saves vertical space by not having the board number on its own line
        // Not in Center mode: see measure_board_height
        let inline_board_label = !self.settings.center
            && !show_diagram
            && show_auction
            && board.auction.is_some()
            && show_board
            && board.board_id.is_some()
            && !show_dealer
            && !show_vulnerable;

        // Event commentary above the board (Center mode). The board proper
        // starts a line below it; `top` stays this board's top, for the height
        // returned.
        let top = start_y;
        let start_y = if commentary.above.is_empty() {
            start_y
        } else {
            let ascender = get_times_measurer().ascender_mm(self.settings.commentary_font_size);
            let end = self.render_blocks(
                layer,
                &commentary.above,
                fonts,
                column_x,
                start_y - ascender,
                column_width,
            );
            let has_body = show_board
                || show_dealer
                || show_vulnerable
                || show_diagram
                || show_auction
                || !commentary.under_diagram.is_empty()
                || !commentary.below.is_empty();
            if !has_body {
                return top - end;
            }
            end - line_height
        };

        // Build and render title lines (Deal #, Dealer, Vulnerability)
        let font_size = self.settings.body_font_size;

        // Title baseline: cap_height below start_y so text top aligns with start_y
        let first_baseline = start_y - cap_height;
        let mut title_line = 0;

        layer.set_fill_color(Color::Rgb(BLACK));

        // Check for single-card deal - these get special centered rendering
        let is_single_card = board.deal.get_single_visible_card(&board.hidden).is_some();

        // Render board number in title section (unless it will be inline with auction or single-card)
        if show_board && !inline_board_label && !is_single_card {
            if let Some(ref board_id) = board.board_id {
                let y = first_baseline - (title_line as f32 * line_height);
                // Use board label format from settings (e.g., "Board %" -> "Board 1", "%)" -> "1)")
                let label = self.settings.board_label_format.replace('%', board_id);
                layer.use_text_builtin(
                    label,
                    font_size,
                    Mm(column_x),
                    Mm(y),
                    hand_record_fonts.bold_italic,
                );
                title_line += 1;
            }
        }

        if show_dealer && !is_single_card {
            if let Some(dealer) = board.dealer {
                let y = first_baseline - (title_line as f32 * line_height);
                layer.use_text_builtin(
                    format!("{} Deals", dealer),
                    font_size,
                    Mm(column_x),
                    Mm(y),
                    hand_record_fonts.regular,
                );
                title_line += 1;
            }
        }

        if show_vulnerable && !is_single_card {
            let y = first_baseline - (title_line as f32 * line_height);
            layer.use_text_builtin(
                board.vulnerable.to_string(),
                font_size,
                Mm(column_x),
                Mm(y),
                hand_record_fonts.regular,
            );
        }

        // Render hand diagram if enabled
        if show_diagram {
            let diagram_x = column_x;

            // Compute display options - all visibility decisions are made here
            let diagram_options = DiagramDisplayOptions::from_deal(&board.deal, &board.hidden)
                .with_trick(board.bc_flags, board.play.as_ref());

            // Check for single-card deal - render just the rank number instead of a full diagram
            if let Some((_suit, rank)) = board.deal.get_single_visible_card(&board.hidden) {
                // Single-card: render board label and rank centered vertically
                // Total available height: cap_height * 2.0 + line_height (from measurement)
                let total_height = cap_height * 2.0 + line_height;
                // Content is just one line of text
                let content_height = line_height;
                // Vertical offset to center content
                let vertical_offset = (total_height - content_height) / 2.0;
                let centered_y = start_y - vertical_offset - cap_height;

                // Render board label on the left
                if show_board {
                    if let Some(ref board_id) = board.board_id {
                        let label = self.settings.board_label_format.replace('%', board_id);
                        layer.use_text_builtin(
                            label,
                            font_size,
                            Mm(column_x),
                            Mm(centered_y),
                            hand_record_fonts.bold_italic,
                        );
                    }
                }

                // Render rank number centered in the diagram area
                let rank_text = rank.display_str().to_string();
                let rank_font_size = font_size; // Use same font size as board label

                // Calculate x position - center in the diagram area
                let hand_w = self.settings.hand_width;
                let compass_size = 10.0; // Approximate compass box size
                let north_base_x = diagram_x + hand_w + (compass_size - hand_w) / 2.0;
                let compass_center_x = north_base_x + compass_size / 2.0;

                // Measure text width for centering
                let text_measurer = text_metrics::BuiltinFontMeasurer::new(diagram_fonts.regular);
                let text_width = text_measurer.measure_width_mm(&rank_text, rank_font_size);
                let rank_x = compass_center_x - text_width / 2.0;

                layer.use_text_builtin(
                    rank_text,
                    rank_font_size,
                    Mm(rank_x),
                    Mm(centered_y),
                    diagram_fonts.regular,
                );

                // Debug box for single card area
                self.draw_debug_box(layer, column_x, start_y, column_width, total_height);

                current_y = start_y - total_height;
            } else {
                // Full compass: diagram starts at start_y (title already moved down by title_spacing)
                // Hidden compass: cards should be on same line as title text
                // The diagram renderer subtracts cap_height internally, so we add it back
                let diagram_y = if diagram_options.hide_compass {
                    first_baseline + cap_height
                } else {
                    start_y
                };

                let hand_renderer = HandDiagramRenderer::new(
                    diagram_fonts.regular,
                    diagram_fonts.bold,
                    card_table_fonts.regular,
                    fonts.symbol_font(),
                    &self.settings,
                );
                let diagram_height = hand_renderer.render_deal_with_options(
                    layer,
                    &board.deal,
                    (Mm(diagram_x), Mm(diagram_y)),
                    &diagram_options,
                );

                // Debug box for diagram
                self.draw_debug_box(layer, diagram_x, diagram_y, column_width, diagram_height);

                current_y = diagram_y - diagram_height;
            }
        } else if inline_board_label {
            // Auction-only with inline board label: position at first_baseline
            current_y = first_baseline;
        } else if title_line > 0 {
            // Title lines: content starts below title lines
            current_y = first_baseline - (title_line as f32 * line_height);
        } else {
            // Commentary-only: position first baseline so ascenders reach start_y
            let commentary_ascender =
                get_times_measurer().ascender_mm(self.settings.commentary_font_size);
            current_y = start_y - commentary_ascender;
        }

        // Diagram commentary, between the diagram and the auction (Center mode)
        if !commentary.under_diagram.is_empty() {
            current_y = self.render_blocks(
                layer,
                &commentary.under_diagram,
                fonts,
                column_x,
                current_y - line_height,
                column_width,
            );
        }

        // Render bidding table if present and enabled
        if show_auction {
            if let Some(ref auction) = board.auction {
                // Calculate effective bid column width that fits 4 columns in the column
                // Use the same width for 2-col and 4-col so columns align vertically
                let effective_bid_col_width =
                    self.settings.bid_column_width.min(column_width / 4.0);
                let num_cols =
                    if self.settings.two_col_auctions && auction.uncontested_pair().is_some() {
                        2
                    } else {
                        4
                    };
                let table_width = num_cols as f32 * effective_bid_col_width;

                // Use narrowed bid column width if needed to fit
                let narrowed_settings;
                let bid_settings = if effective_bid_col_width < self.settings.bid_column_width {
                    narrowed_settings = {
                        let mut s = self.settings.clone();
                        s.bid_column_width = effective_bid_col_width;
                        s
                    };
                    &narrowed_settings
                } else {
                    &self.settings
                };
                let bidding_renderer = BiddingTableRenderer::new(
                    hand_record_fonts.regular,
                    hand_record_fonts.bold,
                    hand_record_fonts.italic,
                    fonts.symbol_font(),
                    bid_settings,
                );

                // Center the auction table within the column
                let table_x = column_x + (column_width - table_width) / 2.0;

                // Render board label inline with auction header (to the left of the table)
                // For 2-column auctions, eliminate the spacing row and put label on header line
                // For 4-column auctions, place label above the header (keep spacing row)
                let auction_y = if inline_board_label && num_cols == 2 {
                    // 2-column inline: move auction up by one row so header aligns with first_baseline
                    // The auction adds row_height spacing internally, so we compensate here
                    current_y + self.settings.bid_row_height
                } else {
                    current_y
                };

                if inline_board_label {
                    if let Some(ref board_id) = board.board_id {
                        let label = self.settings.board_label_format.replace('%', board_id);
                        // Board label at first_baseline (same line as auction header after offset)
                        layer.use_text_builtin(
                            label,
                            font_size,
                            Mm(column_x),
                            Mm(current_y),
                            hand_record_fonts.bold_italic,
                        );
                    }
                }

                // Calculate max width for notes: from table_x to right edge of column
                let notes_max_width = (column_x + column_width) - table_x;
                let table_height = bidding_renderer.render_with_players_and_notes_width(
                    layer,
                    auction,
                    (Mm(table_x), Mm(auction_y)),
                    Some(&board.players),
                    Some(notes_max_width),
                );

                // Debug box for bidding table: tightly bound visible content
                // table_height is measured from origin which includes an empty spacing row (row 0)
                // Header is at row 1 (origin - row_height), so subtract spacing row and add ascender
                let row_height = self.settings.bid_row_height;
                let bid_asc = get_times_measurer().ascender_mm(self.settings.body_font_size);
                let box_top = auction_y - row_height + bid_asc;
                let box_height = table_height - row_height + bid_asc;
                self.draw_debug_box(layer, table_x, box_top, table_width, box_height);

                // For 2-column inline labels, we moved auction up by row_height, so subtract less
                if inline_board_label && num_cols == 2 {
                    current_y -= table_height - self.settings.bid_row_height;
                } else {
                    current_y -= table_height;
                }

                let has_contract = board.contract.is_some();
                let has_lead = board
                    .play
                    .as_ref()
                    .and_then(|p| p.tricks.first())
                    .and_then(|t| t.cards[0])
                    .is_some();
                let has_more_below = if self.settings.center {
                    !commentary.below.is_empty()
                } else {
                    show_commentary && !board.commentary.is_empty()
                };

                // Add spacing after auction before contract/lead (only if there's contract or lead)
                if has_contract || has_lead {
                    current_y -= line_height;
                }

                // Center mode: with no contract or lead to step past, the
                // commentary under the auction needs its own line's gap
                if self.settings.center && !has_contract && !has_lead && has_more_below {
                    current_y -= line_height;
                }

                // Render contract (only if explicitly in PBN, not inferred from auction)
                if let Some(ref contract) = board.contract {
                    let colors =
                        SuitColors::new(self.settings.black_color, self.settings.red_color);
                    self.render_contract(
                        layer,
                        contract,
                        Mm(column_x),
                        Mm(current_y),
                        hand_record_fonts.regular,
                        fonts.symbol_font(),
                        &colors,
                    );
                    // Only add spacing if there's more content below
                    if has_lead || has_more_below {
                        current_y -= line_height;
                    }
                }

                // Render opening lead
                if let Some(ref play) = board.play {
                    if let Some(first_trick) = play.tricks.first() {
                        if let Some(lead_card) = first_trick.cards[0] {
                            let colors =
                                SuitColors::new(self.settings.black_color, self.settings.red_color);
                            self.render_lead(
                                layer,
                                &lead_card,
                                Mm(column_x),
                                Mm(current_y),
                                hand_record_fonts.regular,
                                fonts.symbol_font(),
                                &colors,
                            );
                            // Only add spacing if there's more content below
                            if has_more_below {
                                current_y -= line_height;
                            }
                        }
                    }
                }
            }
        }

        // Commentary below the board - no floating in a column
        current_y = self.render_blocks(
            layer,
            &commentary.below,
            fonts,
            column_x,
            current_y,
            column_width,
        );

        // Return total height used
        top - current_y
    }

    /// Render consecutive commentary blocks the width of the column, one line
    /// apart, the first with its baseline at `y`. Returns where the last ends.
    fn render_blocks(
        &self,
        layer: &mut LayerBuilder,
        blocks: &[&CommentaryBlock],
        fonts: &FontManager,
        column_x: f32,
        y: f32,
        column_width: f32,
    ) -> f32 {
        let commentary_fonts = fonts.builtin_set_for_spec(self.settings.fonts.commentary.as_ref());
        let commentary_renderer = CommentaryRenderer::new(
            commentary_fonts.regular,
            commentary_fonts.bold,
            commentary_fonts.italic,
            commentary_fonts.bold_italic,
            fonts.symbol_font(),
            &self.settings,
        );
        let mut current_y = y;
        for (i, block) in blocks.iter().enumerate() {
            let block_start_y = current_y;
            let height = commentary_renderer.render(
                layer,
                block,
                (Mm(column_x), Mm(current_y)),
                column_width,
            );

            // Debug box for commentary block (top at ascender above baseline, bottom at last baseline)
            let asc = get_times_measurer().ascender_mm(self.settings.commentary_font_size);
            self.draw_debug_box(
                layer,
                column_x,
                block_start_y + asc,
                column_width,
                asc + height,
            );

            current_y -= height;
            // Add spacing between blocks, but not after the last one
            if i < blocks.len() - 1 {
                current_y -= self.settings.line_height;
            }
        }
        current_y
    }

    /// Draw a debug outline box (gray, for components)
    fn draw_debug_box(&self, layer: &mut LayerBuilder, x: f32, y: f32, w: f32, h: f32) {
        if !self.settings.debug_boxes {
            return;
        }
        // y is top of box, draw from bottom-left to top-right
        layer.set_outline_color(Color::Rgb(DEBUG_BOX_COLOR));
        layer.set_outline_thickness(0.25);
        layer.add_rect(Mm(x), Mm(y - h), Mm(x + w), Mm(y), PaintMode::Stroke);
    }

    /// Draw a board-level debug outline box (blue, for whole boards)
    fn draw_board_debug_box(&self, layer: &mut LayerBuilder, x: f32, y: f32, w: f32, h: f32) {
        if !self.settings.debug_boxes {
            return;
        }
        // y is top of box, draw from bottom-left to top-right
        layer.set_outline_color(Color::Rgb(DEBUG_BOARD_BOX_COLOR));
        layer.set_outline_thickness(0.5);
        layer.add_rect(Mm(x), Mm(y - h), Mm(x + w), Mm(y), PaintMode::Stroke);
    }

    /// Render a single board - Bridge Composer style layout
    fn render_board(
        &self,
        layer: &mut LayerBuilder,
        board: &Board,
        fonts: &FontManager,
        margin_left: f32,
        top: f32,
    ) {
        // The top of the board: the top margin, or below a page heading
        let page_top = top;
        let line_height = self.settings.line_height;

        // Get font sets based on PBN font specifications
        let diagram_fonts = fonts.builtin_set_for_spec(self.settings.fonts.diagram.as_ref());
        let card_table_fonts = fonts.builtin_set_for_spec(self.settings.fonts.card_table.as_ref());
        let hand_record_fonts =
            fonts.builtin_set_for_spec(self.settings.fonts.hand_record.as_ref());
        let commentary_fonts = fonts.builtin_set_for_spec(self.settings.fonts.commentary.as_ref());

        // Get font metrics for accurate box heights
        let measurer = get_times_measurer();
        let cap_height = measurer.cap_height_mm(self.settings.body_font_size);
        let descender = measurer.descender_mm(self.settings.body_font_size);

        // Title: 3 lines stacked vertically, positioned above West hand area
        let title_x = margin_left;
        let title_start_y = page_top;

        // Build title lines and measure widths
        // Show board info if deal has cards OR there's an auction (for exercise boards)
        let font_size = self.settings.body_font_size;
        let deal_is_empty = board.deal.is_empty();
        let has_auction = board
            .auction
            .as_ref()
            .map(|a| !a.calls.is_empty())
            .unwrap_or(false);
        let has_content = !deal_is_empty || has_auction;

        // Up to three lines: board label (bold italic), dealer, vulnerability.
        // BCFlags hides each one on its own, and `%ShowBoardLabels 0` hides all
        // three -- BridgeComposer honours both on one-board-per-page files too.
        let flags = board.bc_flags;
        let show_labels = has_content && self.settings.show_board_labels;
        let shown =
            |hidden: fn(&BCFlags) -> bool| show_labels && !flags.as_ref().is_some_and(hidden);
        let mut title_lines: Vec<(String, BuiltinFont)> = Vec::new();
        if let Some(board_id) = board
            .board_id
            .as_ref()
            .filter(|_| shown(BCFlags::hide_board))
        {
            // Use board label format from settings (e.g., "Board %" -> "Board 1", "%)" -> "1)")
            let label = self.settings.board_label_format.replace('%', board_id);
            title_lines.push((label, hand_record_fonts.bold_italic));
        }
        if let Some(dealer) = board.dealer.filter(|_| shown(BCFlags::hide_dealer)) {
            title_lines.push((format!("{} Deals", dealer), hand_record_fonts.regular));
        }
        if shown(BCFlags::hide_vulnerable) {
            title_lines.push((board.vulnerable.to_string(), hand_record_fonts.regular));
        }

        let num_lines = title_lines.len();

        // Calculate actual width by measuring all lines
        let title_width = title_lines
            .iter()
            .map(|(line, _)| measurer.measure_width_mm(line, font_size))
            .fold(0.0_f32, |max, w| max.max(w));

        // Title box height: cap_height + (num_lines - 1) gaps + descender
        let title_height =
            cap_height + num_lines.saturating_sub(1) as f32 * line_height + descender;

        // Draw debug box around title area
        self.draw_debug_box(layer, title_x, title_start_y, title_width, title_height);

        // Render title text with cap-height offset
        let first_baseline = title_start_y - cap_height;

        layer.set_fill_color(Color::Rgb(BLACK));

        for (i, (line, font)) in title_lines.iter().enumerate() {
            let y = first_baseline - (i as f32 * line_height);
            layer.use_text_builtin(
                line,
                self.settings.body_font_size,
                Mm(title_x),
                Mm(y),
                *font,
            );
        }

        // Diagram origin: same Y as page_top (North aligns with "Board 1")
        // The diagram renderer will place North to the right (after hand_width gap for title)
        let diagram_x = margin_left;
        let diagram_y = page_top; // Start at same level as title

        // Content below diagram (or title if no diagram)
        let mut content_y;

        // Only render diagram if deal has cards
        if !deal_is_empty {
            // Compute display options - all visibility decisions are made here
            let diagram_options = DiagramDisplayOptions::from_deal(&board.deal, &board.hidden)
                .with_trick(board.bc_flags, board.play.as_ref());

            let hand_renderer = HandDiagramRenderer::new(
                diagram_fonts.regular,
                diagram_fonts.bold,
                card_table_fonts.regular, // Compass uses CardTable font
                fonts.symbol_font(),      // DejaVu Sans for suit symbols
                &self.settings,
            );
            let diagram_height = hand_renderer.render_deal_with_options(
                layer,
                &board.deal,
                (Mm(diagram_x), Mm(diagram_y)),
                &diagram_options,
            );

            // Debug box for diagram
            let content_width = self.settings.content_width();
            self.draw_debug_box(
                layer,
                diagram_x,
                diagram_y,
                content_width / 2.0,
                diagram_height,
            );

            content_y = Mm(diagram_y - diagram_height - 5.0);
        } else {
            // No diagram, content starts below any title lines
            let title_height = title_lines.len() as f32 * line_height;
            content_y = Mm(page_top - title_height - 5.0);
        }

        // Render bidding table if present
        if self.settings.show_bidding {
            if let Some(ref auction) = board.auction {
                let bidding_renderer = BiddingTableRenderer::new(
                    hand_record_fonts.regular,
                    hand_record_fonts.bold,
                    hand_record_fonts.italic,
                    fonts.symbol_font(), // DejaVu Sans for suit symbols
                    &self.settings,
                );
                // Notes wrap to the left half when commentary will float on the right,
                // otherwise use the full content width.
                let has_floating_commentary =
                    self.settings.show_commentary && board.commentary.iter().any(|c| !c.is_blank());
                let notes_max_width = if has_floating_commentary {
                    self.settings.content_width() / 2.0 - 2.0
                } else {
                    self.settings.content_width()
                };
                let num_cols =
                    if self.settings.two_col_auctions && auction.uncontested_pair().is_some() {
                        2
                    } else {
                        4
                    };
                let table_width = num_cols as f32 * self.settings.bid_column_width;
                let table_height = bidding_renderer.render_with_players_and_notes_width(
                    layer,
                    auction,
                    (Mm(margin_left), content_y),
                    Some(&board.players),
                    Some(notes_max_width),
                );

                // Debug box for bidding table
                self.draw_debug_box(layer, margin_left, content_y.0, table_width, table_height);

                content_y = Mm(content_y.0 - table_height);

                let has_contract = board.contract.is_some();
                let has_lead = board
                    .play
                    .as_ref()
                    .and_then(|p| p.tricks.first())
                    .and_then(|t| t.cards[0])
                    .is_some();

                // Add spacing after auction before contract/lead
                if has_contract || has_lead {
                    content_y = Mm(content_y.0 - line_height);
                }

                // Render contract below auction (only if explicitly in PBN)
                if let Some(ref contract) = board.contract {
                    let colors =
                        SuitColors::new(self.settings.black_color, self.settings.red_color);
                    let x = self.render_contract(
                        layer,
                        contract,
                        Mm(margin_left),
                        content_y,
                        hand_record_fonts.regular,
                        fonts.symbol_font(),
                        &colors,
                    );
                    // Debug box for contract line
                    let contract_width = x - margin_left;
                    self.draw_debug_box(
                        layer,
                        margin_left,
                        content_y.0 + cap_height,
                        contract_width,
                        cap_height + descender,
                    );
                    if has_lead {
                        content_y = Mm(content_y.0 - line_height);
                    }
                }

                // Render opening lead if play sequence exists
                if let Some(ref play) = board.play {
                    if let Some(first_trick) = play.tricks.first() {
                        if let Some(lead_card) = first_trick.cards[0] {
                            let colors =
                                SuitColors::new(self.settings.black_color, self.settings.red_color);
                            self.render_lead(
                                layer,
                                &lead_card,
                                Mm(margin_left),
                                content_y,
                                hand_record_fonts.regular,
                                fonts.symbol_font(),
                                &colors,
                            );
                            // Debug box for lead line
                            self.draw_debug_box(
                                layer,
                                margin_left,
                                content_y.0 + cap_height,
                                table_width,
                                cap_height + descender,
                            );
                        }
                    }
                }

                content_y = Mm(content_y.0 - 3.0);
            }
        }

        // Render commentary if present - using floating layout
        if self.settings.show_commentary && board.commentary.iter().any(|c| !c.is_blank()) {
            let commentary_renderer = CommentaryRenderer::new(
                commentary_fonts.regular,
                commentary_fonts.bold,
                commentary_fonts.italic,
                commentary_fonts.bold_italic,
                fonts.symbol_font(), // DejaVu Sans for suit symbols
                &self.settings,
            );

            // Calculate floating layout:
            // - Commentary starts at page_top, on the right half of the page
            // - Float until we clear the deal info (content_y is below diagram + bidding + contract + lead)
            // - Then switch to full width

            let full_width = self.settings.content_width();
            let page_center = margin_left + full_width / 2.0;
            let float_width = full_width / 2.0 - 2.0; // Small gap from center

            // The float_until_y is where the deal content ends (current content_y)
            let float_until_y = content_y.0;

            let float_layout = FloatLayout {
                float_until_y,
                float_left: page_center + 2.0, // Start just right of center
                float_width,
                full_left: margin_left,
                full_width,
            };

            // Start commentary at the top of the page, using floating layout (skip blank blocks)
            // Position first baseline so cap tops align with page_top (matching title text)
            let commentary_cap =
                get_times_measurer().cap_height_mm(self.settings.commentary_font_size);
            let mut commentary_y = page_top - commentary_cap;
            let mut first_block = true;
            let non_blank_blocks: Vec<_> =
                board.commentary.iter().filter(|c| !c.is_blank()).collect();

            let commentary_asc =
                get_times_measurer().ascender_mm(self.settings.commentary_font_size);

            for block in &non_blank_blocks {
                if first_block {
                    // First block uses floating layout
                    let block_start_y = commentary_y;
                    let result = commentary_renderer.render_float(
                        layer,
                        block,
                        (Mm(float_layout.float_left), Mm(commentary_y)),
                        &float_layout,
                    );
                    // Debug box for floating commentary block
                    let float_height = block_start_y - result.final_y + line_height;
                    self.draw_debug_box(
                        layer,
                        float_layout.float_left,
                        block_start_y + commentary_asc,
                        float_layout.float_width,
                        float_height + commentary_asc,
                    );
                    commentary_y = result.final_y - line_height;
                    first_block = false;

                    // Update content_y if commentary went below the deal content
                    if commentary_y < content_y.0 {
                        content_y = Mm(commentary_y);
                    }
                } else {
                    // Subsequent blocks float until a line would clear the deal
                    // content -- the same test render_float applies per line
                    if !float_layout.clears(commentary_y, commentary_renderer.line_ascent()) {
                        // Still in float zone
                        let block_start_y = commentary_y;
                        let result = commentary_renderer.render_float(
                            layer,
                            block,
                            (Mm(float_layout.float_left), Mm(commentary_y)),
                            &float_layout,
                        );
                        let float_height = block_start_y - result.final_y + line_height;
                        self.draw_debug_box(
                            layer,
                            float_layout.float_left,
                            block_start_y + commentary_asc,
                            float_layout.float_width,
                            float_height + commentary_asc,
                        );
                        commentary_y = result.final_y - line_height;
                    } else {
                        // Below float zone, use full width
                        let block_start_y = commentary_y;
                        let height = commentary_renderer.render(
                            layer,
                            block,
                            (Mm(margin_left), Mm(commentary_y)),
                            full_width,
                        );
                        self.draw_debug_box(
                            layer,
                            margin_left,
                            block_start_y + commentary_asc,
                            full_width,
                            commentary_asc + height,
                        );
                        commentary_y -= height + line_height;
                    }

                    if commentary_y < content_y.0 {
                        content_y = Mm(commentary_y);
                    }
                }
            }
        }
    }

    /// Render a contract with proper suit symbol font
    /// Returns the x position after the rendered text
    #[allow(clippy::too_many_arguments)]
    fn render_contract(
        &self,
        layer: &mut LayerBuilder,
        contract: &crate::model::FinalContract,
        x: Mm,
        y: Mm,
        text_font: BuiltinFont,
        symbol_font: &FontId,
        colors: &SuitColors,
    ) -> f32 {
        let measurer = get_times_measurer();
        let font_size = self.settings.body_font_size;
        let mut current_x = x.0;

        // Render level
        let level_str = contract.level.to_string();
        layer.set_fill_color(Color::Rgb(BLACK));
        layer.use_text_builtin(&level_str, font_size, Mm(current_x), y, text_font);
        current_x += measurer.measure_width_mm(&level_str, font_size);

        // Render suit symbol (or NT)
        let (symbol, use_symbol_font) = match contract.strain {
            BidSuit::Clubs => ("♣", true),
            BidSuit::Diamonds => ("♦", true),
            BidSuit::Hearts => ("♥", true),
            BidSuit::Spades => ("♠", true),
            BidSuit::NoTrump => ("NT", false),
        };

        if contract.strain.is_red() {
            layer.set_fill_color(Color::Rgb(colors.hearts.clone()));
        } else {
            layer.set_fill_color(Color::Rgb(BLACK));
        }

        if use_symbol_font {
            layer.use_text(symbol, font_size, Mm(current_x), y, symbol_font);
        } else {
            layer.use_text_builtin(symbol, font_size, Mm(current_x), y, text_font);
        }
        current_x += measurer.measure_width_mm(symbol, font_size);

        // Render doubled/redoubled
        layer.set_fill_color(Color::Rgb(BLACK));
        if contract.redoubled {
            layer.use_text_builtin("XX", font_size, Mm(current_x), y, text_font);
            current_x += measurer.measure_width_mm("XX", font_size);
        } else if contract.doubled {
            layer.use_text_builtin("X", font_size, Mm(current_x), y, text_font);
            current_x += measurer.measure_width_mm("X", font_size);
        }

        // Render " by [declarer]"
        let by_text = format!(" by {}", contract.declarer);
        layer.use_text_builtin(&by_text, font_size, Mm(current_x), y, text_font);
        current_x += measurer.measure_width_mm(&by_text, font_size);

        current_x
    }

    /// Render opening lead with proper suit symbol font
    #[allow(clippy::too_many_arguments)]
    fn render_lead(
        &self,
        layer: &mut LayerBuilder,
        card: &crate::model::Card,
        x: Mm,
        y: Mm,
        text_font: BuiltinFont,
        symbol_font: &FontId,
        colors: &SuitColors,
    ) {
        let measurer = get_times_measurer();
        let font_size = self.settings.body_font_size;
        let mut current_x = x.0;

        // Render "Lead: "
        let prefix = "Lead: ";
        layer.set_fill_color(Color::Rgb(BLACK));
        layer.use_text_builtin(prefix, font_size, Mm(current_x), y, text_font);
        current_x += measurer.measure_width_mm(prefix, font_size);

        // Render suit symbol with color
        let symbol = card.suit.symbol().to_string();
        let suit_color = colors.for_suit(&card.suit);
        layer.set_fill_color(Color::Rgb(suit_color));
        layer.use_text(&symbol, font_size, Mm(current_x), y, symbol_font);
        current_x += measurer.measure_width_mm(&symbol, font_size);

        // Render rank in black
        let rank = card.rank.display_str().to_string();
        layer.set_fill_color(Color::Rgb(BLACK));
        layer.use_text_builtin(&rank, font_size, Mm(current_x), y, text_font);
    }
}

/// Convenience function to generate PDF
pub fn generate_pdf(boards: &[Board], settings: &Settings) -> Result<Vec<u8>, RenderError> {
    let renderer = DocumentRenderer::new(settings.clone());
    renderer.render(boards)
}
