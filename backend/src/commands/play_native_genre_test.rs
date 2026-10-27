//! The genre gate, case by case: no pick serves a listing its own
//! entry page files under Hentai, or one whose page lists no genre,
//! dated or undated, counted or countless — and an undated pick pays
//! for it with the page of each listing it serves and nothing more.

use super::*;
use crate::commands::play_native::pick_candidate_titled;
use crate::commands::play_native_versions::VersionKind;
use crate::commands::play_native_year::{genre_of, Genre};
use crate::scraper::provider::{EntryPage, StreamSource};
use crate::scraper::table_provider::{listing_of, Answer, Asked, TableProvider};
use proptest::prelude::*;

const YEAR: u32 = 2020;

fn hit(slug: &str, title: &str) -> BrowseHit {
    BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: Some("TV".into()),
        jname: None,
    }
}

/// Listings of `count` episodes each, every page from the entry's year
/// — read only where the entry has one.
fn site(shows: &[(u64, u32)]) -> TableProvider {
    shows.iter().fold(TableProvider::new(), |t, (id, count)| {
        t.listing(*id, listing_of(*id, *count))
            .year(*id, Some(YEAR))
    })
}

/// The pick for "The Show", with `count` episodes and `year`, over
/// `hits`, the entry also known as each of `aliases`.
async fn pick(
    client: &TableProvider,
    hits: &[BrowseHit],
    count: Option<u32>,
    year: Option<u32>,
    aliases: &[&str],
) -> Result<PickedShow> {
    let titles: Vec<&str> = std::iter::once("The Show")
        .chain(aliases.iter().copied())
        .collect();
    pick_candidate_titled(client, hits, count, "The Show", &titles, year, Some("TV")).await
}

fn pages_read(client: &TableProvider) -> Vec<String> {
    client
        .asked()
        .into_iter()
        .filter_map(|a| match a {
            Asked::Year(slug) => Some(slug),
            _ => None,
        })
        .collect()
}

fn probes(client: &TableProvider) -> Vec<String> {
    client
        .asked()
        .into_iter()
        .filter_map(|a| match a {
            Asked::Episodes(slug) => Some(slug),
            _ => None,
        })
        .collect()
}

/// Every shape of entry an undated pick can be: with a count, with a
/// single video's, and with none.
const UNDATED_COUNTS: [Option<u32>; 3] = [Some(12), Some(1), None];

/// The listing an undated pick would serve is read for its genre, once:
/// one page for the pick, whatever shape the entry is.
#[tokio::test]
async fn an_undated_pick_reads_the_page_of_the_listing_it_serves_once() {
    for count in UNDATED_COUNTS {
        let n = count.unwrap_or(12);
        let client = site(&[(1, n), (2, n)]);
        let hits = [hit("the-show-1", "The Show"), hit("other-2", "Other")];
        let p = pick(&client, &hits, count, None, &[])
            .await
            .unwrap_or_else(|e| panic!("{count:?}: {e:?}"));
        assert_eq!(p.hit.slug, "the-show-1", "{count:?}");
        assert_eq!(pages_read(&client), ["the-show-1"], "{count:?}");
    }
}

/// An undated entry's listing its page files under Hentai is never the
/// pick: alone in its pool, the pool holds nothing the entry could be —
/// the clean miss; beside a listing the entry's name vouches for, that
/// one is picked. Counted, a single video, or countless alike, and the
/// listings already probed are not asked again.
#[tokio::test]
async fn an_undated_pick_never_serves_a_hentai() {
    for count in UNDATED_COUNTS {
        let n = count.unwrap_or(12);
        let alone = site(&[(1, n)]).genres(1, &["Hentai"]);
        let hits = [hit("the-show-1", "The Show")];
        match pick(&alone, &hits, count, None, &[]).await {
            Ok(p) => panic!("{count:?}: picked {}", p.hit.slug),
            Err(e) => assert!(matches!(e, AniError::NoResults), "{count:?}: {e:?}"),
        }

        let beside = site(&[(1, n), (2, n)]).genres(1, &["Romance", "hentai"]);
        let hits = [
            hit("the-show-1", "The Show"),
            hit("the-show-tv-2", "The Show TV"),
        ];
        let p = pick(&beside, &hits, count, None, &["The Show TV"])
            .await
            .unwrap_or_else(|e| panic!("{count:?}: {e:?}"));
        assert_eq!(p.hit.slug, "the-show-tv-2", "{count:?}");
        assert_eq!(
            pages_read(&beside),
            ["the-show-1", "the-show-tv-2"],
            "{count:?}"
        );
        let probed = probes(&beside);
        let once: std::collections::BTreeSet<&String> = probed.iter().collect();
        assert_eq!(
            probed.len(),
            once.len(),
            "{count:?}: a listing probed twice: {probed:?}"
        );
    }
}

/// A page that lists no genre — a missing page, or one whose markup no
/// longer carries the row — says nothing about whether its listing is a
/// hentai, and its listing is not taken as safe: the next candidate is
/// picked where there is one, and a pool left with none is the answered
/// dead end on a listing that may have been the entry's, never the
/// clean miss the availability cache persists.
#[tokio::test]
async fn an_undated_pick_never_takes_an_unread_page_as_safe() {
    for count in UNDATED_COUNTS {
        let n = count.unwrap_or(12);
        let alone = site(&[(1, n)]).genres(1, &[]);
        let hits = [hit("the-show-1", "The Show")];
        match pick(&alone, &hits, count, None, &[]).await {
            Ok(p) => panic!("{count:?}: picked {}", p.hit.slug),
            Err(e) => assert!(
                matches!(e, AniError::Upstream { status: 404 }),
                "{count:?}: {e:?}"
            ),
        }

        let beside = site(&[(1, n), (2, n)]).genres(1, &[]);
        let hits = [
            hit("the-show-1", "The Show"),
            hit("the-show-tv-2", "The Show TV"),
        ];
        let p = pick(&beside, &hits, count, None, &["The Show TV"])
            .await
            .unwrap_or_else(|e| panic!("{count:?}: {e:?}"));
        assert_eq!(p.hit.slug, "the-show-tv-2", "{count:?}");
    }
}

/// A pool whose other hits the entry's names refuse is still no refusal
/// by name once its one named listing went unread: that listing may
/// have been the entry's.
#[tokio::test]
async fn a_refusal_beside_an_unread_page_is_the_answered_dead_end() {
    let client = site(&[(1, 12), (2, 12)]).genres(1, &[]);
    let hits = [hit("the-show-1", "The Show"), hit("other-2", "Other")];
    match pick(&client, &hits, Some(12), None, &[]).await {
        Ok(p) => panic!("picked {}", p.hit.slug),
        Err(e) => assert!(matches!(e, AniError::Upstream { status: 404 }), "{e:?}"),
    }
}

/// A dated entry's year filter drops a listing whose page lists no
/// genre as it drops a hentai, and a pool left with nothing is the
/// answered dead end rather than the clean miss; another year's listing
/// is out on its year, whatever its page lists, and still the clean
/// miss. The pick reads no page beyond the filter's.
#[tokio::test]
async fn a_dated_pick_never_takes_an_unread_page_as_safe() {
    for count in [Some(12), Some(1), None] {
        let n = count.unwrap_or(12);
        let unread = site(&[(1, n)]).genres(1, &[]);
        let hits = [hit("the-show-1", "The Show")];
        match pick(&unread, &hits, count, Some(YEAR), &[]).await {
            Ok(p) => panic!("{count:?}: picked {}", p.hit.slug),
            Err(e) => assert!(
                matches!(e, AniError::Upstream { status: 404 }),
                "{count:?}: {e:?}"
            ),
        }

        let other_year = site(&[(1, n)]).genres(1, &[]).year(1, Some(YEAR - 5));
        match pick(&other_year, &hits, count, Some(YEAR), &[]).await {
            Ok(p) => panic!("{count:?}: picked {}", p.hit.slug),
            Err(e) => assert!(matches!(e, AniError::NoResults), "{count:?}: {e:?}"),
        }

        let beside = site(&[(1, n), (2, n)]).genres(1, &[]);
        let hits = [
            hit("the-show-1", "The Show"),
            hit("the-show-tv-2", "The Show TV"),
        ];
        let p = pick(&beside, &hits, count, Some(YEAR), &["The Show TV"])
            .await
            .unwrap_or_else(|e| panic!("{count:?}: {e:?}"));
        assert_eq!(p.hit.slug, "the-show-tv-2", "{count:?}");
        assert_eq!(
            pages_read(&beside),
            ["the-show-1", "the-show-tv-2"],
            "{count:?}"
        );
    }
}

/// A copy of the pick under another version tag is served too — the
/// user can switch to it — so its page is read as well, and a copy its
/// page files under Hentai is no version of the pick. The entry names
/// the tagged copy, so an undated pick with a count may take it.
#[tokio::test]
async fn a_version_filed_under_hentai_is_no_version_of_the_pick() {
    let aliases = ["The Show (Uncensored)"];
    let hits = [
        hit("the-show-1", "The Show"),
        hit("the-show-2", "The Show (Uncensored)"),
    ];
    let both = site(&[(1, 12), (2, 12)]);
    let p = pick(&both, &hits, Some(12), None, &aliases)
        .await
        .expect("picked");
    let kinds: Vec<VersionKind> = p.versions.iter().map(|v| v.kind).collect();
    assert_eq!(kinds, [VersionKind::Standard, VersionKind::Uncensored]);

    // The pick plays the uncensored copy, so the plain one is only a
    // version beside it: read, and dropped.
    assert_eq!(p.hit.slug, "the-show-2");
    let plain = site(&[(1, 12), (2, 12)]).genres(1, &["Hentai"]);
    let p = pick(&plain, &hits, Some(12), None, &aliases)
        .await
        .expect("picked");
    assert_eq!(p.hit.slug, "the-show-2");
    assert!(p.versions.is_empty(), "{:?}", p.versions);

    let hentai = site(&[(1, 12), (2, 12)]).genres(2, &["Hentai"]);
    let p = pick(&hentai, &hits, Some(12), None, &aliases)
        .await
        .expect("picked");
    assert_eq!(p.hit.slug, "the-show-1");
    assert!(p.versions.is_empty(), "{:?}", p.versions);
}

/// A page that answers a block is the provider refusing this client:
/// the pick stops there, asking for nothing more, as the year filter
/// does.
#[tokio::test]
async fn a_blocked_page_stops_the_pick() {
    let client =
        site(&[(1, 12), (2, 12)]).year(1, Answer::fails(|| AniError::Upstream { status: 403 }));
    let hits = [
        hit("the-show-1", "The Show"),
        hit("the-show-tv-2", "The Show TV"),
    ];
    match pick(&client, &hits, Some(12), None, &["The Show TV"]).await {
        Ok(p) => panic!("picked {}", p.hit.slug),
        Err(e) => assert!(matches!(e, AniError::Upstream { status: 403 }), "{e:?}"),
    }
    assert_eq!(pages_read(&client), ["the-show-1"]);
}

/// The memo answers a listing and a page it has served from memory, and
/// a listing that answered not-found as that answer again; a block it
/// asks again. Everything else passes through.
#[tokio::test]
async fn the_memo_asks_once_for_what_it_served() {
    let client = TableProvider::new()
        .listing(1, listing_of(1, 3))
        .year(1, Some(YEAR))
        .genres(1, &["Drama"]);
    let memo = Memo::new(&client);
    for _ in 0..2 {
        assert_eq!(memo.episodes("show-1").await.expect("listing").len(), 3);
        let page = memo.detail_page("show-1").await.expect("page");
        assert_eq!(page.year, Some(YEAR));
        assert_eq!(memo.detail_year("show-1").await.expect("year"), Some(YEAR));
        assert!(matches!(
            memo.episodes("show-9").await,
            Err(AniError::Upstream { status: 404 })
        ));
    }
    assert_eq!(probes(&client), ["show-1", "show-9"]);
    assert_eq!(pages_read(&client), ["show-1"]);
    assert_eq!(memo.id(), client.id());
    assert_eq!(memo.label(), client.label());
    assert!(memo.search("x").await.expect("search").is_empty());
    assert!(memo.has_mode(1, "sub").await.is_err());
    assert!(memo.master_playlist_url(1, "sub").await.is_err());
    assert!(memo.playlist("u", None).await.is_err());
    assert!(memo.playlist_at("u", None).await.is_err());
    assert!(memo.stream_for(1, "sub", "best").await.is_err());
    let source = StreamSource {
        master_url: "u".into(),
        referer: None,
        subtitles: Vec::new(),
    };
    assert!(memo.quality_stream_url(&source, "best").await.is_err());
    assert_eq!(memo.last_attempt_at(), client.last_attempt_at());
    memo.bound_attempt(None);

    let blocked =
        TableProvider::new().listing(1, Answer::fails(|| AniError::Upstream { status: 403 }));
    let memo = Memo::new(&blocked);
    for _ in 0..2 {
        assert!(memo.episodes("show-1").await.is_err());
    }
    assert_eq!(probes(&blocked), ["show-1", "show-1"]);
}

/// A pick made again leaves the refused listings out of the head it
/// first considered, and no hit past that head takes their place, as
/// none takes the place of a hit the year filter leaves out: six
/// listings named for the entry, each filed under Hentai, cost the
/// first five a probe and a page each, and the sixth nothing.
#[tokio::test]
async fn a_pick_made_again_stays_within_its_first_head() {
    let shows: Vec<(u64, u32)> = (1..=6).map(|id| (id, 12)).collect();
    let client = (1..=6).fold(site(&shows), |t, id| t.genres(id, &["Hentai"]));
    let hits: Vec<BrowseHit> = (1..=6)
        .map(|id| hit(&format!("the-show-{id}"), "The Show"))
        .collect();
    match pick(&client, &hits, Some(12), None, &[]).await {
        Ok(p) => panic!("picked {}", p.hit.slug),
        Err(e) => assert!(matches!(e, AniError::NoResults), "{e:?}"),
    }
    let first_five: Vec<String> = (1..=5).map(|id| format!("the-show-{id}")).collect();
    assert_eq!(probes(&client), first_five);
    assert_eq!(pages_read(&client), first_five);
}

/// A dated pool's hit that could never be picked — refused by the
/// entry's titles, probed only as evidence — makes no miss the dead end
/// for its page listing no genre: the refusal by name stands.
#[tokio::test]
async fn an_unread_page_on_evidence_leaves_the_refusal_by_name() {
    let client = site(&[(1, 12), (2, 12)]).genres(1, &[]);
    let hits = [
        hit("the-show-season-2-1", "The Show Season 2"),
        hit("other-2", "Other"),
    ];
    match pick(&client, &hits, Some(12), Some(YEAR), &[]).await {
        Ok(p) => panic!("picked {}", p.hit.slug),
        Err(e) => assert!(matches!(e, AniError::RefusedByName), "{e:?}"),
    }
    assert_eq!(pages_read(&client), ["the-show-season-2-1", "other-2"]);
}

/// One of the verdicts a pick ends on, by `kind`, a status for the
/// upstream one.
fn verdict(kind: u8, status: u16) -> AniError {
    match kind {
        0 => AniError::NoResults,
        1 => AniError::RefusedByName,
        2 => AniError::Network,
        3 => AniError::GateRefused,
        4 => AniError::Timeout,
        _ => AniError::Upstream { status },
    }
}

proptest! {
    /// The memo keeps a probe's not-found-shaped answer and a transport
    /// death, each as itself, and no block or other failure.
    #[test]
    fn the_memo_keeps_a_dead_listing_and_nothing_else(kind in 0u8..6, status in 400u16..600) {
        let e = verdict(kind, status);
        let kept = super::memo::replayed(&e).map(|k| format!("{k:?}"));
        let keeps = matches!(e, AniError::Network)
            || matches!(e, AniError::Upstream { .. } if !e.is_provider_block());
        prop_assert_eq!(kept, keeps.then(|| format!("{e:?}")));
    }

    /// With no unread page left out, every verdict stands; with one, a
    /// miss — clean or by name — is the answered dead end on a listing
    /// that may have been the entry's, and nothing else changes.
    #[test]
    fn an_unread_page_makes_a_miss_the_answered_dead_end(
        kind in 0u8..6,
        status in 400u16..600,
        unread in any::<bool>(),
    ) {
        let v = verdict(kind, status);
        let before = format!("{v:?}");
        let miss = matches!(v, AniError::NoResults | AniError::RefusedByName);
        let after = format!("{:?}", unread_miss(v, unread));
        if unread && miss {
            prop_assert_eq!(after, format!("{:?}", AniError::Upstream { status: 404 }));
        } else {
            prop_assert_eq!(after, before);
        }
    }

    /// A page is Hentai when any genre it lists is, case aside; unread
    /// when it lists none; listed otherwise.
    #[test]
    fn a_page_s_genre_is_what_its_genre_row_lists(
        genres in proptest::collection::vec(
            prop_oneof![Just("Hentai".to_string()), Just("HENTAI".to_string()), "[A-Za-z ]{1,12}"],
            0..5,
        ),
    ) {
        let page = EntryPage { genres: genres.clone(), ..EntryPage::default() };
        let expected = if genres.iter().any(|g| g.eq_ignore_ascii_case("hentai")) {
            Genre::Hentai
        } else if genres.is_empty() {
            Genre::Unread
        } else {
            Genre::Listed
        };
        prop_assert_eq!(genre_of(&page), expected);
    }
}
