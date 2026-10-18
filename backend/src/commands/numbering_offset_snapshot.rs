//! The numbering store as one read saw it, so everything a reader
//! derives from a show's row comes from the same row. Split from
//! [`super`] for the per-file complexity bar.

use std::path::Path;

use super::numbering_offset_store::{parse, read_store, Row};

/// The numbering store, read once.
pub struct Numbering(Vec<Row>);

impl Numbering {
    /// The store beside the history file at `history_path` as it is
    /// now; empty when it is missing or unreadable.
    #[must_use]
    pub fn read(history_path: &Path) -> Self {
        Self(parse(&read_store(history_path)))
    }

    fn row(&self, slug: &str) -> Option<&Row> {
        self.0.iter().find(|r| r.slug == slug)
    }

    /// The slug's stamped offset; 0 on a missing row, the no-shift
    /// case.
    #[must_use]
    pub fn offset(&self, slug: &str) -> u32 {
        self.row(slug).map_or(0, |r| r.offset)
    }

    /// The slug's (slot, display tag) stamp, when one exists.
    #[must_use]
    pub fn display(&self, slug: &str) -> Option<(u32, String)> {
        self.row(slug)?.display.clone()
    }

    /// The (slot, tag) the display stamp named its row by before a
    /// repair put it right, while the stamp still names that slot.
    #[must_use]
    pub fn former(&self, slug: &str) -> Option<(u32, String)> {
        let row = self.row(slug)?;
        let (slot, _) = row.display.as_ref()?;
        row.former.clone().filter(|(was, _)| was == slot)
    }
}
