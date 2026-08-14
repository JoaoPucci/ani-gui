use super::*;

proptest::proptest! {
    // The block predicate is a shared contract between the
    // episode-probe walk, the detail-year probe, and the resolve
    // walk's stop condition: exactly the refusal shapes, nothing
    // not-found-shaped.
    #[test]
    fn provider_block_is_exactly_the_refusal_shaped_statuses(
        status in proptest::num::u16::ANY,
    ) {
        let want = status == 403 || status == 429 || status >= 500;
        proptest::prop_assert_eq!(
            AniError::Upstream { status }.is_provider_block(),
            want
        );
    }
}

#[test]
fn rate_limits_block_and_verdicts_do_not() {
    assert!(AniError::RateLimited {
        retry_after_secs: None
    }
    .is_provider_block());
    assert!(!AniError::NoResults.is_provider_block());
    assert!(!AniError::Network.is_provider_block());
}

#[test]
fn rate_limited_maps_to_429_and_a_dedicated_key() {
    // The typed rate-limit answer (allanime's in-band "Too many
    // requests" GraphQL payload) must surface as HTTP 429 so the
    // frontend can distinguish "wait a few seconds" from a
    // generic upstream failure, and carry its own i18n key.
    let e = AniError::RateLimited {
        retry_after_secs: Some(9),
    };
    assert_eq!(e.http_status_code(), 429);
    assert_eq!(e.key(), "error.network.rate_limited");
}

#[test]
fn every_variant_has_a_stable_key() {
    // A representative of each variant — if a new variant lands without
    // a matching arm in `key()`, this test forces the author to think
    // about its i18n key.
    let cases = [
        AniError::Scraper {
            key: "error.scraper.custom_test_key",
        },
        AniError::Timeout,
        AniError::NoResults,
        AniError::ParseFailed { detail: "x".into() },
        AniError::FfmpegMissing,
        AniError::PlayerSpawnFailed {
            binary: "vlc".into(),
        },
        AniError::SyncplaySpawnFailed {
            binary: "syncplay".into(),
        },
        AniError::Upstream { status: 503 },
        AniError::Network,
        AniError::GateRefused,
        AniError::Cache,
        AniError::Io,
        AniError::Config,
        AniError::Metadata,
        AniError::InvalidToken,
    ];
    for c in cases {
        let k = c.key();
        assert!(
            k.starts_with("error."),
            "every error key starts with 'error.': got {k:?} for {c:?}"
        );
        assert!(!k.is_empty());
    }
}

#[test]
fn serializes_with_kind_discriminator() {
    let err = AniError::NoResults;
    let s = serde_json::to_string(&err).expect("serializes");
    assert!(s.contains("\"kind\""), "serialized form has kind tag: {s}");
    assert!(s.contains("no_results"), "snake_case discriminant: {s}");
}

#[test]
fn ffmpeg_missing_serializes_with_a_dedicated_kind_and_key() {
    // A download with no usable tool used to reach the user as a
    // generic "Download failed" tooltip with nothing actionable in
    // it. The dedicated variant is what lets the frontend render a
    // modal pointing at the official download page instead, so its
    // serialized shape is part of that contract rather than an
    // implementation detail.
    let err = AniError::FfmpegMissing;
    let s = serde_json::to_string(&err).expect("serializes");
    assert!(
        s.contains("\"kind\":\"ffmpeg_missing\""),
        "snake_case kind: {s}"
    );
    assert_eq!(err.key(), "error.download.ffmpeg_missing");
}

#[test]
fn syncplay_spawn_failed_carries_the_configured_binary_name() {
    // Mirror of player_spawn_failed: the frontend's ErrorOverlay
    // names which binary failed and links to syncplay.pl so the
    // user can install or fix their path. Pin the JSON shape +
    // i18n key.
    let err = AniError::SyncplaySpawnFailed {
        binary: "syncplay".into(),
    };
    let s = serde_json::to_string(&err).expect("serializes");
    assert!(
        s.contains("\"binary\":\"syncplay\""),
        "serialized form has binary field: {s}"
    );
    assert!(
        s.contains("\"kind\":\"syncplay_spawn_failed\""),
        "serialized form has snake_case kind: {s}"
    );
    assert_eq!(err.key(), "error.syncplay.spawn_failed");
}

#[test]
fn player_spawn_failed_carries_the_configured_binary_name() {
    // The frontend toast should be able to name *which* player
    // failed — generic "missing binary" wasn't actionable. Pin
    // that the JSON the frontend receives includes the binary.
    let err = AniError::PlayerSpawnFailed {
        binary: "vlc".into(),
    };
    let s = serde_json::to_string(&err).expect("serializes");
    assert!(
        s.contains("\"binary\":\"vlc\""),
        "serialized form has binary field: {s}"
    );
    assert!(
        s.contains("\"kind\":\"player_spawn_failed\""),
        "serialized form has snake_case kind: {s}"
    );
    assert_eq!(err.key(), "error.player.spawn_failed");
}

/// Display impl drives `tracing::error!("{err}")` lines and the
/// fallback message text in tests. Pin a representative subset
/// so a stray `#[error("…")]` rewrite gets caught.
#[test]
fn display_messages_match_thiserror_attributes() {
    assert_eq!(format!("{}", AniError::Timeout), "scraper timed out");
    assert_eq!(format!("{}", AniError::NoResults), "no results");
    assert_eq!(format!("{}", AniError::Network), "network error");
    assert_eq!(
        format!("{}", AniError::Upstream { status: 503 }),
        "upstream 503"
    );
    assert_eq!(
        format!(
            "{}",
            AniError::ParseFailed {
                detail: "stdout shape".into()
            }
        ),
        "parse failed: stdout shape"
    );
}

/// Each From impl collapses an upstream error into a single
/// `AniError` variant — the frontend never sees the underlying
/// reqwest / rusqlite / serde / toml type. A bug in one of these
/// would surface as the wrong i18n key for the user.
#[test]
fn rusqlite_error_maps_to_cache_variant() {
    let sqlite_err = rusqlite::Error::ExecuteReturnedResults;
    let mapped: AniError = sqlite_err.into();
    assert!(matches!(mapped, AniError::Cache));
    assert_eq!(mapped.key(), "error.cache.generic");
}

#[test]
fn io_error_maps_to_io_variant() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "x");
    let mapped: AniError = io_err.into();
    assert!(matches!(mapped, AniError::Io));
    assert_eq!(mapped.key(), "error.io.generic");
}

#[test]
fn serde_error_carries_its_detail_into_parse_failed() {
    // The detail field is for logs, not user-facing copy. Pin
    // that the conversion preserves it so debugging stays sane.
    let serde_err = serde_json::from_str::<u32>("not a number").unwrap_err();
    let mapped: AniError = serde_err.into();
    match mapped {
        AniError::ParseFailed { detail } => assert!(!detail.is_empty()),
        other => panic!("expected ParseFailed, got {other:?}"),
    }
}

#[test]
fn upstream_429_surfaces_as_too_many_requests() {
    // A tracker rate-limit (429) must reach the frontend as 429 so the
    // list editor can show a "rate-limited, try again" message instead of
    // a generic failure; every other upstream status still collapses to
    // 502 (a generic bad-gateway).
    assert_eq!(AniError::Upstream { status: 429 }.http_status_code(), 429);
    assert_eq!(AniError::Upstream { status: 500 }.http_status_code(), 502);
    assert_eq!(AniError::Upstream { status: 503 }.http_status_code(), 502);
}

#[test]
fn toml_error_maps_to_config_variant() {
    let toml_err: toml::de::Error =
        toml::from_str::<toml::Value>("not = valid = toml").unwrap_err();
    let mapped: AniError = toml_err.into();
    assert!(matches!(mapped, AniError::Config));
    assert_eq!(mapped.key(), "error.config.parse");
}
