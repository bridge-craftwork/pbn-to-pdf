use super::auction::{Auction, FinalContract};
use super::bcflags::BCFlags;
use super::commentary::CommentaryBlock;
use super::deal::{Deal, Direction};
use bridge_types::PlaySequence;

// Re-export types from bridge-types
pub use bridge_types::{PlayerNames, Vulnerability};

/// Tracks which hands should be hidden in display
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HiddenHands {
    pub north: bool,
    pub east: bool,
    pub south: bool,
    pub west: bool,
}

impl HiddenHands {
    /// Parse from PBN Hidden tag value (e.g., "NS", "ESW", "NESW")
    pub fn from_pbn(s: &str) -> Self {
        let s = s.to_uppercase();
        Self {
            north: s.contains('N'),
            east: s.contains('E'),
            south: s.contains('S'),
            west: s.contains('W'),
        }
    }

    /// Check if a specific direction is hidden
    pub fn is_hidden(&self, direction: Direction) -> bool {
        match direction {
            Direction::North => self.north,
            Direction::East => self.east,
            Direction::South => self.south,
            Direction::West => self.west,
        }
    }

    /// Returns true if all hands are hidden
    pub fn all_hidden(&self) -> bool {
        self.north && self.east && self.south && self.west
    }

    /// Returns true if no hands are hidden
    pub fn none_hidden(&self) -> bool {
        !self.north && !self.east && !self.south && !self.west
    }
}

#[derive(Debug, Clone, Default)]
pub struct Board {
    // Identification
    pub number: Option<u32>,
    /// Raw board identifier string (e.g., "1", "1-1", "1-2")
    pub board_id: Option<String>,
    pub event: Option<String>,
    pub site: Option<String>,
    pub date: Option<String>,

    // Setup
    pub dealer: Option<Direction>,
    pub vulnerable: Vulnerability,
    pub deal: Deal,

    // Player names
    pub players: PlayerNames,

    // Bidding
    pub auction: Option<Auction>,
    pub contract: Option<FinalContract>,
    pub declarer: Option<Direction>,

    // Play
    pub play: Option<PlaySequence>,
    pub result: Option<i8>,

    // Commentary
    pub commentary: Vec<CommentaryBlock>,

    // Display flags (Bridge Composer)
    pub bc_flags: Option<BCFlags>,

    // Hidden hands (from [Hidden] tag)
    pub hidden: HiddenHands,
}

impl Board {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_number(mut self, number: u32) -> Self {
        self.number = Some(number);
        self
    }

    pub fn with_dealer(mut self, dealer: Direction) -> Self {
        self.dealer = Some(dealer);
        self
    }

    pub fn with_vulnerability(mut self, vuln: Vulnerability) -> Self {
        self.vulnerable = vuln;
        self
    }

    pub fn with_deal(mut self, deal: Deal) -> Self {
        self.deal = deal;
        self
    }

    pub fn title(&self) -> String {
        let mut parts = Vec::new();

        if let Some(num) = self.number {
            parts.push(format!("Board {}", num));
        }

        if let Some(dealer) = self.dealer {
            parts.push(format!("{} Deals", dealer));
        }

        parts.push(self.vulnerable.to_string());

        parts.join(" • ")
    }

    pub fn has_commentary(&self) -> bool {
        !self.commentary.is_empty()
    }

    pub fn opening_lead_direction(&self) -> Option<Direction> {
        self.declarer.map(|d| d.next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vulnerability_parsing() {
        assert_eq!(Vulnerability::from_pbn("None"), Some(Vulnerability::None));
        assert_eq!(
            Vulnerability::from_pbn("NS"),
            Some(Vulnerability::NorthSouth)
        );
        assert_eq!(
            Vulnerability::from_pbn("E-W"),
            Some(Vulnerability::EastWest)
        );
        assert_eq!(Vulnerability::from_pbn("Both"), Some(Vulnerability::Both));
    }

    #[test]
    fn test_vulnerability_check() {
        assert!(!Vulnerability::None.is_vulnerable(Direction::North));
        assert!(Vulnerability::Both.is_vulnerable(Direction::North));
        assert!(Vulnerability::NorthSouth.is_vulnerable(Direction::South));
        assert!(!Vulnerability::NorthSouth.is_vulnerable(Direction::East));
    }

    #[test]
    fn test_board_title() {
        let board = Board::new()
            .with_number(1)
            .with_dealer(Direction::North)
            .with_vulnerability(Vulnerability::None);

        assert_eq!(board.title(), "Board 1 • North Deals • None Vul");
    }

    #[test]
    fn test_hidden_hands_parsing() {
        let hidden = HiddenHands::from_pbn("NS");
        assert!(hidden.north);
        assert!(!hidden.east);
        assert!(hidden.south);
        assert!(!hidden.west);

        let hidden = HiddenHands::from_pbn("ESW");
        assert!(!hidden.north);
        assert!(hidden.east);
        assert!(hidden.south);
        assert!(hidden.west);

        let hidden = HiddenHands::from_pbn("NESW");
        assert!(hidden.all_hidden());

        let hidden = HiddenHands::from_pbn("");
        assert!(hidden.none_hidden());
    }

    #[test]
    fn test_hidden_hands_is_hidden() {
        let hidden = HiddenHands::from_pbn("NS");
        assert!(hidden.is_hidden(Direction::North));
        assert!(!hidden.is_hidden(Direction::East));
        assert!(hidden.is_hidden(Direction::South));
        assert!(!hidden.is_hidden(Direction::West));
    }
}
