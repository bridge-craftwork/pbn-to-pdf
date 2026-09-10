//! Auction types for bridge bidding.
//!
//! The auction, its calls and the contract it settles on are bridge-types'.
//! What lives here is the reading of them a page needs: the PBN spellings the
//! CLI and the renderer accept, and the questions a bidding table asks that a
//! general auction type has no reason to answer.

use super::deal::Direction;

pub use bridge_types::{AnnotatedCall, Auction, Call, FinalContract, Strain};

// Type alias for backward compatibility
pub type BidSuit = Strain;

/// Extension trait for Call to add pbn-to-pdf specific functionality
pub trait CallExt {
    fn from_pbn_ext(s: &str) -> Option<Call>;
}

impl CallExt for Call {
    /// Parse a call from PBN notation, with special handling for "AP" (All Pass)
    fn from_pbn_ext(s: &str) -> Option<Call> {
        let s = s.trim();
        if s.to_uppercase() == "AP" {
            return None; // All Pass is handled specially
        }
        Call::from_pbn(s)
    }
}

/// What a bidding table needs to know about an auction beyond its calls.
pub trait AuctionExt {
    /// The pair that did all the bidding, when only one of them did.
    ///
    /// A bidding sheet draws an uncontested auction in two columns rather than
    /// four, so it has to know whether the opponents ever spoke. Passing does
    /// not count as speaking, and neither does the `+` that stands in for a
    /// call not yet made.
    fn uncontested_pair(&self) -> Option<(Direction, Direction)>;
}

impl AuctionExt for Auction {
    fn uncontested_pair(&self) -> Option<(Direction, Direction)> {
        let mut ns_bid = false;
        let mut ew_bid = false;

        let mut current = self.dealer;
        for annotated in &self.calls {
            let dominated = matches!(annotated.call, Call::Pass | Call::Continue);
            if !dominated {
                match current {
                    Direction::North | Direction::South => ns_bid = true,
                    Direction::East | Direction::West => ew_bid = true,
                }
            }
            current = current.next();
        }

        match (ns_bid, ew_bid) {
            (true, false) => Some((Direction::North, Direction::South)),
            (false, true) => Some((Direction::West, Direction::East)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bid_parsing() {
        assert_eq!(
            Call::from_pbn("1C"),
            Some(Call::Bid {
                level: 1,
                strain: Strain::Clubs
            })
        );
        assert_eq!(
            Call::from_pbn("3NT"),
            Some(Call::Bid {
                level: 3,
                strain: Strain::NoTrump
            })
        );
        assert_eq!(Call::from_pbn("Pass"), Some(Call::Pass));
        assert_eq!(Call::from_pbn("X"), Some(Call::Double));
        assert_eq!(Call::from_pbn("XX"), Some(Call::Redouble));
        assert_eq!(Call::from_pbn("+"), Some(Call::Continue));
    }

    #[test]
    fn test_bid_display() {
        assert_eq!(
            Call::Bid {
                level: 1,
                strain: Strain::NoTrump
            }
            .to_string(),
            "1NT"
        );
    }

    #[test]
    fn test_contract_display() {
        let contract = FinalContract {
            level: 4,
            strain: Strain::Spades,
            doubled: true,
            redoubled: false,
            declarer: Direction::South,
        };
        assert_eq!(contract.to_string(), "4♠X by South");
    }

    #[test]
    fn an_uncontested_auction_names_the_pair_that_bid_it() {
        // Passes do not count as speaking, nor does the `+` placeholder.
        let mut auction = Auction::new(Direction::North);
        for call in [Call::bid(1, Strain::Spades), Call::Pass, Call::Continue] {
            auction.add_call(call);
        }
        assert_eq!(
            auction.uncontested_pair(),
            Some((Direction::North, Direction::South))
        );

        // North deals, so the second call is East's: an overcall makes the
        // auction contested, and it gets all four columns.
        let mut auction = Auction::new(Direction::North);
        for call in [Call::bid(1, Strain::Spades), Call::bid(2, Strain::Hearts)] {
            auction.add_call(call);
        }
        assert_eq!(auction.uncontested_pair(), None);
    }

    #[test]
    fn test_bidsuit_alias() {
        // BidSuit is now an alias for Strain
        let suit: BidSuit = Strain::Hearts;
        assert!(suit.is_red());
        assert_eq!(suit.symbol(), "♥");
    }
}
