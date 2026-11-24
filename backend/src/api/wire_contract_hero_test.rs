//! The home hero routes' share of the wire contract (`api::hero`,
//! `crate::hero`): the pick and its walk, the Debug page's view of the
//! last walk, the body the pick reads, and the names its walk carries.
//! Split from `wire_contract_test.rs` like the routes are from
//! `api/mod.rs`.

use serde_json::{json, Map, Value};

use super::{request, response, some};
use crate::api::hero::{FlagView, HeroPickBody, WalkView};
use crate::flags::store::Origin;
use crate::hero::{
    Art, CoverSlots, HeroShow, Pick, SkippedBecause, TakenBecause, Verdict, Walk, WalkStep,
};

const TAKEN: Verdict = Verdict::Taken {
    slot: 0,
    because: TakenBecause::AiringArt,
};
const SKIPPED: Verdict = Verdict::Skipped {
    because: SkippedBecause::NotAiring,
};

fn step(full: bool) -> WalkStep {
    WalkStep {
        rank: 1,
        id: "46474".into(),
        title: "Sousou no Frieren".into(),
        art: Art::Banner,
        status: some(full, "current".into()),
        airing: full,
        preference_applied: full,
        verdict: TAKEN,
    }
}

fn walk(full: bool) -> Walk {
    Walk {
        airing_first: full,
        count: 3,
        entries: 1,
        steps: if full { vec![step(true)] } else { Vec::new() },
    }
}

fn pick(full: bool) -> Pick {
    let ids = |v: &str| if full { vec![v.to_owned()] } else { Vec::new() };
    Pick {
        hero: ids("46474"),
        row: ids("7158"),
        walk: walk(full),
    }
}

fn flag() -> FlagView {
    FlagView {
        name: crate::hero::HERO_AIRING_FIRST.name,
        on: true,
        origin: Origin::File,
    }
}

pub(super) fn responses(out: &mut Map<String, Value>) {
    let mut put = |name: &str, v: Value| {
        out.insert(name.to_owned(), v);
    };
    put("HeroPick", response(&pick(true), &pick(false)));
    put("HeroWalk", response(&walk(true), &walk(false)));
    put("HeroWalkStep", response(&step(true), &step(false)));
    put("HeroVerdict.taken", response(&TAKEN, &TAKEN));
    put("HeroVerdict.skipped", response(&SKIPPED, &SKIPPED));
    put("HeroFlagView", response(&flag(), &flag()));
    put(
        "HeroWalkView",
        response(
            &WalkView {
                flag: flag(),
                walk: Some(walk(true)),
            },
            &WalkView {
                flag: flag(),
                walk: None,
            },
        ),
    );
}

pub(super) fn requests(out: &mut Map<String, Value>) {
    let mut put = |name: &str, v: Value| {
        out.insert(name.to_owned(), v);
    };
    put(
        "HeroPickBody",
        request::<HeroPickBody>(&json!({ "shows": [] })),
    );
    put("HeroShow", request::<HeroShow>(&json!({ "id": "46474" })));
    put("HeroCoverSlots", request::<CoverSlots>(&json!({})));
}

pub(super) fn enums(out: &mut Map<String, Value>) {
    out.insert(
        "HeroArt".into(),
        super::names(&units!(Art: Banner, Cover, None)),
    );
    out.insert(
        "HeroTakenBecause".into(),
        super::names(&units!(TakenBecause: Art, AiringArt, Fill)),
    );
    out.insert(
        "HeroSkippedBecause".into(),
        super::names(&units!(SkippedBecause: Repeat, NotAiring, HeroFull, NoArt)),
    );
}
