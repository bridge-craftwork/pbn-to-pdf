//! PDF post-processing helpers
//!
//! Uses lopdf to repair and compress PDF output after printpdf generates it.

use std::collections::BTreeSet;
use std::io::Cursor;

use lopdf::{dictionary, Dictionary, Object, ObjectId};

/// Repair and compress PDF streams.
///
/// This is a post-processing step needed because printpdf doesn't compress
/// its output streams. We parse the PDF bytes with lopdf, repair Form
/// XObject resources (see [`repair_form_xobject_resources`]), compress all
/// streams, and re-save.
pub fn compress_pdf(uncompressed: Vec<u8>) -> Result<Vec<u8>, String> {
    // Parse the uncompressed PDF
    let mut doc = lopdf::Document::load_mem(&uncompressed)
        .map_err(|e| format!("Failed to parse PDF for compression: {}", e))?;

    repair_form_xobject_resources(&mut doc);

    // Compress all streams
    doc.compress();

    // Save to bytes
    let mut output = Cursor::new(Vec::new());
    doc.save_to(&mut output)
        .map_err(|e| format!("Failed to save compressed PDF: {}", e))?;

    Ok(output.into_inner())
}

/// Resource names a Form XObject's content stream references, grouped by
/// the resource category the referencing operator implies.
#[derive(Default)]
struct ReferencedNames {
    color_spaces: BTreeSet<Vec<u8>>, // `/name cs` or `/name CS`
    ext_g_states: BTreeSet<Vec<u8>>, // `/name gs`
    xobjects: BTreeSet<Vec<u8>>,     // `/name Do`
}

/// Define resources that Form XObject content streams reference but their
/// Resources dictionaries fail to define.
///
/// printpdf's SVG embedding (`Svg::parse`) converts each SVG to PDF via
/// svg2pdf, keeps only the drawing operations, and discards the resource
/// dictionary svg2pdf built alongside them. The resulting Form XObjects
/// select resources such as `/cs0 cs` that are defined nowhere in the file.
/// Lenient viewers (Acrobat, macOS Preview) recover silently, but strict
/// renderers abort the affected drawing blocks, so the SVG artwork
/// disappears from print output.
///
/// The dropped definitions are benign to substitute: svg2pdf's `cs0` is an
/// sRGB color space and every fill/stroke color is re-set with `rg`/`RG`
/// right after selecting it, so DeviceRGB is equivalent; its graphics
/// states and nested XObjects only affect optional refinements, so an empty
/// graphics state and an empty form keep the file valid without changing
/// rendered output.
fn repair_form_xobject_resources(doc: &mut lopdf::Document) {
    // Pass 1: find Form XObjects and the resource names their content uses.
    let mut pending: Vec<(ObjectId, ReferencedNames)> = Vec::new();
    for (&id, object) in &doc.objects {
        let Object::Stream(stream) = object else {
            continue;
        };
        let is_form = stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|o| o.as_name().ok())
            == Some(b"Form".as_slice());
        if !is_form {
            continue;
        }
        let content = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        let names = referenced_resource_names(&content);
        if !names.color_spaces.is_empty()
            || !names.ext_g_states.is_empty()
            || !names.xobjects.is_empty()
        {
            pending.push((id, names));
        }
    }

    if pending.is_empty() {
        return;
    }

    // Placeholder target for references to dropped nested XObjects: an empty
    // form that draws nothing. Created lazily, shared by all repairs.
    let needs_empty_form = pending.iter().any(|(_, n)| !n.xobjects.is_empty());
    let empty_form_id = needs_empty_form.then(|| {
        doc.add_object(Object::Stream(lopdf::Stream::new(
            dictionary! {
                "Type" => Object::Name(b"XObject".to_vec()),
                "Subtype" => Object::Name(b"Form".to_vec()),
                "BBox" => vec![0.into(), 0.into(), 0.into(), 0.into()],
            },
            Vec::new(),
        )))
    });

    // Pass 2: add missing definitions to each Form XObject's Resources.
    for (id, names) in pending {
        // printpdf writes Resources inline in the stream dictionary, but if
        // it is an indirect reference, follow it once.
        let resources_ref = match doc.objects.get(&id) {
            Some(Object::Stream(stream)) => match stream.dict.get(b"Resources") {
                Ok(Object::Reference(target)) => Some(*target),
                _ => None,
            },
            _ => continue,
        };
        let resources = if let Some(target) = resources_ref {
            match doc.objects.get_mut(&target) {
                Some(Object::Dictionary(dict)) => dict,
                _ => continue,
            }
        } else {
            let Some(Object::Stream(stream)) = doc.objects.get_mut(&id) else {
                continue;
            };
            if !matches!(stream.dict.get(b"Resources"), Ok(Object::Dictionary(_))) {
                stream
                    .dict
                    .set("Resources", Object::Dictionary(Dictionary::new()));
            }
            match stream.dict.get_mut(b"Resources") {
                Ok(Object::Dictionary(dict)) => dict,
                _ => continue,
            }
        };

        for name in &names.color_spaces {
            let color_spaces = ensure_subdictionary(resources, "ColorSpace");
            if !color_spaces.has(name) {
                color_spaces.set(name.clone(), Object::Name(b"DeviceRGB".to_vec()));
            }
        }
        for name in &names.ext_g_states {
            let ext_g_states = ensure_subdictionary(resources, "ExtGState");
            if !ext_g_states.has(name) {
                ext_g_states.set(
                    name.clone(),
                    Object::Dictionary(dictionary! {
                        "Type" => Object::Name(b"ExtGState".to_vec()),
                    }),
                );
            }
        }
        for name in &names.xobjects {
            let xobjects = ensure_subdictionary(resources, "XObject");
            if !xobjects.has(name) {
                if let Some(empty_form) = empty_form_id {
                    xobjects.set(name.clone(), Object::Reference(empty_form));
                }
            }
        }
    }
}

/// Get the named sub-dictionary of a Resources dictionary, creating it if
/// absent or malformed. printpdf's SVG embedding writes `ColorSpace` as a
/// bare name where a dictionary belongs, so a non-dictionary entry is
/// replaced rather than preserved.
fn ensure_subdictionary<'a>(resources: &'a mut Dictionary, key: &str) -> &'a mut Dictionary {
    if !matches!(resources.get(key.as_bytes()), Ok(Object::Dictionary(_))) {
        resources.set(key, Object::Dictionary(Dictionary::new()));
    }
    match resources.get_mut(key.as_bytes()) {
        Ok(Object::Dictionary(dict)) => dict,
        _ => unreachable!("entry was just set to a dictionary"),
    }
}

/// Scan a content stream for `/name op` pairs where `op` consumes a named
/// resource: `cs`/`CS` (color space), `gs` (graphics state), `Do` (XObject).
fn referenced_resource_names(content: &[u8]) -> ReferencedNames {
    fn is_regular(byte: u8) -> bool {
        !byte.is_ascii_whitespace() && !b"()<>[]{}/%".contains(&byte)
    }

    let mut names = ReferencedNames::default();
    let mut i = 0;
    while i < content.len() {
        if content[i] != b'/' {
            i += 1;
            continue;
        }
        let name_start = i + 1;
        let mut name_end = name_start;
        while name_end < content.len() && is_regular(content[name_end]) {
            name_end += 1;
        }
        let mut op_start = name_end;
        while op_start < content.len() && content[op_start].is_ascii_whitespace() {
            op_start += 1;
        }
        let mut op_end = op_start;
        while op_end < content.len() && is_regular(content[op_end]) {
            op_end += 1;
        }
        let name = &content[name_start..name_end];
        if !name.is_empty() {
            match &content[op_start..op_end] {
                b"cs" | b"CS" => {
                    names.color_spaces.insert(name.to_vec());
                }
                b"gs" => {
                    names.ext_g_states.insert(name.to_vec());
                }
                b"Do" => {
                    names.xobjects.insert(name.to_vec());
                }
                _ => {}
            }
        }
        i = name_end;
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::Stream;

    fn document_with_form(content: &[u8], resources: Option<Object>) -> lopdf::Document {
        let mut doc = lopdf::Document::with_version("1.5");
        let mut form_dict = dictionary! {
            "Type" => Object::Name(b"XObject".to_vec()),
            "Subtype" => Object::Name(b"Form".to_vec()),
            "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
        };
        if let Some(resources) = resources {
            form_dict.set("Resources", resources);
        }
        let form_id = doc.add_object(Object::Stream(Stream::new(form_dict, content.to_vec())));
        let pages_id = doc.new_object_id();
        let content_id = doc.add_object(Object::Stream(Stream::new(
            dictionary! {},
            b"/F1 Do".to_vec(),
        )));
        let page_id = doc.add_object(dictionary! {
            "Type" => Object::Name(b"Page".to_vec()),
            "Parent" => Object::Reference(pages_id),
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "XObject" => dictionary! { "F1" => Object::Reference(form_id) },
            },
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => Object::Name(b"Pages".to_vec()),
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => Object::Name(b"Catalog".to_vec()),
            "Pages" => Object::Reference(pages_id),
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));
        doc
    }

    fn save_to_bytes(doc: &mut lopdf::Document) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        doc.save_to(&mut bytes).unwrap();
        bytes.into_inner()
    }

    fn form_resources(doc: &lopdf::Document) -> &Dictionary {
        let stream = doc
            .objects
            .values()
            .find_map(|o| match o {
                Object::Stream(s)
                    if s.dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok())
                        == Some(b"Form".as_slice())
                        && !s.content.is_empty() =>
                {
                    Some(s)
                }
                _ => None,
            })
            .expect("form xobject present");
        stream.dict.get(b"Resources").unwrap().as_dict().unwrap()
    }

    #[test]
    fn defines_missing_resources_referenced_by_form_content() {
        let content = b"q\n/cs0 cs\n1 1 1 rg\n/gs0 gs\n/xo0 Do\nQ\n";
        // ColorSpace as a bare name mirrors printpdf's malformed output.
        let resources = dictionary! { "ColorSpace" => Object::Name(b"DeviceRGB".to_vec()) };
        let mut doc = document_with_form(content, Some(Object::Dictionary(resources)));

        let repaired = compress_pdf(save_to_bytes(&mut doc)).unwrap();
        let repaired_doc = lopdf::Document::load_mem(&repaired).unwrap();
        let resources = form_resources(&repaired_doc);

        let color_spaces = resources.get(b"ColorSpace").unwrap().as_dict().unwrap();
        assert_eq!(
            color_spaces.get(b"cs0").unwrap().as_name().unwrap(),
            b"DeviceRGB"
        );
        let ext_g_states = resources.get(b"ExtGState").unwrap().as_dict().unwrap();
        assert!(ext_g_states.get(b"gs0").unwrap().as_dict().is_ok());
        let xobjects = resources.get(b"XObject").unwrap().as_dict().unwrap();
        let placeholder = xobjects.get(b"xo0").unwrap().as_reference().unwrap();
        assert!(matches!(
            repaired_doc.objects.get(&placeholder),
            Some(Object::Stream(_))
        ));
    }

    #[test]
    fn preserves_already_defined_resources() {
        let content = b"q\n/cs0 cs\n0 0 0 rg\nQ\n";
        let resources = dictionary! {
            "ColorSpace" => dictionary! { "cs0" => Object::Name(b"Pattern".to_vec()) },
        };
        let mut doc = document_with_form(content, Some(Object::Dictionary(resources)));

        let repaired = compress_pdf(save_to_bytes(&mut doc)).unwrap();
        let repaired_doc = lopdf::Document::load_mem(&repaired).unwrap();
        let resources = form_resources(&repaired_doc);

        let color_spaces = resources.get(b"ColorSpace").unwrap().as_dict().unwrap();
        assert_eq!(
            color_spaces.get(b"cs0").unwrap().as_name().unwrap(),
            b"Pattern"
        );
    }

    #[test]
    fn creates_resources_dictionary_when_absent() {
        let content = b"q\n/cs0 CS\n0 0 0 RG\nQ\n";
        let mut doc = document_with_form(content, None);

        let repaired = compress_pdf(save_to_bytes(&mut doc)).unwrap();
        let repaired_doc = lopdf::Document::load_mem(&repaired).unwrap();
        let resources = form_resources(&repaired_doc);

        let color_spaces = resources.get(b"ColorSpace").unwrap().as_dict().unwrap();
        assert_eq!(
            color_spaces.get(b"cs0").unwrap().as_name().unwrap(),
            b"DeviceRGB"
        );
    }
}
