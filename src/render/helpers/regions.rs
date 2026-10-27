//! Where each board landed on the page, as the layout drew it.
//!
//! Contract 5's click map and `lesson-block:` link annotations need every
//! board's page and rectangle. A browser-printed lesson has to recover those
//! from the print engine afterwards; here the layout *is* the print engine, so
//! each one records the rectangle it already computed as it draws.

/// One board figure on one page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoardRegion {
    /// Index into the slice of boards the layout was given.
    pub board: usize,
    /// 0-based page index.
    pub page: usize,
    /// `[left, bottom, right, top]` in mm, origin bottom-left -- the same space
    /// every layout draws in.
    pub rect: [f32; 4],
}

/// The regions a layout records as it draws, in drawing order.
///
/// Drawing order is reading order for every layout, which is what the click
/// map's block indices follow. A board may appear more than once: bidding
/// sheets show each board on four pages, and each appearance is its own figure.
#[derive(Debug, Default)]
pub struct Regions(Vec<BoardRegion>);

impl Regions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record `board` on `page`, given its top-left corner and size in mm.
    pub fn push(&mut self, board: usize, page: usize, x: f32, top: f32, width: f32, height: f32) {
        self.0.push(BoardRegion {
            board,
            page,
            rect: [x, top - height, x + width, top],
        });
    }

    pub fn into_vec(self) -> Vec<BoardRegion> {
        self.0
    }
}
