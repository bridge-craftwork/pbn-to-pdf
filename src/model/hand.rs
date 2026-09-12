//! A hand as a page shows it: four suits, each in display order.
//!
//! This is deliberately not `bridge_types::Hand`, and the difference is the
//! point. That one is the deal — a flat set of thirteen cards, the right shape
//! for dealing, solving and scoring. This one is a *view* of it, built once per
//! board by the parser and read by every diagram, fan and dummy layout, all of
//! which walk suits and then ranks in display order.
//!
//! Holding it as a view rather than a second source of truth is what keeps the
//! two from drifting: nothing constructs a `Hand` here except from a
//! `bridge_types::Hand`. Deriving the suits on each access instead would rebuild
//! them per call and hand back a temporary where the renderer wants a borrow.

use std::fmt;

use super::card::{rank_display_cmp, Rank, RankExt, Suit, SUITS_DISPLAY_ORDER};

/// Cards held in a single suit, stored in display order (Ace first)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Holding {
    /// Ranks stored in display order (Ace, King, Queen, ... Two)
    pub ranks: Vec<Rank>,
    /// Spot cards whose rank the file does not give -- the `x`s of `Kxx` --
    /// which a diagram shows after the ranks
    pub unknown: u8,
}

impl Holding {
    pub fn new() -> Self {
        Self {
            ranks: Vec::new(),
            unknown: 0,
        }
    }

    pub fn from_ranks(ranks: impl IntoIterator<Item = Rank>) -> Self {
        let mut holding = Self {
            ranks: ranks.into_iter().collect(),
            unknown: 0,
        };
        holding.sort_display_order();
        holding
    }

    /// Sort ranks in display order (Ace first, Two last)
    fn sort_display_order(&mut self) {
        self.ranks.sort_by(rank_display_cmp);
    }

    pub fn add(&mut self, rank: Rank) {
        if !self.ranks.contains(&rank) {
            self.ranks.push(rank);
            self.sort_display_order();
        }
    }

    /// Cards held, the unknown spots included
    pub fn len(&self) -> usize {
        self.ranks.len() + self.unknown as usize
    }

    pub fn is_empty(&self) -> bool {
        self.ranks.is_empty() && self.unknown == 0
    }

    pub fn hcp(&self) -> u8 {
        self.ranks.iter().map(|r| r.hcp_value()).sum()
    }

    pub fn is_void(&self) -> bool {
        self.is_empty()
    }

    pub fn contains(&self, rank: &Rank) -> bool {
        self.ranks.contains(rank)
    }
}

impl fmt::Display for Holding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_void() {
            write!(f, "-")
        } else {
            for rank in &self.ranks {
                write!(f, "{}", rank.display_str())?;
            }
            for _ in 0..self.unknown {
                write!(f, "x")?;
            }
            Ok(())
        }
    }
}

/// A player's 13-card hand
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hand {
    pub spades: Holding,
    pub hearts: Holding,
    pub diamonds: Holding,
    pub clubs: Holding,
}

impl Hand {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_holdings(
        spades: Holding,
        hearts: Holding,
        diamonds: Holding,
        clubs: Holding,
    ) -> Self {
        Self {
            spades,
            hearts,
            diamonds,
            clubs,
        }
    }

    pub fn holding(&self, suit: Suit) -> &Holding {
        match suit {
            Suit::Spades => &self.spades,
            Suit::Hearts => &self.hearts,
            Suit::Diamonds => &self.diamonds,
            Suit::Clubs => &self.clubs,
        }
    }

    pub fn holding_mut(&mut self, suit: Suit) -> &mut Holding {
        match suit {
            Suit::Spades => &mut self.spades,
            Suit::Hearts => &mut self.hearts,
            Suit::Diamonds => &mut self.diamonds,
            Suit::Clubs => &mut self.clubs,
        }
    }

    pub fn total_hcp(&self) -> u8 {
        self.spades.hcp() + self.hearts.hcp() + self.diamonds.hcp() + self.clubs.hcp()
    }

    /// Calculate length points (1 point for each card beyond 4 in each suit)
    pub fn length_points(&self) -> u8 {
        let mut points = 0u8;
        for suit in SUITS_DISPLAY_ORDER {
            let len = self.holding(suit).len();
            if len > 4 {
                points += (len - 4) as u8;
            }
        }
        points
    }

    pub fn shape(&self) -> [u8; 4] {
        [
            self.spades.len() as u8,
            self.hearts.len() as u8,
            self.diamonds.len() as u8,
            self.clubs.len() as u8,
        ]
    }

    pub fn card_count(&self) -> usize {
        self.spades.len() + self.hearts.len() + self.diamonds.len() + self.clubs.len()
    }

    /// Check if the hand contains a specific card
    pub fn contains(&self, suit: Suit, rank: Rank) -> bool {
        self.holding(suit).contains(&rank)
    }
}

impl fmt::Display for Hand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} {} {} {} {} {}",
            Suit::Spades.symbol(),
            self.spades,
            Suit::Hearts.symbol(),
            self.hearts,
            Suit::Diamonds.symbol(),
            self.diamonds,
            Suit::Clubs.symbol(),
            self.clubs
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_holding_hcp() {
        let holding = Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen]);
        assert_eq!(holding.hcp(), 9);
    }

    #[test]
    fn test_empty_holding() {
        let holding = Holding::new();
        assert!(holding.is_void());
        assert_eq!(holding.to_string(), "-");
    }

    #[test]
    fn test_holding_display_order() {
        // Even if we add ranks in wrong order, they should display in correct order
        let holding = Holding::from_ranks([Rank::Two, Rank::Ace, Rank::King]);
        assert_eq!(holding.to_string(), "AK2");
    }

    #[test]
    fn test_hand_shape() {
        let mut hand = Hand::new();
        hand.spades =
            Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen, Rank::Jack, Rank::Ten]);
        hand.hearts = Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen]);
        hand.diamonds = Holding::from_ranks([Rank::Ace, Rank::King]);
        hand.clubs = Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen]);

        assert_eq!(hand.shape(), [5, 3, 2, 3]);
        assert_eq!(hand.card_count(), 13);
    }

    #[test]
    fn test_length_points() {
        let mut hand = Hand::new();
        // 5-card spade suit = 1 length point
        hand.spades =
            Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen, Rank::Jack, Rank::Ten]);
        // 3-card heart suit = 0 length points
        hand.hearts = Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen]);
        // 2-card diamond suit = 0 length points
        hand.diamonds = Holding::from_ranks([Rank::Ace, Rank::King]);
        // 3-card club suit = 0 length points
        hand.clubs = Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen]);

        assert_eq!(hand.length_points(), 1);

        // Add 6-card suit (2 length points)
        let mut hand2 = Hand::new();
        hand2.spades = Holding::from_ranks([
            Rank::Ace,
            Rank::King,
            Rank::Queen,
            Rank::Jack,
            Rank::Ten,
            Rank::Nine,
        ]);
        hand2.hearts = Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen]);
        hand2.diamonds = Holding::from_ranks([Rank::Ace, Rank::King]);
        hand2.clubs = Holding::from_ranks([Rank::Ace, Rank::King]);

        assert_eq!(hand2.length_points(), 2);

        // Balanced hand (4-3-3-3) = 0 length points
        let mut hand3 = Hand::new();
        hand3.spades = Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen, Rank::Jack]);
        hand3.hearts = Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen]);
        hand3.diamonds = Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen]);
        hand3.clubs = Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen]);

        assert_eq!(hand3.length_points(), 0);
    }

    #[test]
    fn test_contains() {
        let mut hand = Hand::new();
        hand.spades = Holding::from_ranks([Rank::Ace, Rank::King]);

        assert!(hand.contains(Suit::Spades, Rank::Ace));
        assert!(!hand.contains(Suit::Spades, Rank::Queen));
        assert!(!hand.contains(Suit::Hearts, Rank::Ace));
    }
}
