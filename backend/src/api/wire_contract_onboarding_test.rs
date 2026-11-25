//! The feature tips' share of the wire contract (`api::onboarding`,
//! `crate::onboarding`): the tips offered, the Debug page's list, the
//! two seen bodies, and the names a tip's tool, page and moment take.
//! Split from `wire_contract_test.rs` like the routes are from
//! `api/mod.rs`.

use serde_json::{json, Map, Value};

use super::{request, response, some};
use crate::api::onboarding::{DebugSeenBody, SeenBody};
use crate::onboarding::decl::{Kind, Moment, Surface};
use crate::onboarding::release::Release;
use crate::onboarding::view::{DebugTip, DebugView, HowToView, OnboardingView, TipView};

const SINCE: Release = Release::new(0, 19, 0);

fn how_to() -> HowToView {
    HowToView {
        id: "a-guide".into(),
        surface: Surface::Franchise,
        places: vec!["one".into(), "two".into()],
    }
}

fn places(full: bool) -> Vec<String> {
    if full {
        vec!["watch-order-strip".into()]
    } else {
        Vec::new()
    }
}

fn tip(full: bool) -> TipView {
    TipView {
        id: "watch-order".into(),
        kind: Kind::Single,
        surface: Surface::Show,
        moment: Moment::Arrive,
        since: SINCE,
        places: places(full),
        how_to: some(full, how_to()),
        pending: full,
    }
}

fn debug_tip(full: bool) -> DebugTip {
    DebugTip {
        id: "watch-order".into(),
        kind: Kind::Single,
        surface: Surface::Show,
        moment: Moment::Arrive,
        since: SINCE,
        until: some(full, Release::new(0, 25, 0)),
        opens: some(full, SINCE),
        ends: some(full, Release::new(0, 21, 0)),
        flag: some(full, "a_feature".into()),
        flag_on: full,
        flag_on_since: some(full, SINCE),
        on_demand: full,
        offered: full,
        seen: full,
        pending: full,
        places: places(full),
        how_to: some(full, how_to()),
    }
}

pub(super) fn responses(out: &mut Map<String, Value>) {
    let mut put = |name: &str, v: Value| {
        out.insert(name.to_owned(), v);
    };
    put("OnboardingTip", response(&tip(true), &tip(false)));
    put("OnboardingHowTo", response(&how_to(), &how_to()));
    put(
        "OnboardingView",
        response(
            &OnboardingView {
                version: SINCE,
                tips: vec![tip(true)],
            },
            &OnboardingView {
                version: SINCE,
                tips: Vec::new(),
            },
        ),
    );
    put(
        "OnboardingDebugTip",
        response(&debug_tip(true), &debug_tip(false)),
    );
    put(
        "OnboardingDebugView",
        response(
            &DebugView {
                version: SINCE,
                installed_in: Some(SINCE),
                tips: vec![debug_tip(true)],
            },
            &DebugView {
                version: SINCE,
                installed_in: None,
                tips: Vec::new(),
            },
        ),
    );
}

pub(super) fn requests(out: &mut Map<String, Value>) {
    let mut put = |name: &str, v: Value| {
        out.insert(name.to_owned(), v);
    };
    put(
        "OnboardingSeenBody",
        request::<SeenBody>(&json!({ "id": "watch-order" })),
    );
    put(
        "OnboardingDebugSeenBody",
        request::<DebugSeenBody>(&json!({ "id": "watch-order", "seen": true })),
    );
}

pub(super) fn enums(out: &mut Map<String, Value>) {
    out.insert(
        "OnboardingKind".into(),
        super::names(&units!(Kind: Single, Sequence, Entry)),
    );
    out.insert(
        "OnboardingSurface".into(),
        super::names(&units!(Surface: Home, Show, Franchise, Settings, Play, Any)),
    );
    out.insert(
        "OnboardingMoment".into(),
        super::names(&units!(Moment: Arrive, StreamLoad)),
    );
}
