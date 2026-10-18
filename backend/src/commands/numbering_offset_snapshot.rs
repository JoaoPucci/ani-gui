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

    /// The store as `body` writes it, for a case that needs no file.
    #[cfg(test)]
    #[must_use]
    pub fn from_body(body: &str) -> Self {
        Self(parse(body))
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

    /// The per-entry (Kitsu-space) reading of a stored row: the
    /// stamped slot presents as its display tag, translated per entry;
    /// everything else translates by the offset. Offset and stamp come
    /// from this one read.
    #[must_use]
    pub fn read_ep_no(&self, slug: &str, ep_no: &str) -> String {
        let offset = self.offset(slug);
        if let Some((slot, tag)) = self.display(slug) {
            if ep_no.trim() == slot.to_string() {
                return super::kitsu_ep_no(&tag, offset);
            }
        }
        super::kitsu_ep_no(ep_no, offset)
    }

    /// The ep_no a listing-less writer (mark-watched, cache-hit)
    /// stores for `episode`, under the offset this read carries: the
    /// offset translation, except when the provider-space value names
    /// the stamped display tag — by numeric identity, since the
    /// frontend normalizes — in which case the slot is written
    /// instead, because that is what a resume looks the row up by in
    /// the provider listing.
    #[must_use]
    pub fn write_ep_no(&self, slug: &str, episode: &str) -> String {
        let provider = super::provider_ep_no(episode, self.offset(slug));
        if let Some((slot, tag)) = self.display(slug) {
            if super::super::play_native_episode::tag_matches(&tag, &provider) {
                return slot.to_string();
            }
        }
        provider
    }
}
