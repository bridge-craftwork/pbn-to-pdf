//! The PDF document every layout draws into.

use printpdf::PdfDocument;

/// The renderer's name and version, stamped into every PDF as its `/Producer`
/// and `/Creator`.
pub const PRODUCER: &str = concat!("pbn-to-pdf ", env!("CARGO_PKG_VERSION"));

/// A new document, stamped with the build that made it.
///
/// Every layout creates its document here, so a packaged PDF can answer "which
/// build made this?" from `pdfinfo` alone (issue #23). The dates are left at
/// printpdf's fixed defaults: unlike a timestamp, the version changes only on a
/// release, so output stays byte-reproducible between releases.
pub fn new_document(title: &str) -> PdfDocument {
    let mut doc = PdfDocument::new(title);
    doc.metadata.info.producer = PRODUCER.to_string();
    doc.metadata.info.creator = PRODUCER.to_string();
    doc
}
