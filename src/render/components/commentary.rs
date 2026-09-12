use crate::config::Settings;
use crate::model::{CommentaryBlock, FormattedText, Suit, TextSpan};
use printpdf::{BuiltinFont, Color, FontId, Mm};

use crate::model::card::{Rank, RankExt};
use crate::render::helpers::colors::{SuitColors, BLACK};
use crate::render::helpers::layer::LayerBuilder;
use crate::render::helpers::text_metrics::{
    get_helvetica_bold_measurer, get_helvetica_measurer, get_times_bold_measurer,
    get_times_measurer, BuiltinFontMeasurer,
};

/// Check if a character is a Unicode suit symbol and return the corresponding Suit
fn suit_from_symbol(c: char) -> Option<Suit> {
    match c {
        '♠' => Some(Suit::Spades),
        '♥' => Some(Suit::Hearts),
        '♦' => Some(Suit::Diamonds),
        '♣' => Some(Suit::Clubs),
        _ => None,
    }
}

/// Parameters for floating layout
#[derive(Debug, Clone)]
pub struct FloatLayout {
    /// The bottom of the content the text floats beside: a line must lie wholly
    /// below this before it may run full width (page coordinates, so smaller =
    /// lower)
    pub float_until_y: f32,
    /// Left margin while floating (right side of page)
    pub float_left: f32,
    /// Max width while floating
    pub float_width: f32,
    /// Left margin after clearing float (full width)
    pub full_left: f32,
    /// Max width after clearing float
    pub full_width: f32,
}

impl FloatLayout {
    /// Whether a line on `baseline_y`, whose glyphs rise `ascent` above it, lies
    /// wholly below the floated-beside content and may run full width.
    ///
    /// Testing the baseline alone let a full-width line's upper half reach back
    /// into that content and print over the `Lead:` line beneath the auction
    /// (issue #29).
    pub fn clears(&self, baseline_y: f32, ascent: f32) -> bool {
        baseline_y + ascent < self.float_until_y
    }
}

/// Result of rendering with float layout
#[derive(Debug)]
pub struct FloatRenderResult {
    /// Total height used
    pub height: f32,
    /// Final Y position
    pub final_y: f32,
}

/// Renderer for commentary text
pub struct CommentaryRenderer<'a> {
    font: BuiltinFont,
    bold_font: BuiltinFont,
    italic_font: BuiltinFont,
    bold_italic_font: BuiltinFont,
    symbol_font: &'a FontId, // Font with Unicode suit symbols (DejaVu Sans)
    colors: SuitColors,
    settings: &'a Settings,
    use_sans_measurer: bool,
}

/// A fragment is an atomic piece of text with a specific style
#[derive(Debug, Clone)]
enum RenderFragment {
    Text {
        text: String,
        style: TextStyle,
        /// Optional foreground color override (from `<span style=color:...>`)
        color: Option<(u8, u8, u8)>,
    },
    SuitSymbol(Suit),
    CardRef {
        suit: Suit,
        rank: Rank,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextStyle {
    Plain,
    Bold,
    Italic,
    BoldItalic,
    Underline,
}

/// A word group is a sequence of fragments that should be kept together (no whitespace between them)
#[derive(Debug, Clone)]
struct WordGroup {
    fragments: Vec<RenderFragment>,
    width: f32,
}

/// A render token is either a word group or a space
#[derive(Debug, Clone)]
enum RenderToken {
    WordGroup(WordGroup),
    Space,
    LineBreak,
}

/// Check if a character is a valid card rank or placeholder in card lists.
/// Matches:
/// - Uppercase rank letters: A, K, Q, J, T
/// - Digits: 2-9
/// - Lowercase 'x' representing a low/unknown card
fn is_card_char(c: char) -> bool {
    c == 'x' || ((c.is_ascii_uppercase() || c.is_ascii_digit()) && Rank::from_char(c).is_some())
}

/// Check if a character is a valid card rank character for card lists in commentary.
/// Only matches uppercase letters (A, K, Q, J, T), digits (2-9), and lowercase 'x'.
/// Also checks that the character isn't followed by another letter (to avoid matching
/// "Joker" as J-oker).
fn is_rank_char_standalone(c: char, next_chars: &[char]) -> bool {
    // Must be a valid card character
    if !is_card_char(c) {
        return false;
    }

    // Check if followed by another letter (indicating it's part of a word like "Joker")
    // Look for the next non-whitespace character
    for &next in next_chars {
        if next.is_whitespace() {
            continue;
        }
        // If next non-whitespace is a letter that's NOT a card char, this isn't a standalone rank
        if next.is_alphabetic() && !is_card_char(next) {
            return false;
        }
        break;
    }

    true
}

/// Simple check for rank character (used for checking current word)
fn is_rank_char(c: char) -> bool {
    is_card_char(c)
}

/// Parse spans into render tokens, grouping fragments that should stay together
fn tokenize_spans(
    spans: &[TextSpan],
    font_size: f32,
    regular_measurer: &BuiltinFontMeasurer,
    bold_measurer: &BuiltinFontMeasurer,
    symbol_measurer: &BuiltinFontMeasurer,
) -> Vec<RenderToken> {
    let mut tokens: Vec<RenderToken> = Vec::new();
    let mut current_group: Vec<RenderFragment> = Vec::new();
    let mut current_group_width: f32 = 0.0;
    // Track if we're in a "card list" context (e.g., "♣K J 10")
    let mut in_card_list = false;

    // Helper to flush the current word group
    let flush_group = |tokens: &mut Vec<RenderToken>,
                       group: &mut Vec<RenderFragment>,
                       width: &mut f32,
                       in_card_list: &mut bool| {
        if !group.is_empty() {
            tokens.push(RenderToken::WordGroup(WordGroup {
                fragments: std::mem::take(group),
                width: *width,
            }));
            *width = 0.0;
        }
        *in_card_list = false;
    };

    for span in spans {
        match span {
            TextSpan::Plain(_)
            | TextSpan::Italic(_)
            | TextSpan::Bold(_)
            | TextSpan::BoldItalic(_)
            | TextSpan::Underline(_)
            | TextSpan::Colored { .. } => {
                let (s, style, color): (&str, TextStyle, Option<(u8, u8, u8)>) = match span {
                    TextSpan::Plain(s) => (s.as_str(), TextStyle::Plain, None),
                    TextSpan::Italic(s) => (s.as_str(), TextStyle::Italic, None),
                    TextSpan::Bold(s) => (s.as_str(), TextStyle::Bold, None),
                    TextSpan::BoldItalic(s) => (s.as_str(), TextStyle::BoldItalic, None),
                    TextSpan::Underline(s) => (s.as_str(), TextStyle::Underline, None),
                    TextSpan::Colored { text, italic, rgb } => (
                        text.as_str(),
                        if *italic {
                            TextStyle::Italic
                        } else {
                            TextStyle::Plain
                        },
                        Some(*rgb),
                    ),
                    _ => unreachable!(),
                };
                let measurer = match style {
                    TextStyle::Plain | TextStyle::Italic | TextStyle::Underline => regular_measurer,
                    TextStyle::Bold | TextStyle::BoldItalic => bold_measurer,
                };

                let chars: Vec<char> = s.chars().collect();
                let mut i = 0;
                let mut current_word = String::new();

                while i < chars.len() {
                    let c = chars[i];
                    if c.is_whitespace() {
                        // Flush any accumulated word fragment
                        if !current_word.is_empty() {
                            // Check if this word is a rank character to update card list state
                            let is_rank = current_word.len() == 1
                                && is_rank_char(current_word.chars().next().unwrap());
                            let w = measurer.measure_width_mm(&current_word, font_size);
                            current_group.push(RenderFragment::Text {
                                text: std::mem::take(&mut current_word),
                                style,
                                color,
                            });
                            current_group_width += w;
                            // Update card list state
                            if is_rank && in_card_list {
                                // Continue in card list mode
                            } else {
                                in_card_list = false;
                            }
                        }

                        // Check if we should stay in card list context
                        let should_stay_in_card_list = in_card_list && {
                            // Look ahead to see if the next non-space char is a standalone rank
                            let rest = &chars[i + 1..];
                            if let Some(pos) = rest.iter().position(|ch| !ch.is_whitespace()) {
                                // Pass the characters after this position to check if it's standalone
                                is_rank_char_standalone(rest[pos], &rest[pos + 1..])
                            } else {
                                false
                            }
                        };

                        if should_stay_in_card_list {
                            // Keep space in the group - add it as a text fragment
                            let space_w = regular_measurer.measure_width_mm(" ", font_size);
                            current_group.push(RenderFragment::Text {
                                text: " ".to_string(),
                                style,
                                color,
                            });
                            current_group_width += space_w;
                        } else {
                            // Normal case: flush the word group before the space
                            flush_group(
                                &mut tokens,
                                &mut current_group,
                                &mut current_group_width,
                                &mut in_card_list,
                            );
                            // Add space token
                            tokens.push(RenderToken::Space);
                        }
                    } else if let Some(suit) = suit_from_symbol(c) {
                        // Unicode suit symbol - handle specially for correct coloring
                        // First flush any accumulated word
                        if !current_word.is_empty() {
                            let w = measurer.measure_width_mm(&current_word, font_size);
                            current_group.push(RenderFragment::Text {
                                text: std::mem::take(&mut current_word),
                                style,
                                color,
                            });
                            current_group_width += w;
                        }
                        // Add suit symbol fragment
                        let symbol_w = symbol_measurer.measure_width_mm(&c.to_string(), font_size);
                        current_group.push(RenderFragment::SuitSymbol(suit));
                        current_group_width += symbol_w;
                        in_card_list = true;
                    } else {
                        current_word.push(c);
                    }
                    i += 1;
                }

                // Don't forget remaining characters in current_word
                if !current_word.is_empty() {
                    // Check if this is a rank character
                    let is_rank = current_word.len() == 1
                        && is_rank_char(current_word.chars().next().unwrap());
                    let w = measurer.measure_width_mm(&current_word, font_size);
                    current_group.push(RenderFragment::Text {
                        text: current_word,
                        style,
                        color,
                    });
                    current_group_width += w;
                    // Update card list state
                    if is_rank && in_card_list {
                        // Continue in card list mode
                    } else {
                        in_card_list = false;
                    }
                }
            }
            TextSpan::SuitSymbol(suit) => {
                let w = symbol_measurer.measure_width_mm(&suit.symbol().to_string(), font_size);
                current_group.push(RenderFragment::SuitSymbol(*suit));
                current_group_width += w;
                // Start card list mode
                in_card_list = true;
            }
            TextSpan::CardRef { suit, rank } => {
                let symbol_w =
                    symbol_measurer.measure_width_mm(&suit.symbol().to_string(), font_size);
                let rank_w = regular_measurer.measure_width_mm(rank.display_str(), font_size);
                current_group.push(RenderFragment::CardRef {
                    suit: *suit,
                    rank: *rank,
                });
                current_group_width += symbol_w + rank_w;
                // Start card list mode
                in_card_list = true;
            }
            TextSpan::LineBreak => {
                flush_group(
                    &mut tokens,
                    &mut current_group,
                    &mut current_group_width,
                    &mut in_card_list,
                );
                tokens.push(RenderToken::LineBreak);
            }
        }
    }

    // Flush any remaining group
    flush_group(
        &mut tokens,
        &mut current_group,
        &mut current_group_width,
        &mut in_card_list,
    );

    tokens
}

impl<'a> CommentaryRenderer<'a> {
    pub fn new(
        font: BuiltinFont,
        bold_font: BuiltinFont,
        italic_font: BuiltinFont,
        bold_italic_font: BuiltinFont,
        symbol_font: &'a FontId,
        settings: &'a Settings,
    ) -> Self {
        // Determine if we should use sans-serif measurement based on font settings
        let use_sans_measurer = settings
            .fonts
            .commentary
            .as_ref()
            .map(|f| f.is_sans_serif())
            .unwrap_or(false);

        Self {
            font,
            bold_font,
            italic_font,
            bold_italic_font,
            symbol_font,
            colors: SuitColors::new(settings.black_color, settings.red_color),
            settings,
            use_sans_measurer,
        }
    }

    /// Get the appropriate text measurer for regular text
    fn get_regular_measurer(&self) -> &'static BuiltinFontMeasurer {
        if self.use_sans_measurer {
            get_helvetica_measurer()
        } else {
            get_times_measurer()
        }
    }

    /// Get the appropriate text measurer for bold text
    fn get_bold_measurer(&self) -> &'static BuiltinFontMeasurer {
        if self.use_sans_measurer {
            get_helvetica_bold_measurer()
        } else {
            get_times_bold_measurer()
        }
    }

    /// How far a line of commentary rises above its baseline.
    pub fn line_ascent(&self) -> f32 {
        self.get_regular_measurer()
            .ascender_mm(self.settings.commentary_font_size)
    }

    /// Measure the height of a commentary block without rendering
    pub fn measure_height(&self, block: &CommentaryBlock, max_width: f32) -> f32 {
        self.measure_formatted_text_height(&block.content, max_width)
    }

    /// Measure formatted text height without rendering
    fn measure_formatted_text_height(&self, text: &FormattedText, max_width: f32) -> f32 {
        let font_size = self.settings.commentary_font_size;
        let line_height = self.settings.line_height;

        let regular_measurer = self.get_regular_measurer();
        let bold_measurer = self.get_bold_measurer();
        let symbol_measurer = get_helvetica_measurer();

        let base_space_width = regular_measurer.measure_width_mm(" ", font_size);

        // Tokenize the spans into word groups and spaces
        let tokens = tokenize_spans(
            &text.spans,
            font_size,
            regular_measurer,
            bold_measurer,
            symbol_measurer,
        );

        // Count lines by simulating the line-wrapping logic
        let mut token_idx = 0;
        let mut line_count = 0;

        while token_idx < tokens.len() {
            // Collect word groups for the current line
            let mut line_groups: Vec<&WordGroup> = Vec::new();
            let mut line_width: f32 = 0.0;
            let mut pending_spaces: usize = 0;

            while token_idx < tokens.len() {
                match &tokens[token_idx] {
                    RenderToken::WordGroup(group) => {
                        let space_needed = if line_groups.is_empty() {
                            0.0
                        } else {
                            base_space_width * pending_spaces.max(1) as f32
                        };
                        let new_width = line_width + space_needed + group.width;

                        if line_groups.is_empty() || new_width <= max_width {
                            line_groups.push(group);
                            line_width = new_width;
                            token_idx += 1;
                            pending_spaces = 0;
                        } else {
                            break;
                        }
                    }
                    RenderToken::Space => {
                        pending_spaces += 1;
                        token_idx += 1;
                    }
                    RenderToken::LineBreak => {
                        token_idx += 1;
                        break;
                    }
                }
            }

            // Count this line (even if empty due to consecutive line breaks)
            line_count += 1;
        }

        // Return total height: line_count * line_height, minus the extra spacing after last line
        // We only need descender space after the last line, not full line_height
        let descender_allowance = line_height * 0.3;
        (line_count as f32) * line_height - (line_height - descender_allowance)
    }

    /// Render a commentary block and return the height used
    pub fn render(
        &self,
        layer: &mut LayerBuilder,
        block: &CommentaryBlock,
        origin: (Mm, Mm),
        max_width: f32,
    ) -> f32 {
        self.render_formatted_text(layer, &block.content, origin, max_width, None)
            .height
    }

    /// Render a commentary block with floating layout
    /// Returns the result including final Y position for continuation
    pub fn render_float(
        &self,
        layer: &mut LayerBuilder,
        block: &CommentaryBlock,
        origin: (Mm, Mm),
        float_layout: &FloatLayout,
    ) -> FloatRenderResult {
        self.render_formatted_text(
            layer,
            &block.content,
            origin,
            float_layout.float_width,
            Some(float_layout),
        )
    }

    /// Render formatted text and return height used
    fn render_formatted_text(
        &self,
        layer: &mut LayerBuilder,
        text: &FormattedText,
        origin: (Mm, Mm),
        initial_max_width: f32,
        float_layout: Option<&FloatLayout>,
    ) -> FloatRenderResult {
        let (ox, oy) = origin;
        let font_size = self.settings.commentary_font_size;
        let line_height = self.settings.line_height;
        let justify = self.settings.justify;

        // Use appropriate measurers based on font type (sans vs serif)
        // Symbol font (DejaVu Sans) always uses sans measurer
        let regular_measurer = self.get_regular_measurer();
        let bold_measurer = self.get_bold_measurer();
        let symbol_measurer = get_helvetica_measurer();

        let base_space_width = regular_measurer.measure_width_mm(" ", font_size);

        // Track current layout state
        let mut current_line_start = ox.0;
        let mut max_width = initial_max_width;
        let mut in_float = float_layout.is_some();
        let mut y = oy.0;

        // Tokenize the spans into word groups and spaces
        let tokens = tokenize_spans(
            &text.spans,
            font_size,
            regular_measurer,
            bold_measurer,
            symbol_measurer,
        );

        // Process tokens and render lines on-the-fly
        // This allows us to handle width changes dynamically
        let mut token_idx = 0;

        while token_idx < tokens.len() {
            // Check if we've crossed the float boundary before starting a new line
            if in_float {
                if let Some(fl) = float_layout {
                    if fl.clears(y, regular_measurer.ascender_mm(font_size)) {
                        // Switch to full width layout
                        in_float = false;
                        current_line_start = fl.full_left;
                        max_width = fl.full_width;
                    }
                }
            }

            // Collect word groups for the current line using current max_width
            // Track (word_group, preceding_space_count) for each word
            let mut line_groups: Vec<(&WordGroup, usize)> = Vec::new();
            let mut line_width: f32 = 0.0;
            let mut is_paragraph_end = false;
            let mut pending_spaces: usize = 0;

            while token_idx < tokens.len() {
                match &tokens[token_idx] {
                    RenderToken::WordGroup(group) => {
                        // Calculate width if we add this word
                        let space_needed = if line_groups.is_empty() {
                            0.0
                        } else {
                            base_space_width * pending_spaces.max(1) as f32
                        };
                        let new_width = line_width + space_needed + group.width;

                        if line_groups.is_empty() || new_width <= max_width {
                            // Word fits on this line
                            line_groups.push((group, pending_spaces));
                            line_width = new_width;
                            token_idx += 1;
                            pending_spaces = 0;
                        } else {
                            // Word doesn't fit, break line here (don't consume this token)
                            break;
                        }
                    }
                    RenderToken::Space => {
                        // Count consecutive spaces
                        pending_spaces += 1;
                        token_idx += 1;
                    }
                    RenderToken::LineBreak => {
                        // Explicit line break - end the current line as a paragraph end
                        is_paragraph_end = true;
                        token_idx += 1;
                        break;
                    }
                }
            }

            // If we collected no words (e.g., multiple consecutive line breaks), just move to next line
            if line_groups.is_empty() {
                y -= line_height;
                continue;
            }

            // Check if this is the last line (paragraph end)
            if token_idx >= tokens.len() {
                is_paragraph_end = true;
            }

            // Calculate total space units needed (sum of all space counts between words)
            let total_space_units: usize = line_groups
                .iter()
                .skip(1)
                .map(|(_, count)| (*count).max(1))
                .sum();

            // Calculate space width for justification
            let space_width = if justify && !is_paragraph_end && total_space_units > 0 {
                // Calculate total content width (word groups only, no spaces)
                let total_word_width: f32 = line_groups.iter().map(|(g, _)| g.width).sum();
                // Available space for distribution (divided by total space units)
                let available_space = max_width - total_word_width;
                // Each space unit gets this width
                available_space / total_space_units as f32
            } else {
                // Use base space width for paragraph-ending lines or when not justifying
                base_space_width
            };

            // Render the line
            let mut x = current_line_start;

            // Track underline spans to draw them continuously (including spaces)
            let mut underline_start_x: Option<f32> = None;
            let underline_y = y - 0.5; // Slightly below baseline

            // Helper to check if a fragment is underlined
            let is_underlined = |frag: &RenderFragment| -> bool {
                matches!(
                    frag,
                    RenderFragment::Text {
                        style: TextStyle::Underline,
                        ..
                    }
                )
            };

            // Helper to draw underline if active
            let draw_underline = |layer: &mut LayerBuilder, start: Option<f32>, end: f32| {
                if let Some(start_x) = start {
                    layer.set_outline_color(Color::Rgb(BLACK));
                    layer.set_outline_thickness(0.3);
                    layer.add_line(Mm(start_x), Mm(underline_y), Mm(end), Mm(underline_y));
                }
            };

            for (i, (group, space_count)) in line_groups.iter().enumerate() {
                // Check if this group starts with underlined content
                let group_starts_underlined =
                    group.fragments.first().map(is_underlined).unwrap_or(false);

                // Add space before word (except first word)
                if i > 0 {
                    let num_spaces = (*space_count).max(1);
                    let space_advance = space_width * num_spaces as f32;

                    // If we're continuing an underline into this group, include the space
                    // If we're ending an underline (next group not underlined), draw it before space
                    if underline_start_x.is_some() && !group_starts_underlined {
                        draw_underline(layer, underline_start_x, x);
                        underline_start_x = None;
                    }

                    x += space_advance;

                    // If starting underline at this group after space, start tracking from here
                    if underline_start_x.is_none() && group_starts_underlined {
                        underline_start_x = Some(x);
                    }
                }

                // Render all fragments in the group
                for fragment in &group.fragments {
                    match fragment {
                        RenderFragment::Text {
                            text: txt,
                            style,
                            color,
                        } => {
                            let font = match style {
                                TextStyle::Plain | TextStyle::Underline => self.font,
                                TextStyle::Bold => self.bold_font,
                                TextStyle::Italic => self.italic_font,
                                TextStyle::BoldItalic => self.bold_italic_font,
                            };
                            let measurer = match style {
                                TextStyle::Plain | TextStyle::Italic | TextStyle::Underline => {
                                    &regular_measurer
                                }
                                TextStyle::Bold | TextStyle::BoldItalic => &bold_measurer,
                            };
                            let width = measurer.measure_width_mm(txt, font_size);

                            // Check underline state transitions
                            let is_underline = *style == TextStyle::Underline;
                            if is_underline && underline_start_x.is_none() {
                                underline_start_x = Some(x);
                            } else if !is_underline && underline_start_x.is_some() {
                                draw_underline(layer, underline_start_x, x);
                                underline_start_x = None;
                            }

                            let fill = match color {
                                Some((r, g, b)) => printpdf::Rgb {
                                    r: *r as f32 / 255.0,
                                    g: *g as f32 / 255.0,
                                    b: *b as f32 / 255.0,
                                    icc_profile: None,
                                },
                                None => BLACK,
                            };
                            layer.set_fill_color(Color::Rgb(fill));
                            layer.use_text_builtin(txt, font_size, Mm(x), Mm(y), font);

                            x += width;
                        }
                        RenderFragment::SuitSymbol(suit) => {
                            // Suit symbols break underline spans
                            if underline_start_x.is_some() {
                                draw_underline(layer, underline_start_x, x);
                                underline_start_x = None;
                            }

                            let symbol = suit.symbol().to_string();
                            let width = symbol_measurer.measure_width_mm(&symbol, font_size);

                            let color = self.colors.for_suit(suit);
                            layer.set_fill_color(Color::Rgb(color));
                            layer.use_text(&symbol, font_size, Mm(x), Mm(y), self.symbol_font);
                            x += width;
                        }
                        RenderFragment::CardRef { suit, rank } => {
                            // Card refs break underline spans
                            if underline_start_x.is_some() {
                                draw_underline(layer, underline_start_x, x);
                                underline_start_x = None;
                            }

                            let symbol = suit.symbol().to_string();
                            let symbol_width = symbol_measurer.measure_width_mm(&symbol, font_size);
                            let rank_str = rank.display_str().to_string();
                            let rank_width =
                                regular_measurer.measure_width_mm(&rank_str, font_size);

                            // Render suit symbol with color
                            let color = self.colors.for_suit(suit);
                            layer.set_fill_color(Color::Rgb(color));
                            layer.use_text(&symbol, font_size, Mm(x), Mm(y), self.symbol_font);
                            x += symbol_width;

                            // Render rank in black
                            layer.set_fill_color(Color::Rgb(BLACK));
                            layer.use_text_builtin(&rank_str, font_size, Mm(x), Mm(y), self.font);
                            x += rank_width;
                        }
                    }
                }
            }

            // Draw any remaining underline at end of line
            if underline_start_x.is_some() {
                draw_underline(layer, underline_start_x, x);
            }

            // Move to next line
            y -= line_height;
        }

        // Return total height used and final Y position
        // Height is the advancement from first baseline to last baseline: (N-1)*line_height.
        // The caller handles positioning the first baseline (cap_height below start_y),
        // and descenders on the last line hang below into inter-board spacing.
        // This keeps centering based on the main body of text rather than descenders.
        FloatRenderResult {
            height: ((oy.0 - y) - line_height).max(0.0),
            final_y: y + line_height,
        }
    }
}
