//! What a test's stub tool writes for the test to read back: its
//! argv, a pid. The stub runs beside the test — detached, or still
//! running when the test polls for the file — so the test can read the
//! file while the stub is writing it.

/// A shell line that writes `producer`'s output to `path`.
pub(crate) fn publish(path: &std::path::Path, producer: &str) -> String {
    format!("{producer} > '{path}'", path = path.display())
}
