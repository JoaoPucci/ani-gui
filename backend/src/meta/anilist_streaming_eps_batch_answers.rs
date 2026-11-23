//! What the pages of one id space answered for each key a batch asked,
//! and the single lookups a batch falls back to. Split from
//! `anilist_streaming_eps_batch.rs` so each file stays inside the CRAP
//! gate's per-file bar.

use std::collections::{HashMap, HashSet};
use std::future::Future;

use super::{Keyed, Space, StreamingListing, Want};
use crate::error::{AniError, Result};
use crate::meta::anilist_pages::{whole_pages, Page, PAGE_CAP};

/// What the pages of one id space said about each key asked.
#[derive(Default)]
pub(super) struct Answers {
    found: HashMap<u32, StreamingListing>,
    /// Keys only a single lookup can settle.
    single: HashSet<u32>,
    failed: HashMap<u32, AniError>,
}

/// What one key reads as.
pub(super) enum Got {
    Listing(StreamingListing),
    Failed(AniError),
    Single,
    /// The page answered without it.
    Absent,
}

impl Answers {
    pub(super) fn get(&self, key: u32) -> Got {
        if let Some(l) = self.found.get(&key) {
            return Got::Listing(l.clone());
        }
        if let Some(e) = self.failed.get(&key) {
            return Got::Failed(e.clone());
        }
        if self.single.contains(&key) {
            return Got::Single;
        }
        Got::Absent
    }
}

/// Ask `keys` in `space`, a walk of whole pages per fifty ids: a walk's
/// failure answers its own keys only.
pub(super) async fn answers<P, PF>(space: Space, keys: &[u32], ask: &mut P) -> Answers
where
    P: FnMut(Space, Vec<u32>) -> PF,
    PF: Future<Output = Result<Page<Keyed>>>,
{
    let mut out = Answers::default();
    for chunk in keys.chunks(PAGE_CAP) {
        let mut alone = Vec::new();
        let walked = whole_pages(
            chunk,
            |ids| ask(space, ids.to_vec()),
            |one| {
                alone.push(one);
                Ok(())
            },
        )
        .await;
        match walked {
            Ok(pages) => {
                for (once, several) in pages {
                    out.found.extend(once);
                    out.single.extend(several);
                }
                out.single.extend(alone);
            }
            Err(e) => out.failed.extend(chunk.iter().map(|k| (*k, e.clone()))),
        }
    }
    out
}

/// `want`'s single lookup, asked once however many shows need it.
pub(super) async fn single_once<S, SF>(
    singles: &mut HashMap<Want, Result<StreamingListing>>,
    single: &mut S,
    want: Want,
) -> Result<StreamingListing>
where
    S: FnMut(Want) -> SF,
    SF: Future<Output = Result<StreamingListing>>,
{
    if let Some(done) = singles.get(&want) {
        return done.clone();
    }
    let got = single(want).await;
    singles.insert(want, got.clone());
    got
}
