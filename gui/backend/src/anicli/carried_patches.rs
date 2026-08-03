//! The fork's carried-patch table, as exact byte hunks.
//!
//! Each entry pairs a hunk as upstream publishes it with the same
//! hunk as the fork carries it (comment block, inserted lines, and
//! swapped lines together, plus one line of unchanged leading context
//! so pure insertions anchor uniquely). `update::revert_carried_patches`
//! maps fork -> upstream so `-U`'s whole-file comparison sees exactly
//! what upstream published; `update::repair_carried_patches` maps
//! upstream -> fork at boot and after every update, skipping hunks
//! whose fork form is already present so the pass is idempotent. The
//! loader-guard line is deliberately absent: the runtime cache never
//! contains it (`strip_lib_guard`).
//!
//! The table is empty as of the 5.0 sync. Every 4.15-era patch
//! retired with the code it patched: the greedy name capture and the
//! portable base64 lived in allanime functions 5.0 deleted, the
//! flatpak directory acceptance was absorbed upstream, and 5.0's
//! process_hist_entry dropped the fallback the history guard existed
//! to guard. The only fork content on the bundled script is the
//! source-guard line, which the runtime cache strips — so the cache
//! is byte-identical to what upstream published, and revert/repair
//! are identities until the next patch is carried.
//!
//! The machinery stays armed: when a patch is carried again, its
//! `(upstream_hunk, fork_hunk)` pair lands here, regenerated from a
//! line diff of the repo's `ani-cli` against upstream's, and the
//! round-trip test over the real script fails on any drift between
//! this table and the script's actual bytes.

/// `(upstream_hunk, fork_hunk)` pairs, in file order.
pub(crate) const CARRIED_PATCHES: &[(&str, &str)] = &[];

/// Fork hunks as EARLIER builds wrote them into the cache, paired
/// with today's form. A cache patched by a previous build matches
/// neither side of [`CARRIED_PATCHES`] when a carried patch itself
/// evolves — with auto-update disabled nothing would ever migrate it.
/// The repair pass applies these first, so newly carried changes
/// reach existing caches without requiring `-U`. Entries are dropped
/// once no supported upgrade path can still hold the legacy form —
/// which is the case now: any cache still holding a 4.15 fork form
/// gets replaced wholesale by the 5.0 bundle before repair runs.
pub(crate) const LEGACY_FORK_MIGRATIONS: &[(&str, &str)] = &[];
