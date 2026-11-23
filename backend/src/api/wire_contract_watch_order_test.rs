//! The watch-order routes' share of the wire contract
//! (`api::watch_order`, `crate::watch_order::wire`): their answers, the
//! bodies they read, and the names their tagged answers carry. Split from
//! `wire_contract_test.rs` like the routes are from `api/mod.rs`.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use super::{request, response, some};
use crate::api::watch_order::{DayArgs, HandoffAnswer, UpNextArgs};
use crate::meta::kitsu::FranchiseEntry;
use crate::watch_order::chain_end::ChainEndCard;
use crate::watch_order::chain_view::Seam;
use crate::watch_order::entry::ProgressView;
use crate::watch_order::handoff::{AskRow, Handoff, HandoffMatch, PlayedRow};
use crate::watch_order::page_model::{Eyebrow, Seams, StripCaption};
use crate::watch_order::strip_view::{FranchiseSummary, StripKind};
use crate::watch_order::wire::{
    FranchiseView, FranchiseWire, LinkWire, NextWire, RailRequest, RailView, ResumeWire, RowWire,
    SectionWire, ShowView, Stack, StripWire, TitleRef, UpNextView, UpNextWire, ViewEntry,
    WatchingView,
};
use crate::watch_order::WatchOrder;

fn entry(full: bool) -> FranchiseEntry {
    FranchiseEntry {
        id: "7158".into(),
        title: "JoJo no Kimyou na Bouken (TV)".into(),
        subtype: some(full, "TV".into()),
        start: some(full, "2012-10-06".into()),
        episodes: some(full, 26),
        status: some(full, "finished".into()),
        poster: some(full, "https://media.kitsu.app/anime/7158/poster.jpg".into()),
        cover: some(full, "https://media.kitsu.app/anime/7158/cover.jpg".into()),
    }
}

fn view_entry(full: bool) -> ViewEntry {
    let e = entry(full);
    ViewEntry {
        id: e.id,
        title: e.title,
        subtype: e.subtype,
        start: e.start,
        episodes: e.episodes,
        status: e.status,
        poster: e.poster,
        cover: e.cover,
        unaired: full,
        progress: ProgressView::None,
    }
}

fn list<T>(full: bool, v: T) -> Vec<T> {
    if full {
        vec![v]
    } else {
        Vec::new()
    }
}

fn summary(full: bool) -> FranchiseSummary {
    FranchiseSummary {
        total: 9,
        main: 5,
        films: 2,
        sides: 1,
        recaps: 1,
        posters: list(full, entry(full)),
    }
}

fn stack(full: bool) -> Stack {
    Stack {
        front: "7158".into(),
        summary: summary(full),
    }
}

fn seams(full: bool) -> Seams {
    Seams {
        before: some(full, Seam::From),
        after: some(full, Seam::Into),
    }
}

fn watching(full: bool) -> WatchingView {
    WatchingView {
        entry: view_entry(full),
        episode: 7.5,
    }
}

fn strip(full: bool) -> StripWire {
    StripWire {
        kind: StripKind::Run,
        caption: StripCaption::Position {
            position: 4,
            total: 5,
        },
        eyebrow: some(
            full,
            Eyebrow {
                position: 4,
                total: 5,
            },
        ),
        complete: full,
        outside: full,
        seams: seams(full),
        before: list(full, view_entry(full)),
        after: list(full, view_entry(full)),
        watching: some(full, watching(full)),
        stack: stack(full),
        similar_hidden: list(full, "8063".to_string()),
    }
}

fn link(full: bool) -> LinkWire {
    LinkWire {
        stack: stack(full),
        complete: full,
        similar_hidden: list(full, "8063".to_string()),
    }
}

fn show(full: bool) -> ShowView {
    ShowView {
        order: WatchOrder::Linked,
        strip: some(full, strip(true)),
        link: some(full, link(true)),
    }
}

fn row(full: bool) -> RowWire {
    RowWire {
        entry: view_entry(full),
        position: some(full, 1),
        share: 0.5,
        leads_into: some(full, "Stardust Crusaders".into()),
        here: full,
    }
}

fn resume(full: bool) -> ResumeWire {
    ResumeWire {
        entry: view_entry(full),
        episode: 7.0,
        position: some(full, 1),
    }
}

fn page(full: bool) -> FranchiseWire {
    FranchiseWire {
        name: "JoJo no Kimyou na Bouken".into(),
        art: entry(full),
        total: some(full, 9),
        alone: full,
        resume: some(full, resume(true)),
        start: some(full, view_entry(true)),
        sections: list(
            full,
            SectionWire::Group {
                rows: vec![row(true)],
            },
        ),
        versions: list(full, entry(true)),
        films: list(full, entry(true)),
        sides: list(full, entry(true)),
        recaps: list(full, entry(true)),
        recap_count: some(full, 1),
        partial: full,
        opened_from: some(full, title_ref()),
    }
}

fn title_ref() -> TitleRef {
    TitleRef {
        id: "7158".into(),
        title: "JoJo no Kimyou na Bouken (TV)".into(),
    }
}

fn franchise_view(full: bool) -> FranchiseView {
    FranchiseView {
        order: WatchOrder::Released,
        page: some(full, page(true)),
    }
}

fn chain_end(full: bool) -> ChainEndCard {
    ChainEndCard {
        name: "JoJo no Kimyou na Bouken".into(),
        id: "7730".into(),
        front: "7158".into(),
        summary: summary(full),
    }
}

fn handoff(full: bool) -> Handoff {
    Handoff {
        finished: entry(full),
        next: entry(full),
    }
}

fn rail(full: bool) -> RailView {
    RailView {
        asks: if full {
            [("row".to_string(), Some("7158".to_string()))].into()
        } else {
            BTreeMap::new()
        },
        handoffs: if full {
            [("row".to_string(), handoff(true))].into()
        } else {
            BTreeMap::new()
        },
    }
}

/// Every variant of a tagged answer, under `Name.kind`.
fn tagged<T: serde::Serialize>(put: &mut impl FnMut(&str, Value), name: &str, all: &[T]) {
    for v in all {
        let kind = serde_json::to_value(v).expect("serializes")["kind"]
            .as_str()
            .expect("a tagged answer names its kind")
            .to_owned();
        put(&format!("{name}.{kind}"), response(v, v));
    }
}

/// The watch-order answers, put into the contract's responses.
pub(super) fn responses(out: &mut Map<String, Value>) {
    let mut put = |name: &str, v: Value| {
        out.insert(name.to_owned(), v);
    };
    put("ShowView", response(&show(true), &show(false)));
    put("StripWire", response(&strip(true), &strip(false)));
    put("LinkWire", response(&link(true), &link(false)));
    put("WatchOrderStack", response(&stack(true), &stack(false)));
    put(
        "FranchiseSummary",
        response(&summary(true), &summary(false)),
    );
    put("ViewEntry", response(&view_entry(true), &view_entry(false)));
    put("StripSeams", response(&seams(true), &seams(false)));
    put(
        "Eyebrow",
        response(
            &Eyebrow {
                position: 1,
                total: 2,
            },
            &Eyebrow {
                position: 1,
                total: 2,
            },
        ),
    );
    put("WatchingView", response(&watching(true), &watching(false)));
    put(
        "FranchiseView",
        response(&franchise_view(true), &franchise_view(false)),
    );
    put("FranchiseWire", response(&page(true), &page(false)));
    put("RowWire", response(&row(true), &row(false)));
    put("ResumeWire", response(&resume(true), &resume(false)));
    put("TitleRef", response(&title_ref(), &title_ref()));
    put(
        "UpNextView",
        response(
            &UpNextView {
                card: Some(UpNextWire::Franchise(chain_end(true))),
            },
            &UpNextView { card: None },
        ),
    );
    put(
        "ChainEndCard",
        response(&chain_end(true), &chain_end(false)),
    );
    put(
        "HandoffAnswer",
        response(
            &HandoffAnswer {
                handoff: Some(handoff(true)),
            },
            &HandoffAnswer { handoff: None },
        ),
    );
    put("Handoff", response(&handoff(true), &handoff(false)));
    put("RailView", response(&rail(true), &rail(false)));
    tagged(&mut put, "ProgressView", &progress_views());
    // Part-way through an entry whose count Kitsu does not know.
    put(
        "ProgressView.partway",
        response(
            &ProgressView::Partway {
                episode: 7.5,
                total: Some(12),
            },
            &ProgressView::Partway {
                episode: 7.5,
                total: None,
            },
        ),
    );
    tagged(&mut put, "StripCaption", &captions());
    tagged(&mut put, "SectionWire", &sections());
    tagged(&mut put, "NextWire", &nexts());
    tagged(&mut put, "UpNextWire", &up_next_cards());
}

fn progress_views() -> Vec<ProgressView> {
    listed!(ProgressView {
        ProgressView::None => ProgressView::None,
        ProgressView::Watched => ProgressView::Watched,
        ProgressView::Partway { .. } => ProgressView::Partway { episode: 7.5, total: Some(12) },
    })
}

fn captions() -> Vec<StripCaption> {
    listed!(StripCaption {
        StripCaption::Position { .. } => StripCaption::Position { position: 4, total: 5 },
        StripCaption::Numeral { .. } => StripCaption::Numeral { position: 4 },
        StripCaption::Partial => StripCaption::Partial,
        StripCaption::Side => StripCaption::Side,
        StripCaption::Film => StripCaption::Film,
        StripCaption::Cycle { .. } => StripCaption::Cycle { count: 10 },
        StripCaption::Branch => StripCaption::Branch,
        StripCaption::Alone => StripCaption::Alone,
    })
}

fn sections() -> Vec<SectionWire> {
    listed!(SectionWire {
        SectionWire::Line { .. } => SectionWire::Line { rows: vec![row(true)], unplaced: true },
        SectionWire::Ways { .. } => SectionWire::Ways { ways: vec![vec![row(true)]] },
        SectionWire::Group { .. } => SectionWire::Group { rows: vec![row(true)] },
    })
}

fn nexts() -> Vec<NextWire> {
    listed!(NextWire {
        NextWire::One { .. } => NextWire::One { entry: view_entry(true) },
        NextWire::Unaired { .. } => NextWire::Unaired { entry: view_entry(true) },
        NextWire::Choices { .. } => NextWire::Choices { entries: vec![view_entry(true)] },
    })
}

fn up_next_cards() -> Vec<UpNextWire> {
    listed!(UpNextWire {
        UpNextWire::Entry(_) => UpNextWire::Entry(NextWire::One { entry: view_entry(true) }),
        UpNextWire::Franchise(_) => UpNextWire::Franchise(chain_end(true)),
    })
}

/// The bodies the watch-order routes read.
pub(super) fn requests(out: &mut Map<String, Value>) {
    let mut put = |name: &str, v: Value| {
        out.insert(name.to_owned(), v);
    };
    put(
        "WatchOrderDayBody",
        request::<DayArgs>(&json!({ "today": "2026-11-02" })),
    );
    put(
        "WatchOrderUpNextBody",
        request::<UpNextArgs>(&json!({ "today": "2026-11-02", "episode": 12 })),
    );
    put(
        "RailRequest",
        request::<RailRequest>(&json!({ "rows": [], "history": [], "matches": {}, "series": {} })),
    );
    put(
        "AskRow",
        request::<AskRow>(&json!({ "row_id": "r", "ep_no": "12" })),
    );
    put(
        "HandoffMatch",
        request::<HandoffMatch>(&json!({ "id": "7158" })),
    );
    put("PlayedRow", request::<PlayedRow>(&json!({ "id": "r" })));
}

/// The names the watch-order answers carry.
pub(super) fn enums(out: &mut Map<String, Value>) {
    out.insert(
        "WatchOrder".into(),
        super::names(&listed!(WatchOrder {
            WatchOrder::Linked => WatchOrder::Linked,
            WatchOrder::Released => WatchOrder::Released,
        })),
    );
    out.insert(
        "StripKind".into(),
        super::names(&listed!(StripKind {
            StripKind::Run => StripKind::Run,
            StripKind::Side => StripKind::Side,
        })),
    );
    out.insert(
        "Seam".into(),
        super::names(&listed!(Seam {
            Seam::From => Seam::From,
            Seam::Into => Seam::Into,
        })),
    );
}
