//! The genre gate's pieces: the memo the pick runs over, what a page's
//! Genres row says, and the verdict a pool with an unread page ends on.

use super::*;
use crate::commands::play_native_year::{genre_of, Genre};
use crate::scraper::provider::{EntryPage, StreamSource};
use crate::scraper::table_provider::{listing_of, Answer, Asked, TableProvider};
use proptest::prelude::*;

const YEAR: u32 = 2020;

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
