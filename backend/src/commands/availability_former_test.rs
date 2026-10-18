//! The probe puts a stitched recap's former-tag stamp right.

use super::*;
use crate::commands::play_native_test_provider::{stitched_recap, STITCHED, STITCHED_SLUG};

fn args() -> AvailabilityArgs {
    AvailabilityArgs {
        title: STITCHED.into(),
        mode: "sub".into(),
        alt_titles: Vec::new(),
        episode_count: Some(12),
        year: Some(2026),
        subtype: Some("ONA".into()),
        kitsu_id: Some("77".into()),
        status: None,
        background: true,
        bypass_cache: false,
    }
}

#[tokio::test]
async fn a_probe_of_a_stitched_show_puts_its_former_tag_stamp_right() {
    // A history row from before stamped the recap's slot 2 "2.5". The
    // Continue card's probe pays for the stitched listing, where slot
    // 2 is "1.5": the stamp names it so before the card is clicked.
    let td = tempfile::tempdir().expect("td");
    let state = super::tests::cache_only_state(&td);
    let table = std::sync::Arc::new(stitched_recap());
    let key = crate::scraper::provider::ShowKey::new(
        crate::scraper::provider::Provider::id(table.as_ref()),
        STITCHED_SLUG,
    )
    .to_string();
    crate::commands::numbering_offset::put_display(&state, &key, 0, 2, "2.5");
    let got = super::tests::check_with(&state, &args(), &table)
        .await
        .expect("probed");
    assert!(got.available);
    assert_eq!(got.episode_count, Some(12), "the stitched listing");
    assert_eq!(
        crate::commands::numbering_offset::display_stamp(&state, &key),
        Some((2, "1.5".to_string()))
    );
    assert_eq!(
        crate::commands::numbering_offset::former_stamp(&state, &key),
        Some((2, "2.5".to_string()))
    );
}

#[tokio::test]
async fn a_probe_puts_the_numbering_right_before_it_caches_its_verdict() {
    // The verdict a probe caches is what the next probe takes instead
    // of fetching the listing: it lands after the stamp is put right.
    use crate::commands::play_native_test_provider::{
        assert_rows_landed_after_the_repair, stamps_as_rows_land,
    };
    let td = tempfile::tempdir().expect("td");
    let state = super::tests::cache_only_state(&td);
    let table = std::sync::Arc::new(stitched_recap());
    let key = crate::scraper::provider::ShowKey::new(
        crate::scraper::provider::Provider::id(table.as_ref()),
        STITCHED_SLUG,
    )
    .to_string();
    crate::commands::numbering_offset::put_display(&state, &key, 0, 2, "2.5");
    let (_observing, seen) = stamps_as_rows_land(&state, &key, &["availability:"]);
    super::tests::check_with(&state, &args(), &table)
        .await
        .expect("probed");
    let after = crate::commands::numbering_offset::display_stamp(&state, &key);
    assert_eq!(after, Some((2, "1.5".to_string())));
    assert_rows_landed_after_the_repair(&seen, &["availability:"], &after);
}
