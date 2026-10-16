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
