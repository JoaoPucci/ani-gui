//! The pick's golden cases, shared with the renderer's tests: each
//! input in `tests/fixtures/hero-pick/cases.json`, with the pick the
//! backend makes of it. The acceptance tier answers Home's pick with a
//! fake of this module (`frontend/src/test/hero-pick-fake.ts`), held
//! to the same file, so the fake cannot drift from the pick unnoticed
//! on these cases.

use serde_json::Value;

use super::{pick, HeroShow, HERO_ROTATION_COUNT};

const CASES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/hero-pick/cases.json"
);

#[test]
fn every_golden_case_is_the_pick_the_backend_makes() {
    let text = std::fs::read_to_string(CASES).expect("the golden cases are readable");
    let cases: Vec<Value> = serde_json::from_str(&text).expect("the golden cases are JSON");
    assert!(!cases.is_empty());
    for case in cases {
        let shows: Vec<HeroShow> =
            serde_json::from_value(case["shows"].clone()).expect("shows read");
        let on = case["airing_first"].as_bool().expect("airing_first");
        let got = serde_json::to_value(pick(&shows, HERO_ROTATION_COUNT, on)).expect("serializes");
        assert_eq!(
            got, case["expected"],
            "{} (airing_first {on})",
            case["name"]
        );
    }
}
