use crate::config::Settings;
use crate::model::{
    AnnotatedCall, Auction, AuctionExt, BidSuit, Call, Direction, DirectionExt, PlayerNames,
};
use crate::parser::replace_suit_escapes;
use printpdf::{BuiltinFont, Color, FontId, Mm};

use crate::render::helpers::colors::{SuitColors, BLACK};
use crate::render::helpers::layer::LayerBuilder;
use crate::render::helpers::text_metrics;

/// Size ratio for superscript text relative to body font
const SUPERSCRIPT_RATIO: f32 = 0.65;
/// Vertical offset for superscript as fraction of font size
const SUPERSCRIPT_RISE: f32 = 0.4;

/// Whether an annotation is a suffix marker (`!`, `?`, `!?`, `??`, ...) rather
/// than a note reference.
fn is_suffix_annotation(annotation: &str) -> bool {
    !annotation.is_empty() && annotation.chars().all(|c| c == '!' || c == '?')
}

/// Renderer for bidding tables
pub struct BiddingTableRenderer<'a> {
    font: BuiltinFont,
    #[allow(dead_code)]
    bold_font: BuiltinFont,
    italic_font: BuiltinFont,
    symbol_font: &'a FontId, // Font with Unicode suit symbols (DejaVu Sans)
    colors: SuitColors,
    settings: &'a Settings,
    use_sans_measurer: bool,
}

impl<'a> BiddingTableRenderer<'a> {
    pub fn new(
        font: BuiltinFont,
        bold_font: BuiltinFont,
        italic_font: BuiltinFont,
        symbol_font: &'a FontId,
        settings: &'a Settings,
    ) -> Self {
        // Determine if we should use sans-serif measurement based on font settings
        let use_sans_measurer = settings
            .fonts
            .hand_record
            .as_ref()
            .map(|f| f.is_sans_serif())
            .unwrap_or(false);

        Self {
            font,
            bold_font,
            italic_font,
            symbol_font,
            colors: SuitColors::new(settings.black_color, settings.red_color),
            settings,
            use_sans_measurer,
        }
    }

    /// Get the appropriate text measurer based on font type
    fn get_measurer(&self) -> &'static text_metrics::BuiltinFontMeasurer {
        if self.use_sans_measurer {
            text_metrics::get_helvetica_measurer()
        } else {
            text_metrics::get_times_measurer()
        }
    }

    /// Calculate the height of the bidding table without rendering
    pub fn measure_height(&self, auction: &Auction) -> f32 {
        self.measure_height_with_players(auction, None, None)
    }

    /// Calculate the height of the bidding table with optional player names
    pub fn measure_height_with_players(
        &self,
        auction: &Auction,
        players: Option<&PlayerNames>,
        notes_max_width: Option<f32>,
    ) -> f32 {
        Self::measure_height_static(auction, players, self.settings, notes_max_width)
    }

    /// Calculate the height of the bidding table with all options (static version)
    /// This can be called without creating a renderer instance, useful for layout measurement
    pub fn measure_height_static(
        auction: &Auction,
        players: Option<&PlayerNames>,
        settings: &Settings,
        notes_max_width: Option<f32>,
    ) -> f32 {
        let row_height = settings.bid_row_height;
        let two_col_auctions = settings.two_col_auctions;

        let calls = &auction.calls;

        // Check if we should use two-column mode for this auction
        let uncontested_pair = if two_col_auctions {
            auction.uncontested_pair()
        } else {
            None
        };

        // Check if auction is passed out (exactly 4 passes, no bids)
        let is_passed_out = calls.len() == 4 && calls.iter().all(|a| a.call == Call::Pass);

        // Check if auction ends with 3+ unannotated passes after bidding (for "All Pass" rendering)
        // If any trailing pass has an annotation, show passes individually
        let trailing_passes = calls
            .iter()
            .rev()
            .take_while(|a| a.call == Call::Pass && a.annotation.is_none())
            .count();
        let show_all_pass = !is_passed_out && trailing_passes >= 3;
        let calls_to_render = if is_passed_out || show_all_pass {
            calls.len() - trailing_passes.min(calls.len())
        } else {
            calls.len()
        };

        // Start counting rows: spacing + header + optional player names row
        // Row 0 is spacing, Row 1 is header, Row 2 is player names (if present)
        // After counting, `row` will be one past the last content row
        let has_player_names = players.is_some_and(|p| p.has_any());
        let mut row = if has_player_names { 3 } else { 2 };

        // Handle passed out auction
        if is_passed_out {
            row += 1;
        } else if let Some((d1, d2)) = uncontested_pair {
            // Two-column mode: count only calls from the bidding pair
            let mut last_col: Option<usize> = None; // Track last column rendered
            let mut current_player = auction.dealer;

            for _ in calls.iter().take(calls_to_render) {
                if current_player == d1 || current_player == d2 {
                    // Determine which column (0 or 1) based on which player in the pair
                    let display_col = if current_player == d1 { 0 } else { 1 };

                    // Increment row when transitioning from column 1 to column 0
                    if last_col == Some(1) && display_col == 0 {
                        row += 1;
                    }

                    last_col = Some(display_col);
                }
                current_player = current_player.next();
            }

            // "All Pass" goes on next row after content
            if show_all_pass {
                row += 1; // Move to next row
                row += 1; // Row for "All Pass"
            } else if last_col.is_some() {
                // Account for the row we just counted
                row += 1;
            }
        } else {
            // Standard 4-column mode: count rows for regular calls
            let start_col = auction.dealer.table_position();
            let mut col = start_col;

            for _ in calls.iter().take(calls_to_render) {
                col += 1;
                if col >= 4 {
                    col = 0;
                    row += 1;
                }
            }

            // "All Pass" goes on next row after content
            if show_all_pass {
                if col > 0 {
                    row += 1; // Move past partial row
                }
                row += 1; // Row for "All Pass"
            } else if col > 0 {
                // Partial last row - move past it
                row += 1;
            }
            // If col == 0 and calls_to_render > 0, row was already incremented in the loop
        }

        // Calculate table height: extend to last row's text descenders, not a full row below
        // row is one past the last content row, so last baseline is at (row-1) * row_height
        let measurer = text_metrics::get_times_measurer();
        let descender = measurer.descender_mm(settings.body_font_size);
        let table_height = (row - 1) as f32 * row_height + descender;

        // Account for notes (with word wrapping if max_width specified)
        // Note: render_notes adds one line_height of spacing before the first note
        if !auction.notes.is_empty() {
            let note_font_size = settings.body_font_size; // Same font size as auction
            let note_line_height = note_font_size * 1.3 * 0.352778; // Convert pt to mm

            // Count note content lines (not including initial spacing)
            let note_content_lines = if let Some(max_w) = notes_max_width {
                // Calculate wrapped line count
                let measurer = text_metrics::get_times_measurer();
                let space_width = measurer.measure_width_mm(" ", note_font_size);
                let mut total_lines = 0;

                for (num, text) in &auction.notes {
                    // Convert suit escape codes for accurate width measurement
                    let converted_text = replace_suit_escapes(text);
                    let prefix = format!("{}. ", num);
                    let prefix_width = measurer.measure_width_mm(&prefix, note_font_size);
                    let available_width = max_w - prefix_width;

                    let words: Vec<&str> = converted_text.split_whitespace().collect();
                    if words.is_empty() {
                        total_lines += 1;
                        continue;
                    }

                    let mut line_count = 1;
                    let mut current_line_width = 0.0;

                    for word in words {
                        let word_width = measurer.measure_width_mm(word, note_font_size);

                        if current_line_width == 0.0 {
                            current_line_width = word_width;
                        } else if current_line_width + space_width + word_width <= available_width {
                            current_line_width += space_width + word_width;
                        } else {
                            line_count += 1;
                            current_line_width = word_width;
                        }
                    }
                    total_lines += line_count;
                }
                total_lines
            } else {
                // Original: one line per note
                auction.notes.len()
            };

            // Notes height = initial spacing line + note content lines
            // (matching render_notes which starts at oy.0 - line_height)
            let notes_height = (1 + note_content_lines) as f32 * note_line_height;

            // Return actual combined height (no rounding)
            table_height + notes_height
        } else {
            table_height
        }
    }

    /// Render the bidding table and return the height used
    pub fn render(&self, layer: &mut LayerBuilder, auction: &Auction, origin: (Mm, Mm)) -> f32 {
        self.render_with_options(layer, auction, origin, None, false, None)
    }

    /// Render the bidding table with optional player names and return the height used
    pub fn render_with_players(
        &self,
        layer: &mut LayerBuilder,
        auction: &Auction,
        origin: (Mm, Mm),
        players: Option<&PlayerNames>,
    ) -> f32 {
        self.render_with_options(
            layer,
            auction,
            origin,
            players,
            self.settings.two_col_auctions,
            None,
        )
    }

    /// Render the bidding table with optional player names and notes max width
    pub fn render_with_players_and_notes_width(
        &self,
        layer: &mut LayerBuilder,
        auction: &Auction,
        origin: (Mm, Mm),
        players: Option<&PlayerNames>,
        notes_max_width: Option<f32>,
    ) -> f32 {
        self.render_with_options(
            layer,
            auction,
            origin,
            players,
            self.settings.two_col_auctions,
            notes_max_width,
        )
    }

    /// Render the bidding table with all options
    pub fn render_with_options(
        &self,
        layer: &mut LayerBuilder,
        auction: &Auction,
        origin: (Mm, Mm),
        players: Option<&PlayerNames>,
        two_col_auctions: bool,
        notes_max_width: Option<f32>,
    ) -> f32 {
        let (ox, oy) = origin;
        let col_width = self.settings.bid_column_width;
        let row_height = self.settings.bid_row_height;

        // Check if we should use two-column mode for this auction
        let uncontested_pair = if two_col_auctions {
            auction.uncontested_pair()
        } else {
            None
        };

        // Render header row with spelled-out, italicized direction names
        layer.set_fill_color(Color::Rgb(BLACK));

        // Choose which directions to show in header
        let directions: &[Direction] = if let Some((d1, _d2)) = uncontested_pair {
            // Two-column mode: show only the bidding pair
            static WE: [Direction; 2] = [Direction::West, Direction::East];
            static NS: [Direction; 2] = [Direction::North, Direction::South];
            if d1 == Direction::West || d1 == Direction::East {
                &WE[..]
            } else {
                &NS[..]
            }
        } else {
            // Standard 4-column mode
            static ALL: [Direction; 4] = [
                Direction::West,
                Direction::North,
                Direction::East,
                Direction::South,
            ];
            &ALL[..]
        };

        // Add spacing before header row to separate from content above
        let header_y = Mm(oy.0 - row_height);

        for (i, dir) in directions.iter().enumerate() {
            let x = ox.0 + (i as f32 * col_width);
            layer.use_text_builtin(
                format!("{}", dir), // Use Display trait for full name
                self.settings.header_font_size,
                Mm(x),
                header_y,
                self.italic_font, // Use italic font for direction names (Bridge Composer style)
            );
        }

        // Render player names below direction headers if provided
        let has_player_names = players.is_some_and(|p| p.has_any());
        if has_player_names {
            let players = players.unwrap();
            let name_y = Mm(header_y.0 - row_height);

            for (i, dir) in directions.iter().enumerate() {
                if let Some(name) = players.get(*dir) {
                    if !name.is_empty() {
                        let x = ox.0 + (i as f32 * col_width);
                        layer.use_text_builtin(
                            name,
                            self.settings.header_font_size,
                            Mm(x),
                            name_y,
                            self.italic_font,
                        );
                    }
                }
            }
        }

        let calls = &auction.calls;

        // Check if auction is passed out (exactly 4 passes, no bids)
        let is_passed_out = calls.len() == 4 && calls.iter().all(|a| a.call == Call::Pass);

        // Check if auction ends with 3+ unannotated passes after bidding (for "All Pass" rendering)
        // If any trailing pass has an annotation, show passes individually
        let trailing_passes = calls
            .iter()
            .rev()
            .take_while(|a| a.call == Call::Pass && a.annotation.is_none())
            .count();
        let show_all_pass = !is_passed_out && trailing_passes >= 3;
        let calls_to_render = if is_passed_out || show_all_pass {
            calls.len() - trailing_passes.min(calls.len())
        } else {
            calls.len()
        };

        // Start row accounting for spacing + header + optional player names row
        // Row 0 is spacing, Row 1 is header, Row 2 is player names (if present)
        // After counting, `row` will be one past the last content row
        let mut row = if has_player_names { 3 } else { 2 };

        // Handle passed out auction
        if is_passed_out {
            // In two-column mode, show "Passed Out" at column 0
            let col = if uncontested_pair.is_some() {
                0
            } else {
                auction.dealer.table_position()
            };
            let x = ox.0 + (col as f32 * col_width);
            let y = oy.0 - (row as f32 * row_height);

            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(
                "Passed Out",
                self.settings.body_font_size,
                Mm(x),
                Mm(y),
                self.font,
            );
            row += 1;
        } else if let Some(pair) = uncontested_pair {
            // Two-column mode: only show the bidding pair's calls
            let (d1, d2) = pair;
            let mut last_col: Option<usize> = None; // Track last column rendered
            let mut current_player = auction.dealer;

            for annotated in calls.iter().take(calls_to_render) {
                // Only render calls from the bidding pair
                if current_player == d1 || current_player == d2 {
                    // Determine which column (0 or 1) based on which player in the pair
                    let display_col = if current_player == d1 { 0 } else { 1 };

                    // Increment row when transitioning from column 1 to column 0
                    // This handles the case where d2 opens (e.g., South opens 1NT)
                    // and we need to move to the next row before d1 responds
                    if last_col == Some(1) && display_col == 0 {
                        row += 1;
                    }

                    let x = ox.0 + (display_col as f32 * col_width);
                    let y = oy.0 - (row as f32 * row_height);

                    self.render_annotated_call(layer, annotated, (Mm(x), Mm(y)));

                    last_col = Some(display_col);
                }
                current_player = current_player.next();
            }

            // Render "All Pass" on next row if needed
            if show_all_pass {
                // Always move to next row for "All Pass"
                row += 1;
                let x = ox.0;
                let y = oy.0 - (row as f32 * row_height);

                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin(
                    "All Pass",
                    self.settings.body_font_size,
                    Mm(x),
                    Mm(y),
                    self.font,
                );
                row += 1;
            } else if last_col.is_some() {
                // Account for the row we just rendered to
                row += 1;
            }
        } else {
            // Standard 4-column mode
            let start_col = auction.dealer.table_position();
            let mut col = start_col;

            // Render regular calls (excluding trailing passes if we'll show "All Pass")
            for annotated in calls.iter().take(calls_to_render) {
                let x = ox.0 + (col as f32 * col_width);
                let y = oy.0 - (row as f32 * row_height);

                self.render_annotated_call(layer, annotated, (Mm(x), Mm(y)));

                col += 1;
                if col >= 4 {
                    col = 0;
                    row += 1;
                }
            }

            // Render "All Pass" on next row if needed
            if show_all_pass {
                // Move to next row if current row has content
                if col > 0 {
                    row += 1;
                }
                let x = ox.0;
                let y = oy.0 - (row as f32 * row_height);

                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin(
                    "All Pass",
                    self.settings.body_font_size,
                    Mm(x),
                    Mm(y),
                    self.font,
                );
                row += 1;
            } else if col > 0 {
                // Partial last row - move past it
                row += 1;
            }
            // If col == 0 and calls_to_render > 0, row was already incremented in the loop
        }

        // Calculate table height: extend to last row's text descenders, not a full row below
        // row is one past the last content row, so last baseline is at (row-1) * row_height
        let measurer = self.get_measurer();
        let descender = measurer.descender_mm(self.settings.body_font_size);
        let table_height = (row - 1) as f32 * row_height + descender;

        // Render notes if present and return combined height
        if !auction.notes.is_empty() {
            let notes_height = self.render_notes(
                layer,
                auction,
                (ox, Mm(oy.0 - table_height)),
                notes_max_width,
            );
            // Return actual combined height (no rounding)
            table_height + notes_height
        } else {
            table_height
        }
    }

    /// Render an annotated call (call with optional superscript annotation)
    fn render_annotated_call(
        &self,
        layer: &mut LayerBuilder,
        annotated: &AnnotatedCall,
        pos: (Mm, Mm),
    ) {
        let call_width = self.render_call(layer, &annotated.call, pos);

        // If there's an annotation, render it
        if let Some(ref annotation) = annotated.annotation {
            if annotated.call == Call::Blank {
                // For blanks, render annotation at normal size after the line
                let text_x = Mm(pos.0 .0 + call_width + 0.5);
                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin(
                    annotation,
                    self.settings.body_font_size,
                    text_x,
                    pos.1,
                    self.font,
                );
            } else if is_suffix_annotation(annotation) {
                // `!` and `?` belong to the call and follow it at full size
                // ("Pass?"), as BridgeComposer prints them; only note
                // references are raised
                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin(
                    annotation,
                    self.settings.body_font_size,
                    Mm(pos.0 .0 + call_width),
                    pos.1,
                    self.font,
                );
            } else {
                // Note references render as superscript
                let sup_x = Mm(pos.0 .0 + call_width);
                let sup_y =
                    Mm(pos.1 .0 + (self.settings.body_font_size * SUPERSCRIPT_RISE * 0.352778));
                let sup_size = self.settings.body_font_size * SUPERSCRIPT_RATIO;

                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin(annotation, sup_size, sup_x, sup_y, self.font);
            }
        }
    }

    /// Render a single call and return the width used
    fn render_call(&self, layer: &mut LayerBuilder, call: &Call, pos: (Mm, Mm)) -> f32 {
        let (x, y) = pos;
        let measurer = self.get_measurer();

        match call {
            Call::Pass => {
                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin("Pass", self.settings.body_font_size, x, y, self.font);
                measurer.measure_width_mm("Pass", self.settings.body_font_size)
            }
            Call::Double => {
                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin("Dbl", self.settings.body_font_size, x, y, self.font);
                measurer.measure_width_mm("Dbl", self.settings.body_font_size)
            }
            Call::Redouble => {
                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin("Rdbl", self.settings.body_font_size, x, y, self.font);
                measurer.measure_width_mm("Rdbl", self.settings.body_font_size)
            }
            Call::Bid {
                level,
                strain: suit,
            } => {
                // Render level
                layer.set_fill_color(Color::Rgb(BLACK));
                let level_str = level.to_string();
                layer.use_text_builtin(&level_str, self.settings.body_font_size, x, y, self.font);

                // Render suit symbol immediately after level (no gap)
                let level_width =
                    measurer.measure_width_mm(&level_str, self.settings.body_font_size);
                let suit_x = Mm(x.0 + level_width);
                let suit_width = self.render_bid_suit(layer, *suit, (suit_x, y));
                level_width + suit_width
            }
            Call::Continue => {
                // "+" in PBN becomes "?" in display (student fills in next bid)
                layer.set_fill_color(Color::Rgb(BLACK));
                layer.use_text_builtin("?", self.settings.body_font_size, x, y, self.font);
                measurer.measure_width_mm("?", self.settings.body_font_size)
            }
            Call::Blank => {
                // Underscore sequences in PBN become a horizontal line for students to write answers
                // Draw a line approximately 8mm wide at the text baseline
                let line_width = 8.0; // mm
                let line_thickness = 0.3; // mm
                let baseline_offset = self.settings.body_font_size * 0.08 * 0.352778; // Slightly below baseline

                layer.set_outline_color(Color::Rgb(BLACK));
                layer.set_outline_thickness(line_thickness);
                layer.add_line(
                    x,
                    Mm(y.0 - baseline_offset),
                    Mm(x.0 + line_width),
                    Mm(y.0 - baseline_offset),
                );
                line_width
            }
        }
    }

    /// Render notes below the bidding table with word wrapping
    fn render_notes(
        &self,
        layer: &mut LayerBuilder,
        auction: &Auction,
        origin: (Mm, Mm),
        max_width: Option<f32>,
    ) -> f32 {
        let (ox, oy) = origin;
        let note_font_size = self.settings.body_font_size; // Same font size as auction
        let line_height = note_font_size * 1.3 * 0.352778; // Convert pt to mm
        let measurer = self.get_measurer();

        // Get sorted note numbers
        let mut note_nums: Vec<&u8> = auction.notes.keys().collect();
        note_nums.sort();

        let mut current_y = oy.0 - line_height; // Start below the origin with some spacing

        for num in note_nums {
            if let Some(text) = auction.notes.get(num) {
                // Convert suit escape codes (\S, \H, \D, \C) to Unicode symbols
                let converted_text = replace_suit_escapes(text);
                let prefix = format!("{}. ", num);
                let prefix_width = measurer.measure_width_mm(&prefix, note_font_size);

                // If max_width is specified, wrap the text
                if let Some(max_w) = max_width {
                    let available_width = max_w - prefix_width;
                    let words: Vec<&str> = converted_text.split_whitespace().collect();

                    if words.is_empty() {
                        // Empty note - just render prefix
                        layer.set_fill_color(Color::Rgb(BLACK));
                        layer.use_text_builtin(
                            &prefix,
                            note_font_size,
                            ox,
                            Mm(current_y),
                            self.font,
                        );
                        current_y -= line_height;
                        continue;
                    }

                    // Build lines with word wrapping
                    let mut lines: Vec<String> = Vec::new();
                    let mut current_line = String::new();
                    let mut current_line_width = 0.0;
                    let space_width = measurer.measure_width_mm(" ", note_font_size);

                    for word in words {
                        let word_width = measurer.measure_width_mm(word, note_font_size);

                        if current_line.is_empty() {
                            // First word on line
                            current_line = word.to_string();
                            current_line_width = word_width;
                        } else if current_line_width + space_width + word_width <= available_width {
                            // Word fits on current line
                            current_line.push(' ');
                            current_line.push_str(word);
                            current_line_width += space_width + word_width;
                        } else {
                            // Word doesn't fit - start new line
                            lines.push(current_line);
                            current_line = word.to_string();
                            current_line_width = word_width;
                        }
                    }
                    // Don't forget the last line
                    if !current_line.is_empty() {
                        lines.push(current_line);
                    }

                    // Render lines
                    for (i, line) in lines.iter().enumerate() {
                        if i == 0 {
                            // First line: render prefix then text with suit symbols
                            layer.set_fill_color(Color::Rgb(BLACK));
                            layer.use_text_builtin(
                                &prefix,
                                note_font_size,
                                ox,
                                Mm(current_y),
                                self.font,
                            );
                            self.render_text_with_suits(
                                layer,
                                line,
                                note_font_size,
                                ox.0 + prefix_width,
                                current_y,
                            );
                        } else {
                            // Continuation lines are indented to align with first line's text
                            self.render_text_with_suits(
                                layer,
                                line,
                                note_font_size,
                                ox.0 + prefix_width,
                                current_y,
                            );
                        }
                        current_y -= line_height;
                    }
                } else {
                    // No max width - render as single line (original behavior)
                    // Render prefix then text with suit symbols
                    layer.set_fill_color(Color::Rgb(BLACK));
                    layer.use_text_builtin(&prefix, note_font_size, ox, Mm(current_y), self.font);
                    self.render_text_with_suits(
                        layer,
                        &converted_text,
                        note_font_size,
                        ox.0 + prefix_width,
                        current_y,
                    );
                    current_y -= line_height;
                }
            }
        }

        // Return total height used
        (oy.0 - current_y).max(0.0)
    }

    /// Render a bid suit symbol and return width used
    fn render_bid_suit(&self, layer: &mut LayerBuilder, suit: BidSuit, pos: (Mm, Mm)) -> f32 {
        let (x, y) = pos;
        let measurer = self.get_measurer();

        let (text, is_red, use_symbol_font) = match suit {
            BidSuit::Clubs => ("♣", false, true),
            BidSuit::Diamonds => ("♦", true, true),
            BidSuit::Hearts => ("♥", true, true),
            BidSuit::Spades => ("♠", false, true),
            BidSuit::NoTrump => ("NT", false, false),
        };

        if is_red {
            layer.set_fill_color(Color::Rgb(self.colors.hearts.clone()));
        } else {
            layer.set_fill_color(Color::Rgb(BLACK));
        }

        // Use symbol font for suit symbols, regular font for "NT"
        if use_symbol_font {
            layer.use_text(text, self.settings.body_font_size, x, y, self.symbol_font);
        } else {
            layer.use_text_builtin(text, self.settings.body_font_size, x, y, self.font);
        }

        // Measure width (use Helvetica for symbols, Times for NT)
        if use_symbol_font {
            let sans_measurer = text_metrics::get_helvetica_measurer();
            sans_measurer.measure_width_mm(text, self.settings.body_font_size)
        } else {
            measurer.measure_width_mm(text, self.settings.body_font_size)
        }
    }

    /// Render text that may contain suit symbols (♠♥♦♣)
    /// Renders regular text with builtin font and suit symbols with symbol font in correct colors
    fn render_text_with_suits(
        &self,
        layer: &mut LayerBuilder,
        text: &str,
        font_size: f32,
        x: f32,
        y: f32,
    ) -> f32 {
        let measurer = self.get_measurer();
        let sans_measurer = text_metrics::get_helvetica_measurer();
        let mut current_x = x;
        let mut buffer = String::new();

        for ch in text.chars() {
            match ch {
                '♠' | '♥' | '♦' | '♣' => {
                    // Flush any accumulated regular text
                    if !buffer.is_empty() {
                        layer.set_fill_color(Color::Rgb(BLACK));
                        layer.use_text_builtin(&buffer, font_size, Mm(current_x), Mm(y), self.font);
                        current_x += measurer.measure_width_mm(&buffer, font_size);
                        buffer.clear();
                    }

                    // Render suit symbol with appropriate color
                    let is_red = ch == '♥' || ch == '♦';
                    if is_red {
                        layer.set_fill_color(Color::Rgb(self.colors.hearts.clone()));
                    } else {
                        layer.set_fill_color(Color::Rgb(BLACK));
                    }

                    let symbol = ch.to_string();
                    layer.use_text(&symbol, font_size, Mm(current_x), Mm(y), self.symbol_font);
                    current_x += sans_measurer.measure_width_mm(&symbol, font_size);
                }
                _ => {
                    buffer.push(ch);
                }
            }
        }

        // Flush any remaining regular text
        if !buffer.is_empty() {
            layer.set_fill_color(Color::Rgb(BLACK));
            layer.use_text_builtin(&buffer, font_size, Mm(current_x), Mm(y), self.font);
            current_x += measurer.measure_width_mm(&buffer, font_size);
        }

        // Return total width used
        current_x - x
    }
}

#[cfg(test)]
mod tests {
    use super::is_suffix_annotation;

    #[test]
    fn suffix_markers_are_not_note_references() {
        for marker in ["!", "?", "!!", "??", "!?", "?!"] {
            assert!(is_suffix_annotation(marker), "{marker}");
        }
        for reference in ["1", "12", ""] {
            assert!(!is_suffix_annotation(reference), "{reference:?}");
        }
    }
}
