//! Property tests for `crate::commands::download`. Extracted via
//! `#[path]` per `project_crap_inline_test_gotcha`.

use super::*;

/// Strategy over every [`AniError`] variant, with arbitrary payloads
/// where a variant carries data ([`AniError`] isn't `Clone`, so
/// variants are built in a `prop_map` instead of `Just`). A variant
/// added to the enum without extending this list keeps the property
/// honest by review: the classifier's exhaustiveness is what's under
/// test.
fn any_ani_error() -> impl proptest::strategy::Strategy<Value = AniError> {
    use proptest::strategy::Strategy;
    (0usize..17, ".*", proptest::num::u16::ANY).prop_map(|(pick, text, status)| match pick {
        0 => AniError::Scraper {
            key: crate::i18n::keys::SCRAPER_PARSE_FAILED,
        },
        1 => AniError::Timeout,
        2 => AniError::NoResults,
        3 => AniError::ParseFailed { detail: text },
        4 => AniError::MissingBinary,
        5 => AniError::BashMissing,
        6 => AniError::FfmpegMissing,
        7 => AniError::Upstream { status },
        8 => AniError::Network,
        9 => AniError::PlayerSpawnFailed { binary: text },
        10 => AniError::SyncplaySpawnFailed { binary: text },
        11 => AniError::Cache,
        12 => AniError::Io,
        13 => AniError::Config,
        14 => AniError::Metadata,
        15 => AniError::UnsupportedPkce,
        _ => AniError::InvalidToken,
    })
}

proptest::proptest! {
    // `NoResults` is the search-stage rate-limit signature and the
    // ONLY error allowed to feed the gate from a download run —
    // every other variant is local, transfer-stage, or metadata
    // noise and must leave the gate untouched.
    #[test]
    fn download_gate_signal_flags_only_no_results(e in any_ani_error()) {
        let expect_failure = matches!(e, AniError::NoResults);
        let got = download_gate_signal::<()>(&Err(e));
        let want = if expect_failure { Some(false) } else { None };
        proptest::prop_assert_eq!(got, want);
    }

    // A download's success signal is stale by construction (see the
    // classifier docs) — no Ok payload may ever record a gate
    // success.
    #[test]
    fn download_gate_signal_never_records_success(n in proptest::num::u32::ANY) {
        let got = download_gate_signal(&Ok(n));
        proptest::prop_assert_eq!(got, None);
    }
}
