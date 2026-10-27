//! When a listing was heard, and the record a hearing leaves: the
//! decision [`super::put`] makes between a stored record and a new one,
//! pure. Split from [`super`] so each file stays inside the CRAP gate's
//! per-file bar.

/// When a listing was heard. A walk cannot say exactly when the
/// listing it picked was fetched, only that it was after `start`, when
/// the walk asked for the show's listings, and by `at`, when it made the
/// pick. `run` names the process the stamps belong to: they count this
/// process's hearings, and compare with no other run's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Hearing {
    /// The process whose clock `start` and `at` read.
    pub run: u64,
    /// No earlier than this the listing was fetched.
    pub start: u64,
    /// No later than this.
    pub at: u64,
}

/// A hearing begun: the moment a walk asks for a show's listings.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Begun(u64);

impl Begun {
    /// A hearing beginning now.
    #[must_use]
    pub(crate) fn now() -> Self {
        Self(tick())
    }

    /// The hearing, its pick made now.
    #[must_use]
    pub(crate) fn picked(self) -> Hearing {
        Hearing {
            run: run(),
            start: self.0,
            at: tick(),
        }
    }
}

/// A listing's record: its extra tags, and when the listing was heard.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Record {
    pub(crate) heard: Hearing,
    pub(crate) tags: Vec<String>,
}

/// This process's clock for hearings: a count that only goes up. One
/// backend runs per cache directory, so ordering within a run is all a
/// record needs; a wall clock that jumps would order nothing.
fn tick() -> u64 {
    static CLOCK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    CLOCK.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

/// A tick of [`tick`]'s clock, for a test to place a moment among a
/// walk's stamps.
#[cfg(test)]
pub(crate) fn tick_for_tests() -> u64 {
    tick()
}

/// The run this process's hearings belong to: drawn once, at the first
/// hearing.
fn run() -> u64 {
    static RUN: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *RUN.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        // The low bits of the start instant, mixed with the process id:
        // two runs drawing the same value would need both to match. A
        // run is never 0, which a record no run of this build wrote can
        // carry and be told apart by.
        ((nanos as u64) ^ (u64::from(std::process::id()) << 32)) | 1
    })
}

/// The record to write for `new`, given what the store answered for the
/// show (`stored`: `Err` when the read failed), or `None` to write
/// nothing:
/// - a failed read says nothing of what is stored, which may be newer:
///   nothing is written;
/// - no record, or one an earlier run wrote — older than any hearing
///   now — is replaced;
/// - a record heard wholly before `new` began is replaced, and `new` is
///   dropped when it was heard wholly before the record began;
/// - spans that overlap cannot be ordered, so the record keeps the tags
///   of both, over the span of both: a former number either listing
///   gives an episode is not named.
#[must_use]
pub(crate) fn next_record(stored: Result<Option<Record>, ()>, new: Record) -> Option<Record> {
    let Ok(stored) = stored else {
        return None;
    };
    let Some(old) = stored.filter(|o| o.heard.run == new.heard.run) else {
        return Some(new);
    };
    if new.heard.start >= old.heard.at {
        return Some(new);
    }
    if new.heard.at <= old.heard.start {
        return None;
    }
    let mut tags = old.tags;
    for t in new.tags {
        if !tags.contains(&t) {
            tags.push(t);
        }
    }
    Some(Record {
        heard: Hearing {
            run: new.heard.run,
            start: old.heard.start.min(new.heard.start),
            at: old.heard.at.max(new.heard.at),
        },
        tags,
    })
}
