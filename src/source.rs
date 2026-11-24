//! Where each board's record sits in the PBN text, for showing the source
//! beside the PDF (the web app's and Bridge Viewer's View Source).

/// Each board's record as `(start, end)` byte offsets into `text`, in file
/// order: record `i` is board `i` of [`crate::parse_pbn`], and so board `i + 1`
/// of an embedded render's click map when every board is rendered. `None` if
/// the text won't read as PBN.
pub fn source_records(text: &str) -> Option<Vec<(usize, usize)>> {
    let doc = bridge_encodings::pbn::PbnDocument::parse(text).ok()?;
    let mut spans = Vec::with_capacity(doc.boards().len());
    let mut cursor = 0;
    for board in 0..doc.boards().len() {
        let record = doc.record(board)?;
        // Records are the file's own text, one after another; two boards
        // sharing a block both return all of it.
        let start = match text[cursor..].find(record) {
            Some(at) => cursor + at,
            None => text.find(record)?,
        };
        let end = start + record.len();
        spans.push((start, end));
        cursor = end.min(text.len());
    }
    Some(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_board_has_its_own_record() {
        let text =
            "% header\n[Event \"x\"]\n[Board \"1\"]\n[Deal \"N:AKQJ.T98.765.432 - - -\"]\n\n\
                    [Board \"2\"]\n[Deal \"N:AKQJ.T98.765.432 - - -\"]\n";
        let spans = source_records(text).unwrap();
        assert_eq!(spans.len(), 2);
        assert!(text[spans[0].0..spans[0].1].contains("[Board \"1\"]"));
        assert!(text[spans[1].0..spans[1].1].starts_with("[Board \"2\"]"));
        assert!(spans[0].1 <= spans[1].0);
    }
}
