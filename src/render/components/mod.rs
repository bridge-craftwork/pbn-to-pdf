//! Rendering components for PDF generation

pub mod bidding_table;
pub mod commentary;
pub mod declarers_plan_small;
pub mod dummy;
pub mod fan;
pub mod hand_diagram;
pub mod losers_table;
pub mod page_furniture;
pub mod play_record;
pub mod winners_table;

pub use bidding_table::BiddingTableRenderer;
pub use commentary::CommentaryRenderer;
pub use declarers_plan_small::DeclarersPlanSmallRenderer;
pub use dummy::DummyRenderer;
pub use fan::FanRenderer;
pub use hand_diagram::{DiagramDisplayOptions, HandDiagramRenderer};
pub use losers_table::LosersTableRenderer;
pub use page_furniture::PageFurniture;
pub use winners_table::WinnersTableRenderer;
