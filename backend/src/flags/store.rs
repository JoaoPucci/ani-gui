//! What the rest of the backend reads.
//!
//! The flags are decided once per launch and never change after that.
//! The store starts pending, at the built-in defaults, and is settled
//! exactly once — by the launch refresh (see [`super::refresh`]) or by
//! the test run's own file (see [`super::launch`]) — and every later
//! attempt to settle it is refused. A feature reads the flags through
//! [`FlagStore::settled`], which waits for that one decision, and so
//! never sees a value it will later see change.
//!
//! [`FlagStore::current`] does not wait and is for diagnostics only:
//! before the store settles it answers with the defaults, which a
//! feature must not act on.

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::watch;

use super::decl::FlagDecl;
use super::resolve::{resolve, Document, Resolved};

/// How long the launch waits for the flags file before deciding
/// without it. The app's first page waits behind this, so it is
/// short; the fetch itself goes on past it (see
/// [`super::refresh`]).
pub const SETTLE_CAP: Duration = Duration::from_secs(2);

/// The longest [`FlagStore::settled`] waits. Past it the store
/// settles at the defaults itself: a refresh that never decides
/// (its cache read hung, its task never ran) cannot hold up a
/// reader, and cannot change a value one has already seen.
pub const SETTLE_LIMIT: Duration = Duration::from_secs(SETTLE_CAP.as_secs() + 1);

/// Where the launch's flags came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    /// Not decided yet.
    Pending,
    /// The file, fetched within the cap.
    File,
    /// The cached copy, the file not arriving within the cap.
    Cache,
    /// The built-in defaults: no file within the cap and no fresh
    /// cache, or a store no refresh fills.
    Defaults,
    /// A test run's own file, or the defaults when it gave none.
    Test,
}

impl Origin {
    /// The name the route and the log use.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::File => "file",
            Self::Cache => "cache",
            Self::Defaults => "defaults",
            Self::Test => "test",
        }
    }
}

#[derive(Debug, Clone)]
struct Snapshot {
    flags: Resolved,
    origin: Origin,
}

/// The launch's flags, shared by every handler.
#[derive(Clone)]
pub struct FlagStore {
    decls: &'static [FlagDecl],
    tx: Arc<watch::Sender<Snapshot>>,
}

impl FlagStore {
    /// A store at the defaults, waiting for the launch to settle it.
    #[must_use]
    pub fn pending(decls: &'static [FlagDecl]) -> Self {
        Self::with(decls, Origin::Pending)
    }

    /// A store settled at the defaults; for a state nothing fills.
    #[must_use]
    pub fn fixed(decls: &'static [FlagDecl]) -> Self {
        Self::with(decls, Origin::Defaults)
    }

    fn with(decls: &'static [FlagDecl], origin: Origin) -> Self {
        let (tx, _rx) = watch::channel(Snapshot {
            flags: resolve(decls, None),
            origin,
        });
        Self {
            decls,
            tx: Arc::new(tx),
        }
    }

    /// The flags as they stand, settled or not. Diagnostics only: a
    /// feature reads [`FlagStore::settled`].
    #[must_use]
    pub fn current(&self) -> Resolved {
        self.tx.borrow().flags.clone()
    }

    /// Where the flags came from; [`Origin::Pending`] until settled.
    #[must_use]
    pub fn origin(&self) -> Origin {
        self.tx.borrow().origin
    }

    /// Whether the launch's flags are decided.
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.origin() != Origin::Pending
    }

    /// The launch's flags, once decided. Waits at most
    /// [`SETTLE_LIMIT`]; past it, settles the store at the defaults
    /// and answers with those.
    pub async fn settled(&self) -> Resolved {
        let mut rx = self.tx.subscribe();
        let wait = rx.wait_for(|s| s.origin != Origin::Pending);
        if tokio::time::timeout(SETTLE_LIMIT, wait).await.is_err() {
            self.settle(None, Origin::Defaults);
        }
        self.current()
    }

    /// Wait, without a limit, until the flags are decided.
    #[cfg(test)]
    pub(crate) async fn decided(&self) {
        let mut rx = self.tx.subscribe();
        let _ = rx.wait_for(|s| s.origin != Origin::Pending).await;
    }

    /// Decide the launch's flags from `doc`, unless they are decided
    /// already. Returns whether this call decided them.
    pub(crate) fn settle(&self, doc: Option<&Document>, origin: Origin) -> bool {
        let flags = resolve(self.decls, doc);
        self.tx.send_if_modified(|snap| {
            if snap.origin != Origin::Pending {
                return false;
            }
            *snap = Snapshot { flags, origin };
            true
        })
    }
}

#[cfg(test)]
#[path = "store_test.rs"]
mod tests;
