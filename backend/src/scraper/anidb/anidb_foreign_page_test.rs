//! A page the provider's origin did not serve is no answer from the
//! provider. anidb.app began answering its search with a 302 to an
//! unrelated site, the transport followed it, and the browse parser
//! read that site's home page — a page with layout grids and no
//! result cards — as an empty search. The walk took that for a clean
//! miss: it stopped without asking the fallback and persisted the
//! miss as anidb's verdict on the show.
//!
//! Two guards close it. A request to the provider's origin must be
//! answered from that origin: the transport reports the URL the
//! transfer ended on, and an answer from anywhere else fails loud
//! for every endpoint, not only the search. And a zero-hit page must
//! show the browse page's own markers, not markup any site carries.

use super::*;
use crate::scraper::fetch::{Fetch, FetchRequest, FetchResponse};

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/anidb")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Answers every request with `body` and a 200, reporting that the
/// transfer ended on `landed` — what the production transport's
/// effective URL says after it follows a redirect.
struct Landed {
    landed: &'static str,
    body: String,
}

#[async_trait::async_trait]
impl Fetch for Landed {
    async fn fetch(&self, _req: &FetchRequest) -> crate::error::Result<FetchResponse> {
        Ok(FetchResponse {
            status: 200,
            body: self.body.clone(),
            url: self.landed.to_string(),
        })
    }
}

fn redirected(body: String) -> AnidbClient<Landed> {
    AnidbClient::new(Landed {
        landed: "https://anilab.so/",
        body,
    })
}

#[test]
fn the_page_the_search_was_redirected_to_is_not_an_empty_search() {
    // The recorded page carries `class="grid grid-cols-…"` layout
    // grids and no result cards. Read as the browse page's empty
    // grid it is a clean miss; it is a page this parser does not
    // recognize, and says so.
    assert!(matches!(
        parse_browse(&fixture("redirected_browse_anilab.html")),
        Err(AniError::ParseFailed { .. })
    ));
}

#[tokio::test]
async fn a_search_answered_from_another_origin_is_no_answer() {
    // Even a body that reads exactly like anidb's own no-results page
    // is not anidb's verdict when another origin served it.
    for body in [
        fixture("redirected_browse_anilab.html"),
        fixture("browse_empty.html"),
    ] {
        let err = redirected(body)
            .search("one piece")
            .await
            .expect_err("another origin answered");
        assert!(
            matches!(err, AniError::ParseFailed { .. }),
            "an answer from elsewhere is a failure the walk moves past, got {err:?}"
        );
    }
}

#[tokio::test]
async fn every_origin_endpoint_answered_from_another_origin_fails() {
    // The class, not the one parser: each endpoint that can answer
    // empty-but-OK refuses a foreign answer the same way.
    let err = redirected(r#"{"episodes":[]}"#.into())
        .episodes("one-piece-69")
        .await
        .expect_err("episodes from elsewhere");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");

    let err = redirected(r#"{"languages":[]}"#.into())
        .has_mode(9001, "sub")
        .await
        .expect_err("languages from elsewhere");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");

    let err = redirected(r#"{"languages":[]}"#.into())
        .master_playlist_url(9001, "sub")
        .await
        .expect_err("languages from elsewhere");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");

    // The year is a soft hint where the page is genuinely missing;
    // a page from another site is not a missing page, and reading
    // its year (or its lack of one) would steer the pick.
    let err = redirected(fixture("detail_one_piece.html"))
        .detail_year("one-piece-69")
        .await
        .expect_err("detail page from elsewhere");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

#[tokio::test]
async fn a_redirect_within_the_origin_is_still_the_origin_answering() {
    let client = AnidbClient::new(Landed {
        landed: "https://anidb.app/browse?q=nohit&page=1",
        body: fixture("browse_empty.html"),
    });
    assert!(client
        .search("nohit")
        .await
        .expect("the origin answered")
        .is_empty());
}

#[tokio::test]
async fn the_embed_page_is_not_held_to_the_origin() {
    // The embed lives on its own host by design; only requests built
    // from the provider's origin are held to it.
    struct Split;
    #[async_trait::async_trait]
    impl Fetch for Split {
        async fn fetch(&self, req: &FetchRequest) -> crate::error::Result<FetchResponse> {
            let body = if req.url.contains("/languages") {
                fixture("languages_op.json")
            } else {
                fixture("embed_op.html")
            };
            Ok(FetchResponse {
                status: 200,
                body,
                url: req.url.clone(),
            })
        }
    }
    let got = AnidbClient::new(Split)
        .master_playlist_url(9001, "sub")
        .await
        .expect("the embed host answers for itself");
    assert_eq!(got.master_url, "https://cdn.example/op/master.m3u8");
}
