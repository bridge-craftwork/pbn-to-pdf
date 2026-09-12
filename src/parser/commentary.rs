use crate::model::{CommentaryBlock, FormattedText, Rank, Suit, TextSpan};

/// Parse commentary text from PBN, handling formatting codes
/// Commentary is enclosed in braces: { ... }
/// Supports: <b>bold</b>, <i>italic</i>, \S \H \D \C for suits
pub fn parse_commentary(input: &str) -> Result<CommentaryBlock, String> {
    let content = parse_formatted_text(input)?;
    Ok(CommentaryBlock::new(content))
}

/// Replace suit escape sequences (\S, \H, \D, \C) with Unicode symbols.
/// Used to process text where we need to convert backslash codes to symbols.
pub fn replace_suit_escapes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                match next {
                    'S' | 's' => {
                        result.push('♠');
                        chars.next();
                    }
                    'H' | 'h' => {
                        result.push('♥');
                        chars.next();
                    }
                    'D' | 'd' => {
                        result.push('♦');
                        chars.next();
                    }
                    'C' | 'c' => {
                        result.push('♣');
                        chars.next();
                    }
                    _ => result.push(c),
                }
            } else {
                result.push(c);
            }
        } else if c == '!' && matches!(chars.peek(), Some('S' | 'H' | 'D' | 'C')) {
            // BBO's form. Unlike the backslash form it is uppercase only, and
            // in the corpus it is always a suit -- `3!SX`, `!DK` and `K!H`
            // included -- so it converts without a word-boundary test, which
            // would miss exactly those.
            result.push(match chars.next() {
                Some('S') => '♠',
                Some('H') => '♥',
                Some('D') => '♦',
                _ => '♣',
            });
        } else {
            result.push(c);
        }
    }

    result
}

/// Parse italic content that contains nested `<u>` tags.
/// Splits into italic spans (for non-underlined parts) and underline spans.
/// e.g., "1\H–<u>Pass</u> (0–5 points)" becomes:
///   Italic("1♥–"), Underline("Pass"), Italic(" (0–5 points)")
fn parse_italic_with_nested_underline(content: &str, text: &mut FormattedText) {
    let mut remaining = content;
    while let Some(u_start) = remaining.find("<u>") {
        // Push italic text before the <u> tag
        let before = &remaining[..u_start];
        if !before.is_empty() {
            text.push(TextSpan::italic(replace_suit_escapes(before)));
        }

        // Find closing </u>
        let after_tag = &remaining[u_start + 3..];
        if let Some(u_end) = after_tag.find("</u>") {
            let underline_content = &after_tag[..u_end];
            text.push(TextSpan::underline(replace_suit_escapes(underline_content)));
            remaining = &after_tag[u_end + 4..];
        } else {
            // Unclosed <u> tag — treat rest as italic
            text.push(TextSpan::italic(replace_suit_escapes(
                &remaining[u_start..],
            )));
            return;
        }
    }
    // Push any remaining italic text after the last </u>
    if !remaining.is_empty() {
        text.push(TextSpan::italic(replace_suit_escapes(remaining)));
    }
}

/// Parse italic content that contains nested `<b>` tags.
/// Splits into italic spans and bold_italic spans.
/// e.g., " and <b>1-5</b> are 2/1 GF auctions." becomes:
///   Italic(" and "), BoldItalic("1-5"), Italic(" are 2/1 GF auctions.")
fn parse_italic_with_nested_bold(content: &str, text: &mut FormattedText) {
    let mut remaining = content;
    while let Some(b_start) = remaining.find("<b>") {
        // Push italic text before the <b> tag
        let before = &remaining[..b_start];
        if !before.is_empty() {
            text.push(TextSpan::italic(replace_suit_escapes(before)));
        }

        // Find closing </b>
        let after_tag = &remaining[b_start + 3..];
        if let Some(b_end) = after_tag.find("</b>") {
            let bold_content = &after_tag[..b_end];
            text.push(TextSpan::bold_italic(replace_suit_escapes(bold_content)));
            remaining = &after_tag[b_end + 4..];
        } else {
            // Unclosed <b> tag — treat rest as italic
            text.push(TextSpan::italic(replace_suit_escapes(
                &remaining[b_start..],
            )));
            return;
        }
    }
    // Push any remaining italic text after the last </b>
    if !remaining.is_empty() {
        text.push(TextSpan::italic(replace_suit_escapes(remaining)));
    }
}

/// Parse a `<span style=color:HEX>` opening tag.
/// Returns `Some((rgb, byte_offset_past_closing_angle_bracket))` if matched, else `None`.
///
/// Accepts both shorthand (#RGB) and full (#RRGGBB) hex colors, with optional
/// quotes around the style value and tolerant of extra whitespace.
fn parse_span_open(remaining: &str) -> Option<((u8, u8, u8), usize)> {
    if !remaining.starts_with("<span") {
        return None;
    }
    // Find the closing '>' of the opening tag
    let close_offset = remaining.find('>')?;
    let tag = &remaining[..close_offset];
    // Look for `color:` (case-insensitive prefix not required for our PBN inputs).
    let lower = tag.to_ascii_lowercase();
    let color_idx = lower.find("color:")?;
    let hex_start = color_idx + "color:".len();
    let after_prefix = &tag[hex_start..];
    // Skip a leading '#' if present
    let trimmed = after_prefix.trim_start();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    // Take hex digits only (stops at quote, semicolon, whitespace, or '>')
    let hex_digits: String = hex.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    let rgb = parse_hex_color(&hex_digits)?;
    Some((rgb, close_offset + 1))
}

/// Parse a CSS-style hex color (`#RGB` or `#RRGGBB`, with leading `#` already stripped).
fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8)> {
    match hex.len() {
        3 => {
            // #RGB -> each digit doubled
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some((r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

/// Parse italic content that contains a nested `<span style=color:...>` tag.
/// Splits into italic spans and italic-colored spans.
fn parse_italic_with_nested_span(content: &str, text: &mut FormattedText) {
    let mut remaining = content;
    while let Some(span_start) = remaining.find("<span") {
        // Push italic text before the <span> tag
        let before = &remaining[..span_start];
        if !before.is_empty() {
            text.push(TextSpan::italic(replace_suit_escapes(before)));
        }

        // Parse the span opening
        let after_open = &remaining[span_start..];
        let (rgb, open_len) = match parse_span_open(after_open) {
            Some(v) => v,
            None => {
                // Malformed span — treat the rest as italic and bail
                text.push(TextSpan::italic(replace_suit_escapes(after_open)));
                return;
            }
        };
        let body_and_rest = &after_open[open_len..];
        // Find the closing </span>
        if let Some(end_idx) = body_and_rest.find("</span>") {
            let body = &body_and_rest[..end_idx];
            text.push(TextSpan::italic_colored(replace_suit_escapes(body), rgb));
            remaining = &body_and_rest[end_idx + "</span>".len()..];
        } else {
            // Unclosed </span> — treat the rest as italic-colored
            text.push(TextSpan::italic_colored(
                replace_suit_escapes(body_and_rest),
                rgb,
            ));
            return;
        }
    }
    if !remaining.is_empty() {
        text.push(TextSpan::italic(replace_suit_escapes(remaining)));
    }
}

/// Strip empty or whitespace-only italic tags like `<i> </i>` or `<i></i>`.
/// These are sometimes used in PBN files for formatting around punctuation
/// (e.g., em-dashes) and would otherwise appear as raw tags in output.
fn strip_empty_italic_tags(input: &str) -> String {
    let mut result = input.to_string();
    // Keep stripping until no more matches (handles multiple occurrences)
    loop {
        let before = result.clone();
        // Match <i> followed by optional whitespace and </i>
        if let Some(start) = result.find("<i>") {
            if let Some(end_tag_start) = result[start..].find("</i>") {
                let content = &result[start + 3..start + end_tag_start];
                // If content is empty or whitespace-only, strip the whole tag
                if content.trim().is_empty() {
                    // Replace with just the whitespace content (preserve spacing)
                    result = format!(
                        "{}{}{}",
                        &result[..start],
                        content,
                        &result[start + end_tag_start + 4..]
                    );
                }
            }
        }
        if result == before {
            break;
        }
    }
    result
}

/// Parse formatted text with HTML-like tags and suit symbols
pub fn parse_formatted_text(input: &str) -> Result<FormattedText, String> {
    // Pre-process: strip empty or whitespace-only italic tags like <i> </i>
    // These are sometimes used in PBN files for formatting around punctuation
    let input = strip_empty_italic_tags(input);
    // BBO's `!S` is the backslash form in other clothes; rewriting it here lets
    // every span type, and card references like `!SK`, handle it the same way
    let input = input
        .replace("!S", "\\S")
        .replace("!H", "\\H")
        .replace("!D", "\\D")
        .replace("!C", "\\C");

    let mut text = FormattedText::new();
    let mut remaining = input.as_str();
    let mut plain_buffer = String::new();

    while !remaining.is_empty() {
        if remaining.starts_with("<b>") {
            // Flush plain buffer
            if !plain_buffer.is_empty() {
                text.push(TextSpan::plain(std::mem::take(&mut plain_buffer)));
            }

            // Find closing tag
            let end = remaining.find("</b>").ok_or("Unclosed <b> tag")?;
            let bold_content = &remaining[3..end];

            // Check for nested <i> tag inside bold
            if bold_content.starts_with("<i>") && bold_content.ends_with("</i>") {
                // Extract the content inside both tags
                let inner = &bold_content[3..bold_content.len() - 4];
                text.push(TextSpan::bold_italic(replace_suit_escapes(inner)));
            } else {
                text.push(TextSpan::bold(replace_suit_escapes(bold_content)));
            }
            remaining = &remaining[end + 4..];
        } else if remaining.starts_with("<i>") {
            // Flush plain buffer
            if !plain_buffer.is_empty() {
                text.push(TextSpan::plain(std::mem::take(&mut plain_buffer)));
            }

            // Find closing tag
            let end = remaining.find("</i>").ok_or("Unclosed <i> tag")?;
            let italic_content = &remaining[3..end];

            // Check for nested tags inside italic
            if italic_content.starts_with("<b>") && italic_content.ends_with("</b>") {
                // Nested <b> wrapping entire <i> content -> bold_italic
                let inner = &italic_content[3..italic_content.len() - 4];
                text.push(TextSpan::bold_italic(replace_suit_escapes(inner)));
            } else if italic_content.contains("<b>") {
                // Nested <b> tags inside <i>: split into italic and bold_italic spans
                parse_italic_with_nested_bold(italic_content, &mut text);
            } else if italic_content.contains("<u>") {
                // Nested <u> tags inside <i>: split into italic and underline spans
                parse_italic_with_nested_underline(italic_content, &mut text);
            } else if italic_content.contains("<span") {
                // Nested <span> color tags inside <i>: split into italic and italic-colored spans
                parse_italic_with_nested_span(italic_content, &mut text);
            } else {
                text.push(TextSpan::italic(replace_suit_escapes(italic_content)));
            }
            remaining = &remaining[end + 4..];
        } else if remaining.starts_with("<u>") {
            // Flush plain buffer
            if !plain_buffer.is_empty() {
                text.push(TextSpan::plain(std::mem::take(&mut plain_buffer)));
            }

            // Find closing tag
            let end = remaining.find("</u>").ok_or("Unclosed <u> tag")?;
            let underline_content = &remaining[3..end];
            text.push(TextSpan::underline(replace_suit_escapes(underline_content)));
            remaining = &remaining[end + 4..];
        } else if remaining.starts_with("<span") {
            // Color span: <span style=color:HEX>...</span>
            if let Some((rgb, open_len)) = parse_span_open(remaining) {
                // Flush plain buffer
                if !plain_buffer.is_empty() {
                    text.push(TextSpan::plain(std::mem::take(&mut plain_buffer)));
                }
                let body_and_rest = &remaining[open_len..];
                if let Some(end_idx) = body_and_rest.find("</span>") {
                    let body = &body_and_rest[..end_idx];
                    text.push(TextSpan::colored(replace_suit_escapes(body), rgb));
                    remaining = &body_and_rest[end_idx + "</span>".len()..];
                } else {
                    // Unclosed span — treat rest as colored
                    text.push(TextSpan::colored(replace_suit_escapes(body_and_rest), rgb));
                    remaining = "";
                }
            } else {
                // Malformed <span ...> — fall through and consume one char
                let c = remaining.chars().next().unwrap();
                plain_buffer.push(c);
                remaining = &remaining[c.len_utf8()..];
            }
        } else if remaining.starts_with('\\') && remaining.len() >= 2 {
            // Check for suit symbol escape
            let next_char = remaining.chars().nth(1).unwrap();

            match next_char {
                'S' | 's' => {
                    // Flush plain buffer
                    if !plain_buffer.is_empty() {
                        text.push(TextSpan::plain(std::mem::take(&mut plain_buffer)));
                    }

                    // Check if followed by a rank (card reference)
                    if remaining.len() >= 3 {
                        let rank_char = remaining.chars().nth(2).unwrap();
                        if let Some(rank) = Rank::from_char(rank_char) {
                            text.push(TextSpan::CardRef {
                                suit: Suit::Spades,
                                rank,
                            });
                            remaining = &remaining[3..];
                            continue;
                        }
                    }
                    text.push(TextSpan::SuitSymbol(Suit::Spades));
                    remaining = &remaining[2..];
                }
                'H' | 'h' => {
                    if !plain_buffer.is_empty() {
                        text.push(TextSpan::plain(std::mem::take(&mut plain_buffer)));
                    }

                    if remaining.len() >= 3 {
                        let rank_char = remaining.chars().nth(2).unwrap();
                        if let Some(rank) = Rank::from_char(rank_char) {
                            text.push(TextSpan::CardRef {
                                suit: Suit::Hearts,
                                rank,
                            });
                            remaining = &remaining[3..];
                            continue;
                        }
                    }
                    text.push(TextSpan::SuitSymbol(Suit::Hearts));
                    remaining = &remaining[2..];
                }
                'D' | 'd' => {
                    if !plain_buffer.is_empty() {
                        text.push(TextSpan::plain(std::mem::take(&mut plain_buffer)));
                    }

                    if remaining.len() >= 3 {
                        let rank_char = remaining.chars().nth(2).unwrap();
                        if let Some(rank) = Rank::from_char(rank_char) {
                            text.push(TextSpan::CardRef {
                                suit: Suit::Diamonds,
                                rank,
                            });
                            remaining = &remaining[3..];
                            continue;
                        }
                    }
                    text.push(TextSpan::SuitSymbol(Suit::Diamonds));
                    remaining = &remaining[2..];
                }
                'C' | 'c' => {
                    if !plain_buffer.is_empty() {
                        text.push(TextSpan::plain(std::mem::take(&mut plain_buffer)));
                    }

                    if remaining.len() >= 3 {
                        let rank_char = remaining.chars().nth(2).unwrap();
                        if let Some(rank) = Rank::from_char(rank_char) {
                            text.push(TextSpan::CardRef {
                                suit: Suit::Clubs,
                                rank,
                            });
                            remaining = &remaining[3..];
                            continue;
                        }
                    }
                    text.push(TextSpan::SuitSymbol(Suit::Clubs));
                    remaining = &remaining[2..];
                }
                'n' => {
                    // Newline
                    if !plain_buffer.is_empty() {
                        text.push(TextSpan::plain(std::mem::take(&mut plain_buffer)));
                    }
                    text.push(TextSpan::LineBreak);
                    remaining = &remaining[2..];
                }
                _ => {
                    // Unknown escape, keep as-is
                    plain_buffer.push('\\');
                    plain_buffer.push(next_char);
                    remaining = &remaining[2..];
                }
            }
        } else if remaining.starts_with("\n\n") || remaining.starts_with("\r\n\r\n") {
            // Blank line = paragraph break (double line break for spacing)
            if !plain_buffer.is_empty() {
                text.push(TextSpan::plain(std::mem::take(&mut plain_buffer)));
            }
            text.push(TextSpan::LineBreak);
            text.push(TextSpan::LineBreak);
            if remaining.starts_with("\r\n\r\n") {
                remaining = &remaining[4..];
            } else {
                remaining = &remaining[2..];
            }
        } else if remaining.starts_with('\n') || remaining.starts_with("\r\n") {
            // Single newline = soft line break (treat as space for word wrapping)
            // Always add a space unless the buffer already ends with one.
            // If the buffer is empty but we've pushed non-text spans (like CardRef),
            // we still need the space to separate from the next word.
            if plain_buffer.is_empty() || !plain_buffer.ends_with(' ') {
                plain_buffer.push(' ');
            }
            if remaining.starts_with("\r\n") {
                remaining = &remaining[2..];
            } else {
                remaining = &remaining[1..];
            }
        } else {
            // Regular character
            let c = remaining.chars().next().unwrap();
            plain_buffer.push(c);
            remaining = &remaining[c.len_utf8()..];
        }
    }

    // Flush remaining plain buffer
    if !plain_buffer.is_empty() {
        text.push(TextSpan::plain(plain_buffer));
    }

    Ok(text)
}

/// Extract commentary block from braces
pub fn extract_commentary(input: &str) -> Option<(&str, &str)> {
    let start = input.find('{')?;
    let end = input.rfind('}')?;

    if end > start {
        Some((&input[start + 1..end], &input[end + 1..]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        let text = parse_formatted_text("Hello world").unwrap();
        assert_eq!(text.spans.len(), 1);
        assert_eq!(text.spans[0], TextSpan::Plain("Hello world".to_string()));
    }

    #[test]
    fn test_bold_text() {
        let text = parse_formatted_text("<b>Bold</b> text").unwrap();
        assert_eq!(text.spans.len(), 2);
        assert_eq!(text.spans[0], TextSpan::Bold("Bold".to_string()));
        assert_eq!(text.spans[1], TextSpan::Plain(" text".to_string()));
    }

    #[test]
    fn test_suit_symbols() {
        let text = parse_formatted_text(r"Open 1\S").unwrap();
        assert_eq!(text.spans.len(), 2);
        assert_eq!(text.spans[0], TextSpan::Plain("Open 1".to_string()));
        assert_eq!(text.spans[1], TextSpan::SuitSymbol(Suit::Spades));
    }

    #[test]
    fn test_card_reference() {
        let text = parse_formatted_text(r"Lead the \SQ").unwrap();
        assert_eq!(text.spans.len(), 2);
        assert_eq!(text.spans[0], TextSpan::Plain("Lead the ".to_string()));
        assert_eq!(
            text.spans[1],
            TextSpan::CardRef {
                suit: Suit::Spades,
                rank: Rank::Queen
            }
        );
    }

    #[test]
    fn test_mixed_formatting() {
        let text = parse_formatted_text(r"<b>Bidding.</b> Open 1\D and rebid 2\H").unwrap();
        assert_eq!(text.to_plain_text(), "Bidding. Open 1♦ and rebid 2♥");
    }

    #[test]
    fn test_extract_commentary() {
        let input = "{This is commentary} rest of line";
        let (commentary, rest) = extract_commentary(input).unwrap();
        assert_eq!(commentary, "This is commentary");
        assert_eq!(rest, " rest of line");
    }

    #[test]
    fn test_bold_with_spaces() {
        // Test "Declarer play." in bold followed by plain text
        let text = parse_formatted_text("<b>Declarer play.</b> Declarer (North)").unwrap();
        println!("Spans:");
        for (i, span) in text.spans.iter().enumerate() {
            println!("  {}: {:?}", i, span);
        }
        assert_eq!(text.spans.len(), 2);
        assert_eq!(text.spans[0], TextSpan::Bold("Declarer play.".to_string()));
        assert_eq!(
            text.spans[1],
            TextSpan::Plain(" Declarer (North)".to_string())
        );
    }

    #[test]
    fn test_bold_italic_nested() {
        // Test nested <b><i>...</i></b> produces BoldItalic
        let text = parse_formatted_text("<b><i>Exercise One Answers</i></b>").unwrap();
        assert_eq!(text.spans.len(), 1);
        assert_eq!(
            text.spans[0],
            TextSpan::BoldItalic("Exercise One Answers".to_string())
        );
    }

    #[test]
    fn test_italic_bold_nested() {
        // Test nested <i><b>...</b></i> also produces BoldItalic
        let text = parse_formatted_text("<i><b>Also Bold Italic</b></i>").unwrap();
        assert_eq!(text.spans.len(), 1);
        assert_eq!(
            text.spans[0],
            TextSpan::BoldItalic("Also Bold Italic".to_string())
        );
    }

    #[test]
    fn test_suit_symbol_in_italic() {
        // Test suit symbols inside italic text are converted to Unicode
        let text =
            parse_formatted_text(r"<i>Opener would have rebid 1\S with four spades.</i>").unwrap();
        assert_eq!(text.spans.len(), 1);
        assert_eq!(
            text.spans[0],
            TextSpan::Italic("Opener would have rebid 1♠ with four spades.".to_string())
        );
    }

    #[test]
    fn test_suit_symbol_in_bold() {
        // Test suit symbols inside bold text are converted to Unicode
        let text = parse_formatted_text(r"<b>Open 1\D</b>").unwrap();
        assert_eq!(text.spans.len(), 1);
        assert_eq!(text.spans[0], TextSpan::Bold("Open 1♦".to_string()));
    }

    #[test]
    fn test_suit_symbol_in_bold_italic() {
        // Test suit symbols inside bold-italic text are converted to Unicode
        let text = parse_formatted_text(r"<b><i>Bid 2\H</i></b>").unwrap();
        assert_eq!(text.spans.len(), 1);
        assert_eq!(text.spans[0], TextSpan::BoldItalic("Bid 2♥".to_string()));
    }

    #[test]
    fn test_underline_text() {
        let text = parse_formatted_text("<u>Underlined text</u>").unwrap();
        assert_eq!(text.spans.len(), 1);
        assert_eq!(
            text.spans[0],
            TextSpan::Underline("Underlined text".to_string())
        );
    }

    #[test]
    fn test_underline_with_suit_symbol() {
        let text = parse_formatted_text(r"<u>Lead the \SQ</u>").unwrap();
        assert_eq!(text.spans.len(), 1);
        assert_eq!(
            text.spans[0],
            TextSpan::Underline("Lead the ♠Q".to_string())
        );
    }

    #[test]
    fn test_strip_empty_italic_tags() {
        // Test case from real PBN: <b>Exercise One<i> </i>—<i> </i>Ruffing Losers</b>
        // The <i> </i> tags around the em-dash should be stripped, preserving the spaces
        let text =
            parse_formatted_text("<b>Exercise One<i> </i>—<i> </i>Ruffing Losers</b>").unwrap();
        assert_eq!(text.spans.len(), 1);
        assert_eq!(
            text.spans[0],
            TextSpan::Bold("Exercise One — Ruffing Losers".to_string())
        );
    }

    #[test]
    fn test_colored_span_plain() {
        let text = parse_formatted_text(r#"Click <span style=color:#00f>here</span> now"#).unwrap();
        assert_eq!(text.spans.len(), 3);
        assert_eq!(text.spans[0], TextSpan::Plain("Click ".to_string()));
        assert_eq!(
            text.spans[1],
            TextSpan::Colored {
                text: "here".to_string(),
                italic: false,
                rgb: (0, 0, 255),
            }
        );
        assert_eq!(text.spans[2], TextSpan::Plain(" now".to_string()));
    }

    #[test]
    fn test_colored_span_six_digit_hex() {
        let text = parse_formatted_text(r#"<span style=color:#FF8800>warm</span>"#).unwrap();
        assert_eq!(text.spans.len(), 1);
        assert_eq!(
            text.spans[0],
            TextSpan::Colored {
                text: "warm".to_string(),
                italic: false,
                rgb: (0xFF, 0x88, 0x00),
            }
        );
    }

    #[test]
    fn test_italic_with_colored_span() {
        // The fixture's exact pattern: italic wrapping a color span
        let text =
            parse_formatted_text(r#"<i><span style=color:#00f>2 Over 1 Game Force</span></i>"#)
                .unwrap();
        assert_eq!(text.spans.len(), 1);
        assert_eq!(
            text.spans[0],
            TextSpan::Colored {
                text: "2 Over 1 Game Force".to_string(),
                italic: true,
                rgb: (0, 0, 255),
            }
        );
    }

    #[test]
    fn test_italic_mixed_with_colored_span() {
        // Italic with both plain italic text and a colored span inside
        let text =
            parse_formatted_text(r#"<i>Read <span style=color:#00f>this book</span> first</i>"#)
                .unwrap();
        assert_eq!(text.spans.len(), 3);
        assert_eq!(text.spans[0], TextSpan::Italic("Read ".to_string()));
        assert_eq!(
            text.spans[1],
            TextSpan::Colored {
                text: "this book".to_string(),
                italic: true,
                rgb: (0, 0, 255),
            }
        );
        assert_eq!(text.spans[2], TextSpan::Italic(" first".to_string()));
    }

    #[test]
    fn test_strip_empty_italic_tags_no_content() {
        // Empty italic tags with no content at all
        let text = parse_formatted_text("Before<i></i>After").unwrap();
        assert_eq!(text.spans.len(), 1);
        assert_eq!(text.spans[0], TextSpan::Plain("BeforeAfter".to_string()));
    }

    #[test]
    fn bbo_suit_escapes_are_suits() {
        // `3!SX`, `!DK` and `K!H` all occur in the corpus, so no word-boundary test
        assert_eq!(replace_suit_escapes("3!SX, !DK and K!H"), "3♠X, ♦K and K♥");
        assert_eq!(replace_suit_escapes("!!! 1\\S"), "!!! 1♠");
        let text = parse_formatted_text("lead the !SK").unwrap();
        assert!(text.spans.contains(&TextSpan::CardRef {
            suit: Suit::Spades,
            rank: Rank::King
        }));
    }
}
