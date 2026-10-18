//! Tests for `crate::commands::availability_served_mode`.

use super::merged_body;
use crate::commands::availability::AvailabilityResponse;
use crate::commands::play_native_versions::{VersionInfo, VersionKind};

fn version(kind: VersionKind, key: &str, has_mode: Option<bool>) -> VersionInfo {
    VersionInfo {
        kind,
        show_key: key.into(),
        has_mode,
    }
}

fn row(available: bool, versions: Vec<VersionInfo>) -> String {
    serde_json::to_string(&AvailabilityResponse {
        available,
        episode_count: Some(12),
        extra_episodes: Vec::new(),
        episode_count_approximate: false,
        gate_refused: false,
        unsettled: None,
        provider: None,
        versions,
    })
    .expect("serializes")
}

fn served() -> VersionInfo {
    version(VersionKind::Uncensored, "hianime:show-2", None)
}

#[test]
fn a_negative_row_takes_no_merge() {
    let body = row(
        false,
        vec![version(VersionKind::Uncensored, "hianime:show-2", None)],
    );
    assert_eq!(merged_body(&body, &served()), None);
}

#[test]
fn a_row_that_lists_another_copy_takes_no_merge() {
    // Same kind, another show key: not the version that served.
    let body = row(
        true,
        vec![version(VersionKind::Uncensored, "hianime:other-9", None)],
    );
    assert_eq!(merged_body(&body, &served()), None);
}

#[test]
fn a_row_that_does_not_parse_takes_no_merge() {
    assert_eq!(merged_body("not json", &served()), None);
}

#[test]
fn a_known_absent_mode_is_corrected_by_the_stream_that_served() {
    // A probe's sample said no; the served stream says yes, as a fresh
    // play's stamp would.
    let body = row(
        true,
        vec![version(
            VersionKind::Uncensored,
            "hianime:show-2",
            Some(false),
        )],
    );
    let merged: AvailabilityResponse =
        serde_json::from_str(&merged_body(&body, &served()).expect("merges")).expect("row");
    assert_eq!(merged.versions[0].has_mode, Some(true));
}
