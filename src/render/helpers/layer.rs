//! Compatibility layer for printpdf 0.8
//!
//! This module provides a `LayerBuilder` that mimics the old `PdfLayerReference` API
//! but collects operations into a `Vec<Op>` for the new printpdf 0.8 API.

use std::borrow::Cow;

use super::text_metrics::{winansi_byte, winansi_char};
use printpdf::{
    BuiltinFont, Color, CurTransMat, DictItem, FontId, LinePoint, Mm, Op, PaintMode, PdfFontHandle,
    PdfSaveOptions, Point, Polygon, PolygonRing, Pt, TextItem, WindingOrder, XObjectId,
    XObjectTransform,
};

/// Save options for every document.
///
/// `secure: false` lets through the raw `Tj` that [`LayerBuilder::use_text_builtin`]
/// writes for text past ASCII; printpdf's default drops unknown operators.
/// Nothing else in printpdf reads the flag.
pub fn save_options() -> PdfSaveOptions {
    PdfSaveOptions {
        secure: false,
        ..PdfSaveOptions::default()
    }
}

/// A builder that collects PDF operations
///
/// This mimics the old `PdfLayerReference` API from printpdf 0.7
/// but internally builds a `Vec<Op>` for printpdf 0.8
#[derive(Default)]
pub struct LayerBuilder {
    ops: Vec<Op>,
}

impl LayerBuilder {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    /// Get the collected operations
    pub fn into_ops(self) -> Vec<Op> {
        self.ops
    }

    /// Get a reference to the operations (for inspection)
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    /// Extend with operations from another builder
    pub fn extend(&mut self, other: LayerBuilder) {
        self.ops.extend(other.ops);
    }

    /// Set the fill color
    pub fn set_fill_color(&mut self, color: Color) {
        self.ops.push(Op::SetFillColor { col: color });
    }

    /// Set the outline/stroke color
    pub fn set_outline_color(&mut self, color: Color) {
        self.ops.push(Op::SetOutlineColor { col: color });
    }

    /// Set the outline thickness
    pub fn set_outline_thickness(&mut self, thickness: f32) {
        self.ops.push(Op::SetOutlineThickness { pt: Pt(thickness) });
    }

    /// Draw text at a specific position using an external (embedded) font
    ///
    /// This mimics the old `layer.use_text()` API
    pub fn use_text<S: Into<String>>(
        &mut self,
        text: S,
        font_size: f32,
        x: Mm,
        y: Mm,
        font: &FontId,
    ) {
        self.use_text_with_handle(text, font_size, x, y, PdfFontHandle::External(font.clone()));
    }

    /// Draw text at a specific position using a builtin PDF font
    ///
    /// Uses the Standard 14 PDF fonts (Times-Roman, Helvetica, etc.)
    /// which don't need to be embedded in the PDF.
    ///
    /// Builtin fonts use WinAnsiEncoding (Windows-1252). Characters it can
    /// encode are drawn as themselves; the rest fall back to an ASCII
    /// look-alike (see `winansi_char`).
    pub fn use_text_builtin<S: Into<String>>(
        &mut self,
        text: S,
        font_size: f32,
        x: Mm,
        y: Mm,
        font: BuiltinFont,
    ) {
        let text_str = text.into();
        let sanitized = sanitize_for_winansi(&text_str);
        if sanitized.is_ascii() {
            self.use_text_with_handle(sanitized, font_size, x, y, PdfFontHandle::Builtin(font));
            return;
        }
        if sanitized.is_empty() {
            return;
        }

        // printpdf hands builtin-font text to lopdf as UTF-8 bytes, which the
        // viewer then reads as WinAnsi: a bullet prints as "â€¢". So past ASCII
        // we encode Windows-1252 ourselves and show it with a raw `Tj` -- the
        // operator, and the hex form, printpdf itself writes for such bytes.
        let bytes: Vec<u8> = sanitized.chars().filter_map(winansi_byte).collect();
        self.ops.push(Op::StartTextSection);
        self.ops.push(Op::SetTextCursor {
            pos: Point {
                x: x.into(),
                y: y.into(),
            },
        });
        self.ops.push(Op::SetFont {
            size: Pt(font_size),
            font: PdfFontHandle::Builtin(font),
        });
        self.ops.push(Op::Unknown {
            key: "Tj".to_string(),
            value: vec![DictItem::String {
                data: bytes,
                literal: false,
            }],
        });
        self.ops.push(Op::EndTextSection);
    }

    /// Draw text at a specific position using any font handle
    fn use_text_with_handle<S: Into<String>>(
        &mut self,
        text: S,
        font_size: f32,
        x: Mm,
        y: Mm,
        font: PdfFontHandle,
    ) {
        let text_str = text.into();
        if text_str.is_empty() {
            return;
        }

        self.ops.push(Op::StartTextSection);
        self.ops.push(Op::SetTextCursor {
            pos: Point {
                x: x.into(),
                y: y.into(),
            },
        });
        self.ops.push(Op::SetFont {
            size: Pt(font_size),
            font,
        });
        self.ops.push(Op::ShowText {
            items: vec![TextItem::Text(text_str)],
        });
        self.ops.push(Op::EndTextSection);
    }

    /// Add a filled or stroked rectangle
    ///
    /// Takes lower-left x, y and upper-right x, y coordinates with a paint mode
    pub fn add_rect(&mut self, x1: Mm, y1: Mm, x2: Mm, y2: Mm, mode: PaintMode) {
        let ll = Point {
            x: x1.into(),
            y: y1.into(),
        };
        let lr = Point {
            x: x2.into(),
            y: y1.into(),
        };
        let ur = Point {
            x: x2.into(),
            y: y2.into(),
        };
        let ul = Point {
            x: x1.into(),
            y: y2.into(),
        };

        let points = vec![
            LinePoint {
                p: ll,
                bezier: false,
            },
            LinePoint {
                p: lr,
                bezier: false,
            },
            LinePoint {
                p: ur,
                bezier: false,
            },
            LinePoint {
                p: ul,
                bezier: false,
            },
        ];

        let polygon = Polygon {
            rings: vec![PolygonRing { points }],
            mode,
            winding_order: WindingOrder::NonZero,
        };

        self.ops.push(Op::DrawPolygon { polygon });
    }

    /// Save graphics state
    pub fn save_graphics_state(&mut self) {
        self.ops.push(Op::SaveGraphicsState);
    }

    /// Restore graphics state
    pub fn restore_graphics_state(&mut self) {
        self.ops.push(Op::RestoreGraphicsState);
    }

    /// Set the current transformation matrix
    ///
    /// This applies a transformation to all subsequent drawing operations
    /// until the graphics state is restored.
    pub fn set_transform(&mut self, matrix: CurTransMat) {
        self.ops.push(Op::SetTransformationMatrix { matrix });
    }

    /// Draw a line from (x1, y1) to (x2, y2)
    pub fn add_line(&mut self, x1: Mm, y1: Mm, x2: Mm, y2: Mm) {
        let points = vec![
            LinePoint {
                p: Point {
                    x: x1.into(),
                    y: y1.into(),
                },
                bezier: false,
            },
            LinePoint {
                p: Point {
                    x: x2.into(),
                    y: y2.into(),
                },
                bezier: false,
            },
        ];

        let polygon = Polygon {
            rings: vec![PolygonRing { points }],
            mode: PaintMode::Stroke,
            winding_order: WindingOrder::NonZero,
        };

        self.ops.push(Op::DrawPolygon { polygon });
    }

    /// Place an XObject (SVG/image) with the given transform
    ///
    /// The transform specifies position, scale, rotation, etc.
    /// Use `PdfDocument::add_xobject()` to register an SVG and get the XObjectId.
    pub fn use_xobject(&mut self, id: XObjectId, transform: XObjectTransform) {
        self.ops.push(Op::UseXobject { id, transform });
    }

    /// Begin a rectangular clipping region
    ///
    /// All drawing operations after this call will be clipped to the specified rectangle.
    /// Call `end_clip()` to restore the previous graphics state and end clipping.
    pub fn begin_clip_rect(&mut self, x: Mm, y: Mm, width: Mm, height: Mm) {
        self.save_graphics_state();

        // Create rectangular clipping path
        let points = vec![
            LinePoint {
                p: Point {
                    x: x.into(),
                    y: y.into(),
                },
                bezier: false,
            },
            LinePoint {
                p: Point {
                    x: (Mm(x.0 + width.0)).into(),
                    y: y.into(),
                },
                bezier: false,
            },
            LinePoint {
                p: Point {
                    x: (Mm(x.0 + width.0)).into(),
                    y: (Mm(y.0 + height.0)).into(),
                },
                bezier: false,
            },
            LinePoint {
                p: Point {
                    x: x.into(),
                    y: (Mm(y.0 + height.0)).into(),
                },
                bezier: false,
            },
        ];

        let polygon = Polygon {
            rings: vec![PolygonRing { points }],
            mode: PaintMode::Clip,
            winding_order: WindingOrder::NonZero,
        };

        self.ops.push(Op::DrawPolygon { polygon });
    }

    /// End a clipping region
    ///
    /// Restores the graphics state to before `begin_clip_rect()` was called.
    pub fn end_clip(&mut self) {
        self.restore_graphics_state();
    }

    /// Draw a circle
    ///
    /// Uses Bezier curves to approximate a circle.
    /// center_x, center_y: center of the circle in mm
    /// radius: radius of the circle in mm
    pub fn add_circle(&mut self, center_x: Mm, center_y: Mm, radius: Mm, mode: PaintMode) {
        self.add_ellipse(center_x, center_y, radius, radius, mode);
    }

    /// Draw an ellipse
    ///
    /// Uses Bezier curves to approximate an ellipse.
    /// center_x, center_y: center of the ellipse in mm
    /// radius_x: horizontal radius in mm
    /// radius_y: vertical radius in mm
    pub fn add_ellipse(
        &mut self,
        center_x: Mm,
        center_y: Mm,
        radius_x: Mm,
        radius_y: Mm,
        mode: PaintMode,
    ) {
        // Approximate an ellipse using 4 cubic Bezier curves
        // The magic number for a good ellipse approximation is k = 4 * (sqrt(2) - 1) / 3 ≈ 0.5522848
        let k = 0.552_284_8_f32;

        let cx = center_x.0;
        let cy = center_y.0;
        let rx = radius_x.0;
        let ry = radius_y.0;
        let krx = k * rx;
        let kry = k * ry;

        // Four points on the ellipse (right, top, left, bottom)
        let right = (cx + rx, cy);
        let top = (cx, cy + ry);
        let left = (cx - rx, cy);
        let bottom = (cx, cy - ry);

        // Control points for each curve
        // From right to top
        let cp1_rt = (cx + rx, cy + kry);
        let cp2_rt = (cx + krx, cy + ry);

        // From top to left
        let cp1_tl = (cx - krx, cy + ry);
        let cp2_tl = (cx - rx, cy + kry);

        // From left to bottom
        let cp1_lb = (cx - rx, cy - kry);
        let cp2_lb = (cx - krx, cy - ry);

        // From bottom to right
        let cp1_br = (cx + krx, cy - ry);
        let cp2_br = (cx + rx, cy - kry);

        let points = vec![
            // Start at right point
            LinePoint {
                p: Point {
                    x: Mm(right.0).into(),
                    y: Mm(right.1).into(),
                },
                bezier: false,
            },
            // Curve to top
            LinePoint {
                p: Point {
                    x: Mm(cp1_rt.0).into(),
                    y: Mm(cp1_rt.1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(cp2_rt.0).into(),
                    y: Mm(cp2_rt.1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(top.0).into(),
                    y: Mm(top.1).into(),
                },
                bezier: true,
            },
            // Curve to left
            LinePoint {
                p: Point {
                    x: Mm(cp1_tl.0).into(),
                    y: Mm(cp1_tl.1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(cp2_tl.0).into(),
                    y: Mm(cp2_tl.1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(left.0).into(),
                    y: Mm(left.1).into(),
                },
                bezier: true,
            },
            // Curve to bottom
            LinePoint {
                p: Point {
                    x: Mm(cp1_lb.0).into(),
                    y: Mm(cp1_lb.1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(cp2_lb.0).into(),
                    y: Mm(cp2_lb.1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(bottom.0).into(),
                    y: Mm(bottom.1).into(),
                },
                bezier: true,
            },
            // Curve back to right
            LinePoint {
                p: Point {
                    x: Mm(cp1_br.0).into(),
                    y: Mm(cp1_br.1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(cp2_br.0).into(),
                    y: Mm(cp2_br.1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(right.0).into(),
                    y: Mm(right.1).into(),
                },
                bezier: true,
            },
        ];

        let polygon = Polygon {
            rings: vec![PolygonRing { points }],
            mode,
            winding_order: WindingOrder::NonZero,
        };

        self.ops.push(Op::DrawPolygon { polygon });
    }

    /// Draw a rotated ellipse
    ///
    /// Uses Bezier curves to approximate an ellipse, then rotates all points.
    /// center_x, center_y: center of the ellipse in mm
    /// radius_x: horizontal radius in mm (before rotation)
    /// radius_y: vertical radius in mm (before rotation)
    /// rotation_degrees: rotation angle in degrees (counter-clockwise)
    pub fn add_rotated_ellipse(
        &mut self,
        center_x: Mm,
        center_y: Mm,
        radius_x: Mm,
        radius_y: Mm,
        rotation_degrees: f32,
        mode: PaintMode,
    ) {
        // If no rotation, use the simpler method
        if rotation_degrees.abs() < 0.001 {
            self.add_ellipse(center_x, center_y, radius_x, radius_y, mode);
            return;
        }

        // Approximate an ellipse using 4 cubic Bezier curves
        let k = 0.552_284_8_f32;

        let cx = center_x.0;
        let cy = center_y.0;
        let rx = radius_x.0;
        let ry = radius_y.0;
        let krx = k * rx;
        let kry = k * ry;

        // Four points on the ellipse (right, top, left, bottom) - before rotation
        let unrotated_points: [(f32, f32); 13] = [
            (rx, 0.0),   // right (start)
            (rx, kry),   // cp1_rt
            (krx, ry),   // cp2_rt
            (0.0, ry),   // top
            (-krx, ry),  // cp1_tl
            (-rx, kry),  // cp2_tl
            (-rx, 0.0),  // left
            (-rx, -kry), // cp1_lb
            (-krx, -ry), // cp2_lb
            (0.0, -ry),  // bottom
            (krx, -ry),  // cp1_br
            (rx, -kry),  // cp2_br
            (rx, 0.0),   // right (end)
        ];

        // Rotate all points around center
        let angle_rad = rotation_degrees.to_radians();
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        let rotate_point = |x: f32, y: f32| -> (f32, f32) {
            let rotated_x = x * cos_a - y * sin_a;
            let rotated_y = x * sin_a + y * cos_a;
            (cx + rotated_x, cy + rotated_y)
        };

        let rotated: Vec<(f32, f32)> = unrotated_points
            .iter()
            .map(|(x, y)| rotate_point(*x, *y))
            .collect();

        let points = vec![
            // Start at right point
            LinePoint {
                p: Point {
                    x: Mm(rotated[0].0).into(),
                    y: Mm(rotated[0].1).into(),
                },
                bezier: false,
            },
            // Curve to top
            LinePoint {
                p: Point {
                    x: Mm(rotated[1].0).into(),
                    y: Mm(rotated[1].1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(rotated[2].0).into(),
                    y: Mm(rotated[2].1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(rotated[3].0).into(),
                    y: Mm(rotated[3].1).into(),
                },
                bezier: true,
            },
            // Curve to left
            LinePoint {
                p: Point {
                    x: Mm(rotated[4].0).into(),
                    y: Mm(rotated[4].1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(rotated[5].0).into(),
                    y: Mm(rotated[5].1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(rotated[6].0).into(),
                    y: Mm(rotated[6].1).into(),
                },
                bezier: true,
            },
            // Curve to bottom
            LinePoint {
                p: Point {
                    x: Mm(rotated[7].0).into(),
                    y: Mm(rotated[7].1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(rotated[8].0).into(),
                    y: Mm(rotated[8].1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(rotated[9].0).into(),
                    y: Mm(rotated[9].1).into(),
                },
                bezier: true,
            },
            // Curve back to right
            LinePoint {
                p: Point {
                    x: Mm(rotated[10].0).into(),
                    y: Mm(rotated[10].1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(rotated[11].0).into(),
                    y: Mm(rotated[11].1).into(),
                },
                bezier: true,
            },
            LinePoint {
                p: Point {
                    x: Mm(rotated[12].0).into(),
                    y: Mm(rotated[12].1).into(),
                },
                bezier: true,
            },
        ];

        let polygon = Polygon {
            rings: vec![PolygonRing { points }],
            mode,
            winding_order: WindingOrder::NonZero,
        };

        self.ops.push(Op::DrawPolygon { polygon });
    }
}

/// Sanitize text for WinAnsiEncoding (Windows-1252) used by PDF builtin fonts.
///
/// Converts Unicode characters to their Windows-1252 equivalents where possible,
/// or falls back to ASCII approximations for characters not in the encoding.
fn sanitize_for_winansi(text: &str) -> Cow<'_, str> {
    if text.chars().all(|c| winansi_char(c) == Some(c)) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(text.chars().filter_map(winansi_char).collect())
}

#[cfg(test)]
mod tests {
    use super::sanitize_for_winansi;

    #[test]
    fn windows_1252_text_passes_through_unchanged() {
        // Grant Robinson's slides: bullets, en dashes, curly quotes, an ellipsis
        let text = "\u{2022} Two-over-one \u{2013} \u{201C}no man\u{2019}s land\u{201D}\u{2026} caf\u{00E9}";
        assert_eq!(sanitize_for_winansi(text), text);
    }

    #[test]
    fn characters_outside_windows_1252_fall_back() {
        assert_eq!(sanitize_for_winansi("6\u{202F}pts"), "6 pts");
        assert_eq!(sanitize_for_winansi("1\u{2660}"), "1");
    }
}
