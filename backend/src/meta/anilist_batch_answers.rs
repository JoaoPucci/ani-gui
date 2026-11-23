//! What the pages of one id space answered for each key a batch asked,
//! and the single lookups a batch falls back to. Split from
//! `anilist_batch.rs` so each file stays inside the CRAP gate's
//! per-file bar.

use std::collections::{HashMap, HashSet};
use std::future::Future;

use super::{Keyed, Space, Want};
use crate::error::{AniError, Result};
use crate::meta::anilist_pages::{whole_pages, Page, PAGE_CAP};

/// What the pages of one id space said about each key asked.
pub(super) struct Answers<T> {
    found: HashMap<u32, T>,
    /// Keys only a single lookup can settle.
    single: HashSet<u32>,
    failed: HashMap<u32, AniError>,
}

impl<T> Default for Answers<T> {
    fn default() -> Self {
        Self {
            found: HashMap::new(),
            single: HashSet::new(),
            failed: HashMap::new(),
        }
    }
}

/// What one key reads as.
pub(super) enum Got<T> {
    Listing(T),
    Failed(AniError),
    Single,
    /// The page answered without it.
    Absent,
}

impl<T: Clone> Answers<T> {
    pub(super) fn get(&self, key: u32) -> Got<T> {
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
/// failure answers its own keys only, and one that could not be read
/// hands them to their single lookups.
pub(super) async fn answers<T, P, PF>(space: Space, keys: &[u32], ask: &mut P) -> Answers<T>
where
    P: FnMut(Space, Vec<u32>) -> PF,
    PF: Future<Output = Result<Page<Keyed<T>>>>,
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
            // A page that cannot be read cannot say which of its shows
            // it failed for: each is asked by its single lookup, as
            // before batching. Any other failure is that of every show
            // it covered.
            Err(AniError::ParseFailed { .. }) => out.single.extend(chunk.iter().copied()),
            Err(e) => out.failed.extend(chunk.iter().map(|k| (*k, e.clone()))),
        }
    }
    out
}

/// The single lookup for the key `key` of `space` names, made with
/// `want`, asked once however many shows need it. A key reaches here
/// only when AniList records media for it — several, more than a page
/// holds, or one whose node could not be read — or when its page could
/// not be read at all. In the first cases the lookup by it finds a
/// media and never falls back to the AniList id beside it, so every
/// show naming the key reads the same answer; in the last, shows naming
/// one MAL id with different AniList ids beside it share the first
/// one's answer, as they would whenever AniList knows the MAL id.
pub(super) async fn single_once<T: Clone, S, SF>(
    singles: &mut HashMap<(Space, u32), Result<T>>,
    single: &mut S,
    key: (Space, u32),
    want: Want,
) -> Result<T>
where
    S: FnMut(Want) -> SF,
    SF: Future<Output = Result<T>>,
{
    if let Some(done) = singles.get(&key) {
        return done.clone();
    }
    let got = single(want).await;
    singles.insert(key, got.clone());
    got
}
