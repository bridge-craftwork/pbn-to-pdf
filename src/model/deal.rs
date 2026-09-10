//! Four hands as a page shows them.
//!
//! [`Deal`] stays local for the same reason [`super::hand::Hand`] does: it is
//! the display view's container, and a `bridge_types::Deal` holds
//! `bridge_types::Hand`s. The two travel together or not at all.

use super::card::{Suit, SUITS_DISPLAY_ORDER};
use super::hand::Hand;

// Re-export Direction from bridge-types
pub use bridge_types::Direction;

/// Extension trait for Direction with display-oriented methods
pub trait DirectionExt {
    /// Returns all directions in clockwise order
    fn all() -> [Direction; 4];
    /// Returns the table position (0-3) for bidding display (West=0, North=1, East=2, South=3)
    fn table_position(&self) -> usize;
}

impl DirectionExt for Direction {
    fn all() -> [Direction; 4] {
        Direction::ALL
    }

    fn table_position(&self) -> usize {
        match self {
            Direction::West => 0,
            Direction::North => 1,
            Direction::East => 2,
            Direction::South => 3,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Deal {
    pub north: Hand,
    pub east: Hand,
    pub south: Hand,
    pub west: Hand,
}

impl Deal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn hand(&self, direction: Direction) -> &Hand {
        match direction {
            Direction::North => &self.north,
            Direction::East => &self.east,
            Direction::South => &self.south,
            Direction::West => &self.west,
        }
    }

    pub fn hand_mut(&mut self, direction: Direction) -> &mut Hand {
        match direction {
            Direction::North => &mut self.north,
            Direction::East => &mut self.east,
            Direction::South => &mut self.south,
            Direction::West => &mut self.west,
        }
    }

    pub fn set_hand(&mut self, direction: Direction, hand: Hand) {
        match direction {
            Direction::North => self.north = hand,
            Direction::East => self.east = hand,
            Direction::South => self.south = hand,
            Direction::West => self.west = hand,
        }
    }

    /// Returns which suits have at least one card across all four hands.
    /// Used to detect hand fragments that only show certain suits.
    pub fn suits_present(&self) -> Vec<Suit> {
        SUITS_DISPLAY_ORDER
            .into_iter()
            .filter(|suit| {
                !self.north.holding(*suit).is_void()
                    || !self.east.holding(*suit).is_void()
                    || !self.south.holding(*suit).is_void()
                    || !self.west.holding(*suit).is_void()
            })
            .collect()
    }

    /// Returns true if this is a hand fragment (not all suits have cards)
    pub fn is_fragment(&self) -> bool {
        self.suits_present().len() < 4
    }

    /// Returns true if this deal has no cards at all (empty deal)
    pub fn is_empty(&self) -> bool {
        self.suits_present().is_empty()
    }

    /// Count total visible cards across all non-hidden hands
    pub fn visible_card_count(&self, hidden: &super::HiddenHands) -> usize {
        let mut count = 0;
        if !hidden.north {
            count += self.north.card_count();
        }
        if !hidden.east {
            count += self.east.card_count();
        }
        if !hidden.south {
            count += self.south.card_count();
        }
        if !hidden.west {
            count += self.west.card_count();
        }
        count
    }

    /// Get the single visible card if there's exactly one visible card in the deal
    /// Returns Some((Suit, Rank)) if exactly one card is visible, None otherwise
    pub fn get_single_visible_card(
        &self,
        hidden: &super::HiddenHands,
    ) -> Option<(super::card::Suit, super::card::Rank)> {
        if self.visible_card_count(hidden) != 1 {
            return None;
        }

        // Find the single card
        for direction in Direction::ALL {
            let is_hidden = match direction {
                Direction::North => hidden.north,
                Direction::East => hidden.east,
                Direction::South => hidden.south,
                Direction::West => hidden.west,
            };
            if is_hidden {
                continue;
            }

            let hand = self.hand(direction);
            for suit in super::card::SUITS_DISPLAY_ORDER {
                let holding = hand.holding(suit);
                if !holding.is_empty() {
                    // Return the first (and only) rank in this holding
                    return Some((suit, holding.ranks[0]));
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::card::Rank;
    use super::super::hand::Holding;
    use super::*;

    #[test]
    fn test_direction_from_char() {
        assert_eq!(Direction::from_char('N'), Some(Direction::North));
        assert_eq!(Direction::from_char('e'), Some(Direction::East));
        assert_eq!(Direction::from_char('X'), None);
    }

    #[test]
    fn test_direction_next() {
        assert_eq!(Direction::North.next(), Direction::East);
        assert_eq!(Direction::East.next(), Direction::South);
        assert_eq!(Direction::South.next(), Direction::West);
        assert_eq!(Direction::West.next(), Direction::North);
    }

    #[test]
    fn test_direction_partner() {
        assert_eq!(Direction::North.partner(), Direction::South);
        assert_eq!(Direction::East.partner(), Direction::West);
    }

    #[test]
    fn test_table_position() {
        assert_eq!(Direction::West.table_position(), 0);
        assert_eq!(Direction::North.table_position(), 1);
        assert_eq!(Direction::East.table_position(), 2);
        assert_eq!(Direction::South.table_position(), 3);
    }

    #[test]
    fn test_suits_present_full_deal() {
        let mut deal = Deal::new();
        deal.north.spades = Holding::from_ranks([Rank::Ace, Rank::King]);
        deal.north.hearts = Holding::from_ranks([Rank::Ace]);
        deal.south.diamonds = Holding::from_ranks([Rank::Ace]);
        deal.west.clubs = Holding::from_ranks([Rank::Ace]);

        let suits = deal.suits_present();
        assert_eq!(suits.len(), 4);
        assert!(!deal.is_fragment());
    }

    #[test]
    fn test_suits_present_spades_only() {
        let mut deal = Deal::new();
        deal.north.spades = Holding::from_ranks([Rank::King, Rank::Queen, Rank::Jack, Rank::Ten]);
        deal.east.spades = Holding::from_ranks([Rank::Seven, Rank::Six, Rank::Four, Rank::Two]);

        let suits = deal.suits_present();
        assert_eq!(suits.len(), 1);
        assert_eq!(suits[0], Suit::Spades);
        assert!(deal.is_fragment());
    }

    #[test]
    fn test_suits_present_empty_deal() {
        let deal = Deal::new();
        let suits = deal.suits_present();
        assert_eq!(suits.len(), 0);
        assert!(deal.is_fragment());
        assert!(deal.is_empty());
    }

    #[test]
    fn test_is_empty_with_cards() {
        let mut deal = Deal::new();
        deal.north.spades = Holding::from_ranks([Rank::Ace]);

        assert!(!deal.is_empty());
    }
}
