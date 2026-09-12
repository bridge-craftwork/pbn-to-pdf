//! Layout renderers - one per --layout option

pub mod analysis;
pub mod bidding_sheets;
pub mod dealer_summary;
pub mod declarers_plan;
pub mod hand_record;

pub use analysis::generate_pdf;
pub use bidding_sheets::BiddingSheetsRenderer;
pub use dealer_summary::DealerSummaryRenderer;
pub use declarers_plan::{
    DeclarersPlan1UpRenderer, DeclarersPlan2UpRenderer, DeclarersPlanRenderer,
};
pub use hand_record::HandRecordRenderer;
