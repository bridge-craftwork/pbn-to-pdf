//! Contract 5: the payload a PDF carries inside itself.
//!
//! The contract lives in lesson-stage (`docs/contracts/pdf-attachments.md`).
//! This crate produces its `pbn` profile, payload version 2: four embedded
//! files, and one `lesson-block:<index>` link annotation over each board
//! figure, so a reader such as lesson-stage can find every board on the page
//! and get its deal from the PDF alone.
//!
//! | File | `AFRelationship` | Contents |
//! |---|---|---|
//! | the input's own name | `Source` | the `.pbn`, byte for byte |
//! | `lesson-provenance.json` | `Supplement` | what made this PDF |
//! | `lesson-blocks.json` | `Data` | the click map |
//! | `lesson-hands.pbn` | `Data` | the rendered boards, renumbered from 1 |
//!
//! Output stays byte-reproducible: nothing here reads a clock, and every
//! collection is written in a fixed order.

use std::collections::BTreeMap;
use std::io::Cursor;

use bridge_encodings::pbn::PbnDocument;
use lopdf::{dictionary, Object, ObjectId, Stream, StringFormat};

use crate::cli::Layout;
use crate::error::RenderError;
use crate::model::Board;
use crate::render::helpers::document::PRODUCER;
use crate::render::helpers::regions::BoardRegion;

/// The URI scheme of the link annotation over each figure.
pub const BLOCK_URI_SCHEME: &str = "lesson-block";
pub const BLOCKS_FILE: &str = "lesson-blocks.json";
pub const HANDS_FILE: &str = "lesson-hands.pbn";
pub const PROVENANCE_FILE: &str = "lesson-provenance.json";
/// Contract 5 payload version for the `pbn` profile.
pub const PAYLOAD_VERSION: u32 = 2;

/// Points per millimetre: layouts draw in mm, the click map is in points.
const PT_PER_MM: f32 = 72.0 / 25.4;

/// The PBN file a PDF was rendered from, embedded as its `/Source`.
#[derive(Debug, Clone, Copy)]
pub struct EmbedSource<'a> {
    /// Its filename, as the reader should see it (no directories).
    pub name: &'a str,
    /// The file's full text, exactly as read.
    pub text: &'a str,
}

/// A rendered PDF with its payload attached.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub pdf: Vec<u8>,
    /// The click map, `lesson-blocks.json`, as embedded -- handed back so a
    /// caller that just rendered the PDF need not dig it out again.
    pub click_map: String,
}

/// Attach the payload to a rendered PDF.
///
/// `boards` are the boards the layout drew and `regions` where it drew them.
/// `positions[i]` is the index of `boards[i]` among the boards of
/// `source.text`, as [`crate::parse_pbn`] reads them; it is how each record is
/// copied from the source verbatim.
pub fn embed(
    pdf: Vec<u8>,
    regions: &[BoardRegion],
    boards: &[Board],
    positions: &[usize],
    source: &EmbedSource,
    layout: Layout,
) -> Result<Rendered, RenderError> {
    if positions.len() != boards.len() {
        return Err(payload_error("one source position is needed per board"));
    }
    let mut doc = lopdf::Document::load_mem(&pdf)
        .map_err(|e| payload_error(&format!("could not read the PDF: {e}")))?;
    let pages: Vec<ObjectId> = doc.get_pages().into_values().collect();
    let page_sizes: Vec<[f32; 2]> = pages.iter().map(|&id| page_size(&doc, id)).collect();

    // Boards are numbered in the order they first appear. A board that drew
    // nothing (a page-break marker, an empty record) has no figure and is left
    // out of the hands file, so the join covers exactly what is on the page.
    let mut numbers: BTreeMap<usize, usize> = BTreeMap::new();
    let mut order: Vec<usize> = Vec::new();
    for region in regions {
        numbers.entry(region.board).or_insert_with(|| {
            order.push(region.board);
            order.len()
        });
    }

    for (index, region) in regions.iter().enumerate() {
        let &page = pages
            .get(region.page)
            .ok_or_else(|| payload_error("a board region names a page the PDF lacks"))?;
        add_link(&mut doc, page, index, points(region.rect))?;
    }

    let click_map = click_map(regions, boards, &numbers, &page_sizes);
    let hands = hands_file(source, positions, &order)?;
    let provenance = provenance(source, layout);

    let attachments = [
        (
            source.name,
            "Source",
            "application/x-pbn",
            "The PBN file this PDF was rendered from.",
            source.text.as_bytes().to_vec(),
        ),
        (
            PROVENANCE_FILE,
            "Supplement",
            "application/json",
            "What produced this PDF.",
            provenance.into_bytes(),
        ),
        (
            BLOCKS_FILE,
            "Data",
            "application/json",
            "Page and position of each board.",
            click_map.clone().into_bytes(),
        ),
        (
            HANDS_FILE,
            "Data",
            "application/x-pbn",
            "The rendered boards as PBN, numbered from 1.",
            hands.into_bytes(),
        ),
    ];
    let specs: Vec<(String, ObjectId)> = attachments
        .into_iter()
        .map(|(name, relationship, mime, description, bytes)| {
            let id = add_attachment(&mut doc, name, relationship, mime, description, bytes);
            (name.to_string(), id)
        })
        .collect();
    attach_to_catalog(&mut doc, specs)?;

    let mut output = Cursor::new(Vec::new());
    doc.save_to(&mut output)
        .map_err(|e| payload_error(&format!("could not save the PDF: {e}")))?;
    Ok(Rendered {
        pdf: output.into_inner(),
        click_map,
    })
}

fn payload_error(message: &str) -> RenderError {
    RenderError::Payload(message.to_string())
}

/// A page's `[width, height]` in points, from its MediaBox (letter if absent).
fn page_size(doc: &lopdf::Document, page: ObjectId) -> [f32; 2] {
    let media_box = doc
        .get_dictionary(page)
        .ok()
        .and_then(|dict| dict.get(b"MediaBox").ok())
        .and_then(|object| doc.dereference(object).ok())
        .and_then(|(_, object)| object.as_array().ok())
        .map(|values| {
            values
                .iter()
                .filter_map(|v| {
                    v.as_float()
                        .ok()
                        .or_else(|| v.as_i64().ok().map(|i| i as f32))
                })
                .collect::<Vec<f32>>()
        });
    match media_box.as_deref() {
        Some([x1, y1, x2, y2]) => [x2 - x1, y2 - y1],
        _ => [612.0, 792.0],
    }
}

/// A region's rect in points, rounded to hundredths so the payload is short and
/// the annotation and the click map agree exactly.
fn points(rect: [f32; 4]) -> [f32; 4] {
    rect.map(|mm| (mm * PT_PER_MM * 100.0).round() / 100.0)
}

/// One `lesson-block:<index>` link over a figure, added to the page's
/// `/Annots`. Borderless, so ordinary viewers draw nothing.
fn add_link(
    doc: &mut lopdf::Document,
    page: ObjectId,
    index: usize,
    rect: [f32; 4],
) -> Result<(), RenderError> {
    let uri = format!("{BLOCK_URI_SCHEME}:{index}");
    let annotation = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Link",
        "Rect" => rect.iter().map(|&v| Object::Real(v)).collect::<Vec<_>>(),
        "Border" => vec![0.into(), 0.into(), 0.into()],
        "A" => dictionary! {
            "Type" => "Action",
            "S" => "URI",
            "URI" => Object::String(uri.into_bytes(), StringFormat::Literal),
        },
    });
    let page = doc
        .get_dictionary_mut(page)
        .map_err(|e| payload_error(&format!("could not read a page: {e}")))?;
    match page.get_mut(b"Annots") {
        Ok(Object::Array(annots)) => annots.push(Object::Reference(annotation)),
        // An indirect /Annots array would need resolving; printpdf never
        // writes one, so a fresh array replacing it would lose nothing it made.
        _ => page.set("Annots", vec![Object::Reference(annotation)]),
    }
    Ok(())
}

/// An embedded file and its filespec. Returns the filespec's id.
fn add_attachment(
    doc: &mut lopdf::Document,
    name: &str,
    relationship: &str,
    mime: &str,
    description: &str,
    bytes: Vec<u8>,
) -> ObjectId {
    let size = bytes.len() as i64;
    let mut stream = Stream::new(
        dictionary! {
            "Type" => "EmbeddedFile",
            "Subtype" => Object::Name(mime.as_bytes().to_vec()),
            "Params" => dictionary! { "Size" => size },
        },
        bytes,
    );
    // Small files may not shrink; compress() then leaves them as they are.
    let _ = stream.compress();
    let file = doc.add_object(stream);
    doc.add_object(dictionary! {
        "Type" => "Filespec",
        "F" => text_string(name),
        "UF" => text_string(name),
        "Desc" => text_string(description),
        "AFRelationship" => Object::Name(relationship.as_bytes().to_vec()),
        "EF" => dictionary! { "F" => Object::Reference(file) },
    })
}

/// A PDF text string: plain bytes when ASCII, else UTF-16BE with a BOM.
fn text_string(text: &str) -> Object {
    if text.is_ascii() {
        Object::String(text.as_bytes().to_vec(), StringFormat::Literal)
    } else {
        let mut bytes = vec![0xFE, 0xFF];
        bytes.extend(text.encode_utf16().flat_map(u16::to_be_bytes));
        Object::String(bytes, StringFormat::Hexadecimal)
    }
}

/// List the filespecs in the catalog's `/AF` array and its `/EmbeddedFiles`
/// name tree (one flat leaf, keys sorted as a name tree requires).
fn attach_to_catalog(
    doc: &mut lopdf::Document,
    mut specs: Vec<(String, ObjectId)>,
) -> Result<(), RenderError> {
    let af: Vec<Object> = specs.iter().map(|&(_, id)| Object::Reference(id)).collect();
    specs.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let names: Vec<Object> = specs
        .into_iter()
        .flat_map(|(name, id)| [text_string(&name), Object::Reference(id)])
        .collect();

    let catalog = doc
        .catalog_mut()
        .map_err(|e| payload_error(&format!("could not read the catalog: {e}")))?;
    catalog.set("AF", af);
    let embedded = dictionary! { "Names" => names };
    match catalog.get_mut(b"Names") {
        Ok(Object::Dictionary(existing)) => existing.set("EmbeddedFiles", embedded),
        _ => catalog.set("Names", dictionary! { "EmbeddedFiles" => embedded }),
    }
    Ok(())
}

/// `lesson-blocks.json`. One block per region, in drawing order.
fn click_map(
    regions: &[BoardRegion],
    boards: &[Board],
    numbers: &BTreeMap<usize, usize>,
    page_sizes: &[[f32; 2]],
) -> String {
    let sizes: Vec<String> = page_sizes
        .iter()
        .map(|[w, h]| format!("[{},{}]", number(*w), number(*h)))
        .collect();
    let blocks: Vec<String> = regions
        .iter()
        .enumerate()
        .map(|(index, region)| {
            let rect = points(region.rect).map(number).join(",");
            let mut block = format!(
                "{{\"index\":{index},\"kind\":\"board\",\"page\":{},\"rect\":[{rect}],\"board\":{}",
                region.page + 1,
                numbers[&region.board],
            );
            if let Some(label) = source_board(&boards[region.board]) {
                block.push_str(&format!(",\"sourceBoard\":{}", json_string(&label)));
            }
            block.push('}');
            block
        })
        .collect();
    format!(
        "{{\"version\":{PAYLOAD_VERSION},\"pageCount\":{},\"pageSize\":[{}],\
         \"coordinateSpace\":\"pdf-points, origin bottom-left\",\"blocks\":[{}],\
         \"unlocated\":[],\"fragmented\":[]}}\n",
        page_sizes.len(),
        sizes.join(","),
        blocks.join(","),
    )
}

/// A board's original `[Board]` value, as the page labels it.
fn source_board(board: &Board) -> Option<String> {
    board
        .board_id
        .clone()
        .or_else(|| board.number.map(|n| n.to_string()))
}

/// `lesson-hands.pbn`: each rendered board's record copied from the source
/// verbatim, its `[Board]` renumbered to its place in the PDF.
fn hands_file(
    source: &EmbedSource,
    positions: &[usize],
    order: &[usize],
) -> Result<String, RenderError> {
    let mut document = PbnDocument::parse(source.text)
        .map_err(|e| payload_error(&format!("could not re-read the PBN source: {e}")))?;
    let mut hands = String::new();
    for (i, &board) in order.iter().enumerate() {
        let position = positions[board];
        document
            .set_tag(position, "Board", &(i + 1).to_string())
            .map_err(|e| payload_error(&format!("could not renumber a board: {e}")))?;
        let record = document
            .record(position)
            .ok_or_else(|| payload_error("a board position is past the end of the source"))?;
        hands.push_str(record);
        // The file's last record ends however the file does; keep records
        // apart with the blank line PBN separates games by.
        if !hands.ends_with('\n') {
            hands.push('\n');
        }
        if !hands.ends_with("\n\n") && !hands.ends_with("\n\r\n") {
            hands.push('\n');
        }
    }
    Ok(hands)
}

/// `lesson-provenance.json`. No `renderedAt`: output must stay byte-identical
/// for identical input.
fn provenance(source: &EmbedSource, layout: Layout) -> String {
    format!(
        "{{\"generator\":\"pbn-to-pdf\",\"generatorVersion\":{},\"producer\":{},\
         \"sourceFormat\":\"pbn\",\"sourceFile\":{},\"layout\":\"{}\"}}\n",
        json_string(env!("CARGO_PKG_VERSION")),
        json_string(PRODUCER),
        json_string(source.name),
        layout.as_str(),
    )
}

/// A number as JSON: at most two decimals, no trailing zeros.
fn number(value: f32) -> String {
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_short_and_stable() {
        assert_eq!(number(612.0), "612");
        assert_eq!(number(42.5), "42.5");
        assert_eq!(number(1.006), "1.01");
        assert_eq!(number(-0.001), "0");
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(json_string("a \"b\"\\\n"), "\"a \\\"b\\\"\\\\\\n\"");
        assert_eq!(json_string("Kantar \u{2660}"), "\"Kantar \u{2660}\"");
    }

    #[test]
    fn non_ascii_names_are_utf16() {
        let Object::String(bytes, StringFormat::Hexadecimal) = text_string("é") else {
            panic!("expected a hex string");
        };
        assert_eq!(bytes, vec![0xFE, 0xFF, 0x00, 0xE9]);
    }
}
