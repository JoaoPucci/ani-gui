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
    /// Keys only a single lookup can settle, AniList having shown it
    /// records media for them.
    single: HashSet<u32>,
    /// Keys whose page could not be read: a single lookup settles each
    /// show naming them, with nothing known about the key.
    unread: HashSet<u32>,
    failed: HashMap<u32, AniError>,
}

impl<T> Default for Answers<T> {
    fn default() -> Self {
        Self {
            found: HashMap::new(),
            single: HashSet::new(),
            unread: HashSet::new(),
            failed: HashMap::new(),
        }
    }
}

/// What one key reads as.
pub(super) enum Got<T> {
    Listing(T),
    Failed(AniError),
    /// AniList records media for the key: one single lookup by it
    /// answers every show naming it.
    Single,
    /// The page could not be read: each show naming the key is asked by
    /// its own single lookup.
    Unread,
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
        if self.unread.contains(&key) {
            return Got::Unread;
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
            Err(AniError::ParseFailed { .. }) => out.unread.extend(chunk.iter().copied()),
            Err(e) => out.failed.extend(chunk.iter().map(|k| (*k, e.clone()))),
        }
    }
    out
}

/// Which single lookup answers a show: one per key AniList records
/// media for, however many shows name it — the lookup by it finds a
/// media and never falls back to the AniList id beside it — and one per
/// show where its page could not be read, since nothing then shows
/// AniList knows the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Settles {
    Key(Space, u32),
    Show(Want),
}

/// The single lookups a batch falls back to, each with the shows it
/// answers.
pub(super) struct Singles {
    lookups: Vec<(Want, Vec<usize>)>,
    by: HashMap<Settles, usize>,
}

impl Singles {
    pub(super) fn new() -> Self {
        Self {
            lookups: Vec::new(),
            by: HashMap::new(),
        }
    }

    /// Show `index` is answered by the single lookup `settles` names,
    /// made with `want` when it is the first show to need it.
    pub(super) fn add(&mut self, settles: Settles, want: Want, index: usize) {
        let at = *self.by.entry(settles).or_insert_with(|| {
            self.lookups.push((want, Vec::new()));
            self.lookups.len() - 1
        });
        self.lookups[at].1.push(index);
    }

    /// Run every lookup at once — the gate paces them — and hand each
    /// answer to its shows as it comes in.
    pub(super) async fn run<T, SF>(
        self,
        single: &impl Fn(Want) -> SF,
        on_answer: &impl Fn(usize, Result<T>),
    ) where
        T: Clone,
        SF: Future<Output = Result<T>>,
    {
        use futures_util::stream::{FuturesUnordered, StreamExt};
        let mut out: FuturesUnordered<_> = self
            .lookups
            .into_iter()
            .map(|(want, shows)| {
                let lookup = single(want);
                async move { (shows, lookup.await) }
            })
            .collect();
        while let Some((shows, answer)) = out.next().await {
            for index in shows {
                on_answer(index, answer.clone());
            }
        }
    }
}
