/// BCFlags - Bridge Composer display flags
///
/// A bitmask that controls the visibility and behavior of various board elements.
/// See docs/BCFlags.md for complete documentation.
#[derive(Debug, Clone, Copy, Default)]
pub struct BCFlags {
    raw: u32,
}

impl BCFlags {
    /// Create BCFlags from a raw u32 value
    pub fn new(raw: u32) -> Self {
        Self { raw }
    }

    /// Parse BCFlags from a hexadecimal string (without 0x prefix)
    pub fn from_hex(s: &str) -> Option<Self> {
        u32::from_str_radix(s.trim(), 16).ok().map(Self::new)
    }

    /// Get the raw value
    pub fn raw(&self) -> u32 {
        self.raw
    }

    // === Show flags (bits 0-7) ===

    /// Show the Play section (bit 0)
    pub fn show_play(&self) -> bool {
        self.raw & 0x00000001 != 0
    }

    /// Show the Results section (bit 1)
    pub fn show_results(&self) -> bool {
        self.raw & 0x00000002 != 0
    }

    /// Show the Final Commentary (bit 2)
    pub fn show_final_commentary(&self) -> bool {
        self.raw & 0x00000004 != 0
    }

    /// Show the Diagram section (bit 3)
    pub fn show_diagram(&self) -> bool {
        self.raw & 0x00000008 != 0
    }

    /// Show the Auction section (bit 4)
    pub fn show_auction(&self) -> bool {
        self.raw & 0x00000010 != 0
    }

    /// Show the Event Commentary (bit 5)
    pub fn show_event_commentary(&self) -> bool {
        self.raw & 0x00000020 != 0
    }

    /// Show the Diagram Commentary (bit 6)
    pub fn show_diagram_commentary(&self) -> bool {
        self.raw & 0x00000040 != 0
    }

    /// Show the Auction Commentary (bit 7)
    pub fn show_auction_commentary(&self) -> bool {
        self.raw & 0x00000080 != 0
    }

    /// Show the cards played so far in the card table (bit 11)
    ///
    /// BridgeComposer's own documentation calls this the "View→Slide
    /// Highlighting" flag, after the GUI feature that sets it, so the name
    /// here is ours. That it is what drives this rendering was probed against
    /// 5.118.2, one board under two flag values:
    ///
    /// - 0x800 clear: the ordinary compass with its letters, the full hands,
    ///   and a `Lead:` line.
    /// - 0x800 set: the trick in a white card table, the played cards greyed
    ///   in the hands, and neither compass letters nor lead line.
    ///
    /// The exercise sets use it for "what do you play?" boards.
    pub fn show_trick(&self) -> bool {
        self.raw & 0x00000800 != 0
    }

    // === Hide flags (bits 20-28) ===

    /// Hide the Board field (bit 20)
    pub fn hide_board(&self) -> bool {
        self.raw & 0x00100000 != 0
    }

    /// Hide the Dealer field (bit 21)
    pub fn hide_dealer(&self) -> bool {
        self.raw & 0x00200000 != 0
    }

    /// Hide the Vulnerable field (bit 22)
    pub fn hide_vulnerable(&self) -> bool {
        self.raw & 0x00400000 != 0
    }

    /// Hide the Total Score Table (bit 23)
    pub fn hide_score_table(&self) -> bool {
        self.raw & 0x00800000 != 0
    }

    /// Hide the Contract and Declarer fields (bit 27)
    pub fn hide_contract(&self) -> bool {
        self.raw & 0x08000000 != 0
    }

    /// Hide "Notes" (bit 28)
    pub fn hide_notes(&self) -> bool {
        self.raw & 0x10000000 != 0
    }

    // === Layout control flags (bits 25-26) ===

    /// Column break before this board (bit 25)
    pub fn column_break(&self) -> bool {
        self.raw & 0x02000000 != 0
    }

    /// Page break before this board (bit 26)
    pub fn page_break(&self) -> bool {
        self.raw & 0x04000000 != 0
    }

    // === Preformatted flags (bits 12-15) ===

    /// Event Commentary preformatted flag (bit 12)
    pub fn event_commentary_preformatted(&self) -> bool {
        self.raw & 0x00001000 != 0
    }

    /// Diagram Commentary preformatted flag (bit 13)
    pub fn diagram_commentary_preformatted(&self) -> bool {
        self.raw & 0x00002000 != 0
    }

    /// Auction Commentary preformatted flag (bit 14)
    pub fn auction_commentary_preformatted(&self) -> bool {
        self.raw & 0x00004000 != 0
    }

    /// Final Commentary preformatted flag (bit 15)
    pub fn final_commentary_preformatted(&self) -> bool {
        self.raw & 0x00008000 != 0
    }

    // === Other flags ===

    /// Show space for hidden hands (bit 29)
    pub fn show_space_for_hidden(&self) -> bool {
        self.raw & 0x20000000 != 0
    }

    // === Convenience methods ===

    /// Returns true if any "show" flag is set
    pub fn has_show_flags(&self) -> bool {
        self.raw & 0x000000FF != 0
    }

    /// Returns true if any "hide" flag is set
    pub fn has_hide_flags(&self) -> bool {
        self.raw & 0x00F00000 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex() {
        let flags = BCFlags::from_hex("60001b").unwrap();
        assert!(flags.show_play());
        assert!(flags.show_results());
        assert!(!flags.show_final_commentary());
        assert!(flags.show_diagram());
        assert!(flags.show_auction());
        assert!(!flags.show_event_commentary());
        assert!(!flags.hide_board());
        assert!(flags.hide_dealer());
        assert!(flags.hide_vulnerable());
    }

    #[test]
    fn test_flags_17() {
        // BCFlags "17" - commentary-only board
        let flags = BCFlags::from_hex("17").unwrap();
        assert!(flags.show_play());
        assert!(flags.show_results());
        assert!(flags.show_final_commentary());
        assert!(!flags.show_diagram());
        assert!(flags.show_auction());
        assert!(!flags.hide_board());
        assert!(!flags.hide_dealer());
        assert!(!flags.hide_vulnerable());
    }

    #[test]
    fn test_flags_600023() {
        // BCFlags "600023" - event commentary header
        let flags = BCFlags::from_hex("600023").unwrap();
        assert!(flags.show_play());
        assert!(flags.show_results());
        assert!(!flags.show_diagram());
        assert!(!flags.show_auction());
        assert!(flags.show_event_commentary());
        assert!(!flags.hide_board());
        assert!(flags.hide_dealer());
        assert!(flags.hide_vulnerable());
    }

    #[test]
    fn test_page_break() {
        let flags = BCFlags::from_hex("4000000").unwrap();
        assert!(flags.page_break());
        assert!(!flags.column_break());
    }

    #[test]
    fn test_column_break() {
        let flags = BCFlags::from_hex("2000000").unwrap();
        assert!(flags.column_break());
        assert!(!flags.page_break());
    }

    #[test]
    fn test_default() {
        let flags = BCFlags::default();
        assert_eq!(flags.raw(), 0);
        assert!(!flags.show_play());
        assert!(!flags.hide_board());
    }
}
