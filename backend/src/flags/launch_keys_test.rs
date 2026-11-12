//! Which keys each launch plan trusts. The live fetch trusts exactly
//! the build's own keys, in every build. A test run trusts those and,
//! only in a build with the `test-run-key` feature, the fixture
//! test-run key, whose secret anyone can derive from the fixtures'
//! `make.py`. The suite runs both ways: `cargo test --all-features`
//! with the feature, the coverage run without it.

use std::path::{Path, PathBuf};

use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{build_keys, start, LaunchPlan};
use crate::flags::decl::FlagValue;
use crate::flags::remote::{fetch_trusting, FETCH_TIMEOUT};
use crate::flags::signature::{TEST_RUN_KEYS, TRUSTED_KEYS};
use crate::flags::store::{FlagStore, Origin};
use crate::flags::test_decls::{A, B, TEST_DECLS};

const FILE: &str = include_str!("../../tests/fixtures/flags-signature/flags.json");
const TEST_RUN_KEY: &str = include_str!("../../tests/fixtures/flags-signature/test-run-key.pub");
const SIG_BY_TEST_RUN_KEY: &str =
    include_str!("../../tests/fixtures/flags-signature/flags.json.by-test-run-key.minisig");

fn live() -> LaunchPlan {
    LaunchPlan::Live {
        cache: Some(PathBuf::from("/c/remote-flags.json")),
    }
}

#[test]
fn the_live_fetch_trusts_exactly_the_builds_keys() {
    for plan in [LaunchPlan::Live { cache: None }, live()] {
        assert_eq!(build_keys(&plan), TRUSTED_KEYS, "{plan:?}");
        assert!(
            !build_keys(&plan).contains(&TEST_RUN_KEY.trim()),
            "the live fetch trusts the test-run key"
        );
    }
}

#[test]
fn a_test_run_trusts_the_builds_keys_and_the_test_run_keys() {
    let expected: Vec<&str> = TRUSTED_KEYS.iter().chain(TEST_RUN_KEYS).copied().collect();
    for file in [None, Some(PathBuf::from("/t/flags.json"))] {
        let plan = LaunchPlan::Test { file };
        assert_eq!(build_keys(&plan), expected.as_slice(), "{plan:?}");
    }
}

/// The live file signed by the test-run key, fetched under the keys a
/// normal launch trusts: its `true` is dropped, in a build with the
/// feature as in one without.
#[tokio::test]
async fn the_live_fetch_drops_a_turn_on_signed_by_the_test_run_key() {
    let server = MockServer::start().await;
    for (at, body) in [
        ("/flags/flags.json", FILE),
        ("/flags/flags.json.minisig", SIG_BY_TEST_RUN_KEY),
    ] {
        Mock::given(method("GET"))
            .and(path(at))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }
    let url = format!("{}/flags/flags.json", server.uri());
    let doc = fetch_trusting(
        &reqwest::Client::new(),
        &url,
        FETCH_TIMEOUT,
        build_keys(&live()),
    )
    .await
    .expect("document");
    assert_eq!(doc.get(A), None);
    assert_eq!(doc.get(B), Some(&json!(false)));
}

/// The fixture written as a test run's file, signed by the test-run
/// key, and read by a test-run launch under the build's own keys.
async fn test_run_signed_by_the_test_run_key(dir: &Path) -> (Option<FlagValue>, Option<FlagValue>) {
    let file = dir.join("flags.json");
    std::fs::write(&file, FILE).expect("write");
    std::fs::write(dir.join("flags.json.minisig"), SIG_BY_TEST_RUN_KEY).expect("write");
    let store = FlagStore::pending(TEST_DECLS);
    start(
        store.clone(),
        reqwest::Client::new(),
        LaunchPlan::Test { file: Some(file) },
    )
    .await
    .expect("join");
    assert_eq!(store.origin(), Origin::Test);
    let r = store.current();
    (r.get(A), r.get(B))
}

#[cfg(feature = "test-run-key")]
mod with_the_feature {
    use super::*;

    #[test]
    fn the_test_run_key_is_the_fixtures() {
        let keys: Vec<&str> = TEST_RUN_KEYS.iter().map(|k| k.trim()).collect();
        assert_eq!(keys, [TEST_RUN_KEY.trim()]);
    }

    #[tokio::test]
    async fn a_test_runs_file_signed_by_the_test_run_key_turns_its_feature_on() {
        let td = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            test_run_signed_by_the_test_run_key(td.path()).await,
            (Some(FlagValue::Bool(true)), Some(FlagValue::Bool(false)))
        );
    }
}

#[cfg(not(feature = "test-run-key"))]
mod without_the_feature {
    use super::*;

    /// What every packaged build is: the packaging scripts never enable
    /// the feature (`tests/arch/test_run_key.sh`).
    #[test]
    fn a_build_without_the_feature_trusts_no_test_run_key() {
        assert!(std::hint::black_box(TEST_RUN_KEYS).is_empty());
    }

    #[tokio::test]
    async fn a_test_runs_file_signed_by_the_test_run_key_turns_nothing_on() {
        let td = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            test_run_signed_by_the_test_run_key(td.path()).await,
            (Some(FlagValue::Bool(false)), Some(FlagValue::Bool(false)))
        );
    }
}
