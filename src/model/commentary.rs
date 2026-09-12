use super::card::{Rank, RankExt, Suit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextSpan {
    Plain(String),
    Bold(String),
    Italic(String),
    BoldItalic(String),
    Underline(String),
    /// Text with an explicit foreground color, optionally italic.
    /// Parsed from `<span style=color:#XXX>...</span>`, possibly wrapped in `<i>...</i>`.
    Colored {
        text: String,
        italic: bool,
        rgb: (u8, u8, u8),
    },
    SuitSymbol(Suit),
    CardRef {
        suit: Suit,
        rank: Rank,
    },
    LineBreak,
}

impl TextSpan {
    pub fn plain(s: impl Into<String>) -> Self {
        TextSpan::Plain(s.into())
    }

    pub fn bold(s: impl Into<String>) -> Self {
        TextSpan::Bold(s.into())
    }

    pub fn italic(s: impl Into<String>) -> Self {
        TextSpan::Italic(s.into())
    }

    pub fn bold_italic(s: impl Into<String>) -> Self {
        TextSpan::BoldItalic(s.into())
    }

    pub fn underline(s: impl Into<String>) -> Self {
        TextSpan::Underline(s.into())
    }

    pub fn colored(s: impl Into<String>, rgb: (u8, u8, u8)) -> Self {
        TextSpan::Colored {
            text: s.into(),
            italic: false,
            rgb,
        }
    }

    pub fn italic_colored(s: impl Into<String>, rgb: (u8, u8, u8)) -> Self {
        TextSpan::Colored {
            text: s.into(),
            italic: true,
            rgb,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FormattedText {
    pub spans: Vec<TextSpan>,
}

impl FormattedText {
    pub fn new() -> Self {
        Self { spans: Vec::new() }
    }

    pub fn push(&mut self, span: TextSpan) {
        self.spans.push(span);
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Returns true if the text is empty or contains only whitespace
    pub fn is_blank(&self) -> bool {
        if self.spans.is_empty() {
            return true;
        }
        // Check if all spans are whitespace-only or line breaks
        self.spans.iter().all(|span| match span {
            TextSpan::Plain(s)
            | TextSpan::Bold(s)
            | TextSpan::Italic(s)
            | TextSpan::BoldItalic(s)
            | TextSpan::Underline(s) => s.trim().is_empty(),
            TextSpan::Colored { text, .. } => text.trim().is_empty(),
            TextSpan::LineBreak => true,
            // Suit symbols and card refs are not whitespace
            TextSpan::SuitSymbol(_) | TextSpan::CardRef { .. } => false,
        })
    }

    pub fn to_plain_text(&self) -> String {
        let mut result = String::new();
        for span in &self.spans {
            match span {
                TextSpan::Plain(s)
                | TextSpan::Bold(s)
                | TextSpan::Italic(s)
                | TextSpan::BoldItalic(s)
                | TextSpan::Underline(s) => {
                    result.push_str(s);
                }
                TextSpan::Colored { text, .. } => {
                    result.push_str(text);
                }
                TextSpan::SuitSymbol(suit) => {
                    result.push(suit.symbol());
                }
                TextSpan::CardRef { suit, rank } => {
                    result.push(suit.symbol());
                    result.push_str(rank.display_str());
                }
                TextSpan::LineBreak => {
                    result.push('\n');
                }
            }
        }
        result
    }
}

/// Which part of its board a commentary block belongs to, decided by where it
/// stood among the record's tags.
///
/// This is Bridge Composer's reading, measured against 5.118.2: it shows each
/// slot under its own `BCFlags` bit and, in Center mode, in its own place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommentarySlot {
    /// Before `[Board]`: the event commentary, drawn above the board (0x20).
    Event,
    /// Between `[Board]` and `[Deal]`, which Bridge Composer never prints.
    BeforeDeal,
    /// Straight after `[Deal]`: drawn under the diagram, above the auction
    /// (0x40).
    Diagram,
    /// After any later tag: drawn last, under the auction (0x04). Also where a
    /// block of unknown position goes.
    #[default]
    Final,
    /// Inside an `[Auction]` or `[Play]` section's data, which Bridge Composer
    /// discards: a section runs until the next tag, and a block standing in
    /// that run is section data.
    ///
    /// Probed against 5.118.2. A block between two auction lines, one between
    /// two play lines, and one after the last auction line are all left out,
    /// while a block that precedes `[Deal]` is drawn. That holds with the
    /// auction-commentary bit (0x80) both clear and set -- `BCFlags 7f` and
    /// `ff` render alike -- so this is what BridgeComposer does, not a guess at
    /// which commentary bit governs it.
    InSection,
}

#[derive(Debug, Clone)]
pub struct CommentaryBlock {
    pub content: FormattedText,
    pub slot: CommentarySlot,
}

impl CommentaryBlock {
    pub fn new(content: FormattedText) -> Self {
        Self {
            content,
            slot: CommentarySlot::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns true if the content is empty or contains only whitespace
    pub fn is_blank(&self) -> bool {
        self.content.is_blank()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formatted_text_to_plain() {
        let mut text = FormattedText::new();
        text.push(TextSpan::Bold("Bidding.".to_string()));
        text.push(TextSpan::Plain(" Open 1".to_string()));
        text.push(TextSpan::SuitSymbol(Suit::Spades));

        assert_eq!(text.to_plain_text(), "Bidding. Open 1♠");
    }

    #[test]
    fn test_is_blank_empty() {
        let text = FormattedText::new();
        assert!(text.is_blank());
    }

    #[test]
    fn test_is_blank_whitespace_only() {
        let mut text = FormattedText::new();
        text.push(TextSpan::Plain(" ".to_string()));
        assert!(text.is_blank());
    }

    #[test]
    fn test_is_blank_with_content() {
        let mut text = FormattedText::new();
        text.push(TextSpan::Plain("Hello".to_string()));
        assert!(!text.is_blank());
    }

    #[test]
    fn test_is_blank_with_suit_symbol() {
        let mut text = FormattedText::new();
        text.push(TextSpan::SuitSymbol(Suit::Spades));
        assert!(!text.is_blank());
    }

    #[test]
    fn test_commentary_block_is_blank() {
        let mut text = FormattedText::new();
        text.push(TextSpan::Plain(" ".to_string()));
        let block = CommentaryBlock::new(text);
        assert!(block.is_blank());
    }
}
