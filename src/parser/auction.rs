use crate::model::{Auction, Call, Direction};

/// Parse an auction section from PBN
/// The auction starts after [Auction "X"] where X is the dealer
/// Calls are separated by whitespace
pub fn parse_auction(dealer: Direction, input: &str) -> Result<Auction, String> {
    let mut auction = Auction::new(dealer);
    let input = input.trim();

    if input.is_empty() {
        return Ok(auction);
    }

    // Split on whitespace
    let tokens: Vec<&str> = input.split_whitespace().collect();
    let mut i = 0;

    while i < tokens.len() {
        let token = tokens[i];
        i += 1;

        // Skip empty tokens
        if token.is_empty() {
            continue;
        }

        // Handle special tokens
        let upper = token.to_uppercase();
        if upper == "AP" {
            // All Pass - add 3 passes to end the auction
            auction.add_call(Call::Pass);
            auction.add_call(Call::Pass);
            auction.add_call(Call::Pass);
            auction.is_passed_out = false;
            break;
        } else if upper == "*" {
            // End of auction marker (incomplete auction)
            break;
        }

        // Check if this token is a standalone annotation (=N=)
        if token.starts_with('=') && token.ends_with('=') {
            // This is an annotation for the previous call
            if let Some(note_num) = parse_note_reference(token) {
                if let Some(last_call) = auction.calls.last_mut() {
                    last_call.annotation = Some(note_num.to_string());
                }
            }
            continue;
        }

        // Try to parse as a call
        // Extract call and any inline annotation
        let (clean_token, annotation) = extract_annotation(token);

        if let Some(call) = Call::from_pbn(&clean_token) {
            auction.add_annotated_call(call, annotation);
        } else if clean_token.is_empty() && token.starts_with('$') {
            // Standalone $N NAG marker — attach as annotation to previous call
            // PBN standard: $1 = "!", $2 = "?", $3 = "!!", $4 = "??"
            if let Some(last_call) = auction.calls.last_mut() {
                let nag_text = match token {
                    "$1" => "!",
                    "$2" => "?",
                    "$3" => "!!",
                    "$4" => "??",
                    _ => "",
                };
                if !nag_text.is_empty() {
                    last_call.annotation = Some(nag_text.to_string());
                }
            }
        } else {
            log::debug!("Skipping unrecognized auction token: {}", token);
        }
    }

    // Check if auction passed out (4 passes at start)
    if auction.calls.len() >= 4
        && auction.calls[0].call == Call::Pass
        && auction.calls[1].call == Call::Pass
        && auction.calls[2].call == Call::Pass
        && auction.calls[3].call == Call::Pass
    {
        auction.is_passed_out = true;
    }

    Ok(auction)
}

/// Parse a note reference like "=1=" and return the note number
fn parse_note_reference(token: &str) -> Option<u8> {
    if token.starts_with('=') && token.ends_with('=') && token.len() >= 3 {
        let inner = &token[1..token.len() - 1];
        inner.parse::<u8>().ok()
    } else {
        None
    }
}

/// Extract annotation from a call token
/// Returns (clean_call, optional_annotation)
/// e.g., "1C!" -> ("1C", Some("!"))
/// e.g., "2H=1=" -> ("2H", Some("1"))
/// e.g., "3NT" -> ("3NT", None)
fn extract_annotation(token: &str) -> (String, Option<String>) {
    // Check for =N= annotation at the end
    if let Some(eq_pos) = token.find('=') {
        let before = &token[..eq_pos];
        let rest = &token[eq_pos..];
        if rest.ends_with('=') && rest.len() >= 3 {
            let note_num = &rest[1..rest.len() - 1];
            if note_num.parse::<u8>().is_ok() {
                return (before.to_string(), Some(note_num.to_string()));
            }
        }
    }

    // Check for ! or ? at the end (alert/question markers)
    if token.ends_with('!') || token.ends_with('?') {
        let marker = token.chars().last().unwrap();
        let clean = &token[..token.len() - 1];
        return (clean.to_string(), Some(marker.to_string()));
    }

    // Check for $N NAG marker
    if let Some(dollar_pos) = token.find('$') {
        let clean = &token[..dollar_pos];
        return (clean.to_string(), None); // NAG markers are not displayed
    }

    // No annotation
    (token.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Strain;

    #[test]
    fn test_simple_auction() {
        let auction = parse_auction(Direction::North, "1D 1S X Pass 1NT AP").unwrap();

        assert_eq!(auction.dealer, Direction::North);
        assert_eq!(auction.calls.len(), 8); // 5 calls + 3 passes from AP

        // Check first few calls
        assert_eq!(
            auction.calls[0].call,
            Call::Bid {
                level: 1,
                strain: Strain::Diamonds
            }
        );
        assert_eq!(
            auction.calls[1].call,
            Call::Bid {
                level: 1,
                strain: Strain::Spades
            }
        );
        assert_eq!(auction.calls[2].call, Call::Double);
        assert_eq!(auction.calls[3].call, Call::Pass);
    }

    #[test]
    fn test_passed_out() {
        let auction = parse_auction(Direction::South, "Pass Pass Pass Pass").unwrap();
        assert!(auction.is_passed_out);
        assert_eq!(auction.calls.len(), 4);
    }

    #[test]
    fn test_auction_with_annotations() {
        let auction = parse_auction(Direction::West, "1C! 1H 2C$1 Pass").unwrap();
        assert_eq!(auction.calls.len(), 4);
        assert_eq!(
            auction.calls[0].call,
            Call::Bid {
                level: 1,
                strain: Strain::Clubs
            }
        );
        // Check that ! annotation is preserved
        assert_eq!(auction.calls[0].annotation, Some("!".to_string()));
        // $1 NAG marker should not produce an annotation
        assert_eq!(auction.calls[2].annotation, None);
    }

    #[test]
    fn test_auction_with_note_references() {
        // Test =N= style annotations (as in Slam Judgment file)
        // =N= annotations are separate tokens, not counted as calls
        let auction = parse_auction(
            Direction::East,
            "Pass Pass Pass 2NT =1= Pass 3H =2= Pass 4S",
        )
        .unwrap();
        // 8 calls: Pass Pass Pass 2NT Pass 3H Pass 4S (=N= are annotations, not calls)
        assert_eq!(auction.calls.len(), 8);
        // 2NT (index 3) should have annotation "1"
        assert_eq!(
            auction.calls[3].call,
            Call::Bid {
                level: 2,
                strain: Strain::NoTrump
            }
        );
        assert_eq!(auction.calls[3].annotation, Some("1".to_string()));
        // 3H (index 5) should have annotation "2"
        assert_eq!(
            auction.calls[5].call,
            Call::Bid {
                level: 3,
                strain: Strain::Hearts
            }
        );
        assert_eq!(auction.calls[5].annotation, Some("2".to_string()));
    }

    #[test]
    fn test_extract_annotation() {
        assert_eq!(
            extract_annotation("1C!"),
            ("1C".to_string(), Some("!".to_string()))
        );
        assert_eq!(
            extract_annotation("2H=1="),
            ("2H".to_string(), Some("1".to_string()))
        );
        assert_eq!(extract_annotation("3NT"), ("3NT".to_string(), None));
        assert_eq!(extract_annotation("Pass$1"), ("Pass".to_string(), None));
    }

    #[test]
    fn test_empty_auction() {
        let auction = parse_auction(Direction::East, "").unwrap();
        assert_eq!(auction.calls.len(), 0);
    }

    #[test]
    fn test_final_contract() {
        // N bids 1NT, E passes, S bids 3NT. North named notrump first, so
        // North declares — not South, who made the last bid.
        let auction = parse_auction(Direction::North, "1NT Pass 3NT AP").unwrap();
        let contract = auction.final_contract().unwrap();

        assert_eq!(contract.level, 3);
        assert_eq!(contract.suit, Strain::NoTrump);
        assert!(!contract.doubled);
        assert!(!contract.redoubled);
        assert_eq!(contract.declarer, Direction::North);
    }

    #[test]
    fn test_final_contract_declarer_is_first_to_name_strain() {
        // The plain raise: N opens 1S, S raises to 4S, N declares.
        let auction = parse_auction(Direction::North, "1S Pass 4S AP").unwrap();
        let contract = auction.final_contract().unwrap();

        assert_eq!(contract.level, 4);
        assert_eq!(contract.suit, Strain::Spades);
        assert_eq!(contract.declarer, Direction::North);
    }

    #[test]
    fn test_doubled_contract() {
        let auction = parse_auction(Direction::South, "1S X XX Pass Pass Pass").unwrap();
        let contract = auction.final_contract().unwrap();

        assert_eq!(contract.level, 1);
        assert_eq!(contract.suit, Strain::Spades);
        assert!(!contract.doubled);
        assert!(contract.redoubled);
    }

    #[test]
    fn test_blank_parsing() {
        // Single blank (fill-in-the-blank exercise)
        let auction = parse_auction(Direction::North, "_____").unwrap();
        assert_eq!(auction.calls.len(), 1);
        assert_eq!(auction.calls[0].call, Call::Blank);
    }

    #[test]
    fn test_blank_uncontested_pair() {
        // Board 1-1 from Stayman exercises: North deals, auction is just "_____"
        // This should be detected as N-S being the "bidding" side for 2-column display
        let auction = parse_auction(Direction::North, "_____").unwrap();
        let pair = auction.uncontested_pair();
        assert_eq!(pair, Some((Direction::North, Direction::South)));
    }

    #[test]
    fn test_blank_with_bids() {
        // Auction with bids followed by blank
        let auction = parse_auction(Direction::North, "1NT Pass 2C Pass 2H Pass ____").unwrap();
        assert_eq!(auction.calls.len(), 7);
        assert_eq!(auction.calls[6].call, Call::Blank);
        // N-S bid, E-W only pass - should be uncontested N-S
        let pair = auction.uncontested_pair();
        assert_eq!(pair, Some((Direction::North, Direction::South)));
    }
}
