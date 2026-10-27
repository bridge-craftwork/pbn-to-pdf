use thiserror::Error;

#[derive(Error, Debug)]
pub enum PbnError {
    #[error("Invalid tag pair at line {line}: {message}")]
    InvalidTagPair { line: usize, message: String },

    #[error("Invalid deal notation: {0}")]
    InvalidDeal(String),

    #[error("Invalid card: {0}")]
    InvalidCard(String),

    #[error("Invalid call in auction: {0}")]
    InvalidCall(String),

    #[error("Unclosed commentary brace")]
    UnclosedComment,

    #[error("Unknown formatting tag: {0}")]
    UnknownFormatTag(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Error, Debug)]
pub enum RenderError {
    #[error("Failed to load font: {0}")]
    FontLoad(String),

    #[error("PDF generation error: {0}")]
    PdfGeneration(String),

    #[error("Layout overflow: content exceeds page bounds")]
    LayoutOverflow,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Card asset loading error: {0}")]
    CardAsset(String),

    #[error("Could not embed the Contract 5 payload: {0}")]
    Payload(String),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid board range: {0}")]
    InvalidBoardRange(String),

    #[error("Invalid color specification: {0}")]
    InvalidColor(String),
}
