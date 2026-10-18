//! The catalogue the native-walk test files share
//! (`play_native_resolve_test`, `play_native_walk_test`) — compiled
//! only under `cfg(test)`.
//!
//! One provider answering from tables: searches answer by what the
//! query contains, and the shows behind them carry the listings,
//! modes and streams every walk test reads.

use crate::error::AniError;
use crate::scraper::provider::{BrowseHit, EpisodeRef};
use crate::scraper::table_provider::{Answer, TableProvider};

/// What a search whose query matches a needle answers.
#[derive(Clone, Copy)]
pub(crate) enum Pool {
    /// These `(slug, title)` hits.
    Hits(&'static [(&'static str, &'static str)]),
    /// The provider refusing this client — a challenge page.
    Blocked,
}

/// The show most tests resolve: three listed rows, the third a
/// "2.5" recap, the second streaming in sub.
pub(crate) const THE_SHOW: Pool = Pool::Hits(&[("the-show-77", "The Show")]);

/// The hits a pool of `(slug, title)` pairs names.
pub(crate) fn hits(entries: &[(&str, &str)]) -> Vec<BrowseHit> {
    entries
        .iter()
        .map(|(slug, title)| BrowseHit {
            slug: (*slug).to_string(),
            title: (*title).to_string(),
            kind: None,
            jname: None,
        })
        .collect()
}

fn row(id: u64, number: u32, tag: Option<&str>) -> EpisodeRef {
    EpisodeRef {
        id,
        number,
        number2: tag.map(str::to_string),
        former_tag: None,
        title: None,
    }
}

/// The master every streaming episode of the catalogue plays from.
pub(crate) const MASTER: &str = "https://cdn.example/x/master.m3u8";

/// The catalogue, with searches answered by `searches` — first
/// matching needle wins, an unmatched query is the clean no-results
/// answer.
///
/// - `…-77`: rows 701 (#1), 702 (#2), 725 (#3, tagged "2.5"); 702
///   and 725 stream in sub; 701 is listed and not served.
/// - `…-99`: seven rows, past the distance threshold of the tests'
///   expected three, so a pick REJECTS the pool rather than failing
///   its probe.
/// - `…-88`: a continuation entry keeping the franchise's cumulative
///   numbering — two rows numbered 41 and 42 (the TYBW fourth-cour
///   shape); 8841 streams in sub.
///
/// No streaming episode carries dub: asked for it, the embed step
/// answers no stream, as a site whose only embed is the Japanese
/// audio does.
pub(crate) fn scripted(searches: &[(&str, Pool)]) -> TableProvider {
    let mut table = TableProvider::new()
        .listing(
            77,
            vec![
                row(701, 1, None),
                row(702, 2, None),
                row(725, 3, Some("2.5")),
            ],
        )
        .listing(
            99,
            (1..=7)
                .map(|n| row(9900 + u64::from(n), n, None))
                .collect::<Vec<_>>(),
        )
        .listing(88, vec![row(8841, 41, None), row(8842, 42, None)]);
    for id in [702, 725, 8841] {
        table = table
            .stream(id, "sub", MASTER)
            .stream_from(id, "dub", Answer::fails(|| AniError::NoResults))
            .mode(id, "sub", true)
            .mode(id, "dub", false);
    }
    for (needle, pool) in searches {
        table = match pool {
            Pool::Hits(entries) => table.hits(needle, hits(entries)),
            Pool::Blocked => {
                table.hits(needle, Answer::fails(|| AniError::Upstream { status: 403 }))
            }
        };
    }
    table
}

/// A split entry the walk stitches: "Stitched Show" (one episode,
/// show 4) and its "2nd Stage" (show 10465), numbered on from 2 and
/// opening on a recap tagged "1.5". Stitched, the recap is slot 2,
/// tagged "1.5" now and "2.5" by an earlier version; episodes 2 to 12
/// are slots 3 to 13. Kitsu counts twelve episodes, premiering 2026
/// as an ONA. Every row streams in sub.
pub(crate) const STITCHED: &str = "Stitched Show";
/// The show key the stitched entry plays under: its first part's.
pub(crate) const STITCHED_SLUG: &str = "stitched-show-4";
/// The stitched recap's episode id.
pub(crate) const STITCHED_RECAP: u64 = 10_465_001;

pub(crate) fn stitched_recap() -> TableProvider {
    let first = vec![row(4001, 1, None)];
    let mut second = vec![row(STITCHED_RECAP, 1, Some("1.5"))];
    second.extend((2..=12).map(|n| row(10_465_000 + u64::from(n), n, Some(&n.to_string()))));
    let ids: Vec<u64> = first.iter().chain(&second).map(|e| e.id).collect();
    let mut table = TableProvider::new()
        .hits(
            STITCHED,
            hits(&[
                (STITCHED_SLUG, STITCHED),
                ("stitched-show-2nd-stage-10465", "Stitched Show 2nd Stage"),
            ]),
        )
        .listing(4, first)
        .listing(10465, second)
        .year(4, Some(2026))
        .year(10465, Some(2026));
    for id in ids {
        table = table.stream(id, "sub", MASTER).mode(id, "sub", true);
    }
    table
}

/// [`stitched_recap`] with a second recap after the entry's episode 2:
/// stitched, the opening recap is slot 2, "1.5" now and "2.5" before,
/// and the new one is slot 4, "2.5" now. The former tag of the one is
/// the tag of the other.
pub(crate) fn stitched_shared_former() -> TableProvider {
    let first = vec![row(4001, 1, None)];
    let mut second = vec![
        row(STITCHED_RECAP, 1, Some("1.5")),
        row(10_465_002, 2, Some("2")),
        row(10_465_025, 3, Some("2.5")),
    ];
    second.extend((3..=12).map(|n| row(10_465_000 + u64::from(n), n + 1, Some(&n.to_string()))));
    let ids: Vec<u64> = first.iter().chain(&second).map(|e| e.id).collect();
    let mut table = TableProvider::new()
        .hits(
            STITCHED,
            hits(&[
                (STITCHED_SLUG, STITCHED),
                ("stitched-show-2nd-stage-10465", "Stitched Show 2nd Stage"),
            ]),
        )
        .listing(4, first)
        .listing(10465, second)
        .year(4, Some(2026))
        .year(10465, Some(2026));
    for id in ids {
        table = table.stream(id, "sub", MASTER).mode(id, "sub", true);
    }
    table
}

/// A continuation the provider numbers on from 13 (offset 12) and that
/// opens on a recap tagged "12.5": the entry's 0.5, which an earlier
/// version advertised as "12.5" and resolved by asking for "24.5".
/// Kitsu counts two episodes. Every row streams in sub.
pub(crate) const CUMULATIVE: &str = "Cumulative Show";
pub(crate) const CUMULATIVE_SLUG: &str = "cumulative-show-66";

pub(crate) fn cumulative_opening_recap() -> TableProvider {
    let rows = vec![
        row(6601, 1, Some("12.5")),
        row(6602, 2, Some("13")),
        row(6603, 3, Some("14")),
    ];
    let mut table = TableProvider::new()
        .hits(CUMULATIVE, hits(&[(CUMULATIVE_SLUG, CUMULATIVE)]))
        .listing(66, rows)
        .year(66, Some(2026));
    for id in [6601, 6602, 6603] {
        table = table.stream(id, "sub", MASTER).mode(id, "sub", true);
    }
    table
}

/// [`cumulative_opening_recap`] as a show whose title carries cour
/// evidence, "2nd Season": a watch of it with a Kitsu id asks Kitsu
/// whether the pairing holds before it stores the mapping.
pub(crate) const CUMULATIVE_S2: &str = "Cumulative Show 2nd Season";
pub(crate) const CUMULATIVE_S2_SLUG: &str = "cumulative-show-2nd-season-67";

pub(crate) fn cumulative_opening_recap_season() -> TableProvider {
    let rows = vec![
        row(6701, 1, Some("12.5")),
        row(6702, 2, Some("13")),
        row(6703, 3, Some("14")),
    ];
    let mut table = TableProvider::new()
        .hits(CUMULATIVE_S2, hits(&[(CUMULATIVE_S2_SLUG, CUMULATIVE_S2)]))
        .listing(67, rows)
        .year(67, Some(2026));
    for id in [6701, 6702, 6703] {
        table = table.stream(id, "sub", MASTER).mode(id, "sub", true);
    }
    table
}
