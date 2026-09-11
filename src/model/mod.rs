pub mod analysis;
pub mod auction;
pub mod bcflags;
pub mod board;
pub mod card;
pub mod commentary;
pub mod deal;
pub mod hand;
pub mod metadata;

pub use auction::{
    AnnotatedCall, Auction, AuctionExt, BidSuit, Call, CallExt, FinalContract, Strain,
};
pub use bcflags::BCFlags;
pub use board::{Board, HiddenHands, PlayerNames, Vulnerability};
pub use card::{Card, Rank, RankExt, Suit, SuitExt, RANKS_DISPLAY_ORDER, SUITS_DISPLAY_ORDER};
pub use commentary::{CommentaryBlock, CommentarySlot, FormattedText, TextSpan};
pub use deal::{Deal, Direction, DirectionExt};
pub use hand::{Hand, Holding};
pub use metadata::{FontSettings, FontSpec, PbnMetadata};
// The play sequence is bridge-types' as it stands: the renderer reads only
// `tricks` and each trick's `cards`, and those are the same fields there.
pub use bridge_types::{PlaySequence, Trick};
