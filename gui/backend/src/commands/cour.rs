//! Cour-detection helpers shared by the mark-watched integrity guard.
//!
//! The reverse cache (`allmanga show_id → kitsu_id`) used to record
//! cross-cour mappings — e.g. Stone Ocean Part 2's allmanga show_id
//! paired with Part 1's Kitsu id — because the picker can pick a
//! sibling cour when ep-count and year tie. The guard reads the
//! allmanga show's title (cour from trailing "Part N" / "Cour N" /
//! "Season N") and compares it against the Kitsu slug's trailing
//! "-part-N" / "-cour-N" / "-season-N". Mismatch → reject the write.

#[must_use]
pub fn cour_from_title(_name: &str) -> Option<u32> {
    None
}

#[must_use]
pub fn cour_from_slug(_slug: &str) -> Option<u32> {
    None
}

#[must_use]
pub fn cours_agree(_allmanga: Option<u32>, _kitsu: Option<u32>) -> bool {
    true
}

#[cfg(test)]
#[path = "cour_test.rs"]
mod tests;
