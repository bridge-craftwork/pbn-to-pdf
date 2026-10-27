//! Contract 5, `pbn` profile: what a rendered PDF carries inside itself.
//!
//! The contract is lesson-stage's `docs/contracts/pdf-attachments.md`. These
//! tests read the PDF back the way a consumer would: the four attachments by
//! `AFRelationship`, the click map, and the `lesson-block:` link annotations,
//! which must carry the same rects as the map.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use lopdf::{Document, Object};
use pbn_to_pdf::{parse_pbn, render_boards_embedded, EmbedSource, Layout, RenderOptions};
use serde_json::Value;

fn fixture(name: &str) -> (String, String) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    (name.to_string(), fs::read_to_string(path).unwrap())
}

fn metadata(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.starts_with('%'))
        .map(String::from)
        .collect()
}

/// Every attachment, by filename: (AFRelationship, decoded bytes).
fn attachments(doc: &Document) -> BTreeMap<String, (String, Vec<u8>)> {
    let catalog = doc.catalog().unwrap();
    let names = catalog.get(b"Names").unwrap().as_dict().unwrap();
    let tree = names.get(b"EmbeddedFiles").unwrap().as_dict().unwrap();
    let leaf = tree.get(b"Names").unwrap().as_array().unwrap();
    let af = catalog.get(b"AF").unwrap().as_array().unwrap();
    assert_eq!(af.len() * 2, leaf.len(), "/AF lists every attached file");

    let mut keys = Vec::new();
    let mut files = BTreeMap::new();
    for pair in leaf.chunks(2) {
        let key = String::from_utf8(pair[0].as_str().unwrap().to_vec()).unwrap();
        keys.push(key.clone());
        let spec = doc.get_dictionary(pair[1].as_reference().unwrap()).unwrap();
        let relationship = spec.get(b"AFRelationship").unwrap().as_name().unwrap();
        let ef = spec.get(b"EF").unwrap().as_dict().unwrap();
        let stream = doc
            .get_object(ef.get(b"F").unwrap().as_reference().unwrap())
            .unwrap()
            .as_stream()
            .unwrap();
        let bytes = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        files.insert(
            key,
            (String::from_utf8(relationship.to_vec()).unwrap(), bytes),
        );
    }
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "a name tree's keys are sorted");
    files
}

/// Every `lesson-block:` link: (page, index, rect).
fn links(doc: &Document) -> Vec<(usize, usize, Vec<f32>)> {
    let mut found = Vec::new();
    for (page_number, page_id) in doc.get_pages() {
        let page = doc.get_dictionary(page_id).unwrap();
        let Ok(annots) = page.get(b"Annots").and_then(Object::as_array) else {
            continue;
        };
        for annot in annots {
            let annot = doc.get_dictionary(annot.as_reference().unwrap()).unwrap();
            let action = annot.get(b"A").unwrap().as_dict().unwrap();
            let uri =
                String::from_utf8(action.get(b"URI").unwrap().as_str().unwrap().to_vec()).unwrap();
            let index = uri.strip_prefix("lesson-block:").unwrap().parse().unwrap();
            let rect = annot
                .get(b"Rect")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_float().unwrap())
                .collect();
            found.push((page_number as usize, index, rect));
        }
    }
    found
}

#[test]
fn every_layout_carries_the_pbn_profile() {
    let (name, text) = fixture("Drury.pbn");
    let boards = parse_pbn(&text).unwrap().boards;
    let positions: Vec<usize> = (0..boards.len()).collect();
    let source = EmbedSource {
        name: &name,
        text: &text,
    };

    for layout in Layout::ALL {
        let rendered = render_boards_embedded(
            &boards,
            &positions,
            &metadata(&text),
            layout,
            RenderOptions::default(),
            &source,
        )
        .unwrap();
        let doc = Document::load_mem(&rendered.pdf).unwrap();
        let files = attachments(&doc);

        let relationships: Vec<(&str, &str)> = files
            .iter()
            .map(|(name, (rel, _))| (name.as_str(), rel.as_str()))
            .collect();
        assert_eq!(
            relationships,
            vec![
                ("Drury.pbn", "Source"),
                ("lesson-blocks.json", "Data"),
                ("lesson-hands.pbn", "Data"),
                ("lesson-provenance.json", "Supplement"),
            ],
            "{layout}"
        );
        assert_eq!(
            files["Drury.pbn"].1,
            text.as_bytes(),
            "{layout}: source is byte-exact"
        );

        let provenance: Value = serde_json::from_slice(&files["lesson-provenance.json"].1).unwrap();
        assert_eq!(provenance["sourceFormat"], "pbn");
        assert_eq!(provenance["generator"], "pbn-to-pdf");
        assert_eq!(provenance["layout"], layout.as_str());
        assert!(
            provenance.get("renderedAt").is_none(),
            "no clock in the output"
        );

        let map: Value = serde_json::from_slice(&files["lesson-blocks.json"].1).unwrap();
        assert_eq!(
            map.to_string(),
            serde_json::from_str::<Value>(&rendered.click_map)
                .unwrap()
                .to_string()
        );
        assert_eq!(map["version"], 2);
        assert_eq!(map["coordinateSpace"], "pdf-points, origin bottom-left");
        assert_eq!(
            map["pageCount"].as_u64().unwrap() as usize,
            doc.get_pages().len()
        );
        let blocks = map["blocks"].as_array().unwrap();
        assert!(!blocks.is_empty(), "{layout}");

        // The links and the map agree: same count, same page, same rect.
        let links = links(&doc);
        assert_eq!(links.len(), blocks.len(), "{layout}: one link per block");
        for (page, index, rect) in links {
            let block = &blocks[index];
            assert_eq!(block["index"].as_u64().unwrap() as usize, index);
            assert_eq!(block["kind"], "board");
            assert!(block.get("body").is_none(), "the pbn profile has no body");
            assert_eq!(block["page"].as_u64().unwrap() as usize, page, "{layout}");
            let map_rect: Vec<f32> = block["rect"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect();
            assert_eq!(map_rect, rect, "{layout} block {index}");
            let [w, h] = [0, 1].map(|i| map["pageSize"][page - 1][i].as_f64().unwrap() as f32);
            assert!(
                rect[0] >= 0.0 && rect[1] >= 0.0 && rect[2] <= w && rect[3] <= h,
                "{layout} block {index} leaves its page: {rect:?}"
            );
        }

        // Boards are numbered 1..n in order of first appearance, and the
        // hands file holds exactly those records, in that order.
        let mut seen = Vec::new();
        for block in blocks {
            let board = block["board"].as_u64().unwrap();
            if !seen.contains(&board) {
                seen.push(board);
            }
        }
        assert_eq!(
            seen,
            (1..=seen.len() as u64).collect::<Vec<_>>(),
            "{layout}"
        );
        let hands = String::from_utf8(files["lesson-hands.pbn"].1.clone()).unwrap();
        let numbers: Vec<String> = parse_pbn(&hands)
            .unwrap()
            .boards
            .iter()
            .map(|b| b.board_id.clone().unwrap())
            .collect();
        let expected: Vec<String> = seen.iter().map(u64::to_string).collect();
        assert_eq!(numbers, expected, "{layout}");
    }
}

#[test]
fn bidding_sheets_show_each_board_four_times() {
    let (name, text) = fixture("Drury.pbn");
    let boards: Vec<_> = parse_pbn(&text)
        .unwrap()
        .boards
        .into_iter()
        .take(3)
        .collect();
    let rendered = render_boards_embedded(
        &boards,
        &[0, 1, 2],
        &metadata(&text),
        Layout::BiddingSheets,
        RenderOptions::default(),
        &EmbedSource {
            name: &name,
            text: &text,
        },
    )
    .unwrap();
    let map: Value = serde_json::from_str(&rendered.click_map).unwrap();
    let mut appearances = BTreeMap::new();
    for block in map["blocks"].as_array().unwrap() {
        *appearances
            .entry(block["board"].as_u64().unwrap())
            .or_insert(0) += 1;
    }
    assert_eq!(appearances, BTreeMap::from([(1, 4), (2, 4), (3, 4)]));
}

#[test]
fn a_selection_is_renumbered_and_keeps_its_source_numbers() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_pbn-to-pdf"));
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/Drury.pbn");
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/output");
    fs::create_dir_all(&dir).unwrap();

    let out = dir.join("contract5-selection.pdf");
    let status = Command::new(&binary)
        .args(["--layout", "hand-record", "--boards", "3,5"])
        .arg(&input)
        .arg("-o")
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());
    let doc = Document::load_mem(&fs::read(&out).unwrap()).unwrap();
    let files = attachments(&doc);

    let map: Value = serde_json::from_slice(&files["lesson-blocks.json"].1).unwrap();
    let pairs: Vec<(u64, String)> = map["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            (
                b["board"].as_u64().unwrap(),
                b["sourceBoard"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(pairs, vec![(1, "3".to_string()), (2, "5".to_string())]);

    // The records come from the source verbatim, but for the renumbering.
    let hands = String::from_utf8(files["lesson-hands.pbn"].1.clone()).unwrap();
    let source = fs::read_to_string(&input).unwrap();
    let original = parse_pbn(&source).unwrap().boards;
    let embedded = parse_pbn(&hands).unwrap().boards;
    assert_eq!(embedded.len(), 2);
    for (board, number) in embedded.iter().zip(["3", "5"]) {
        let from = original
            .iter()
            .find(|b| b.board_id.as_deref() == Some(number))
            .unwrap();
        assert_eq!(board.deal, from.deal);
        assert_eq!(board.event, from.event);
    }

    // And --no-embed leaves the PDF plain.
    let plain = dir.join("contract5-plain.pdf");
    let status = Command::new(&binary)
        .args(["--layout", "hand-record", "--no-embed"])
        .arg(&input)
        .arg("-o")
        .arg(&plain)
        .status()
        .unwrap();
    assert!(status.success());
    let doc = Document::load_mem(&fs::read(&plain).unwrap()).unwrap();
    assert!(doc.catalog().unwrap().get(b"Names").is_err());
    assert!(links(&doc).is_empty());
}
