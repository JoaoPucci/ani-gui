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

/// Answers every request with `status` and `body` from the URL it was
/// asked for — what the transport reports for a redirect it did not
/// follow, and for a page served where it was asked.
struct Answers {
    status: u16,
    body: &'static str,
}

#[async_trait::async_trait]
impl Fetch for Answers {
    async fn fetch(&self, req: &FetchRequest) -> crate::error::Result<FetchResponse> {
        Ok(FetchResponse {
            status: self.status,
            body: self.body.to_string(),
            url: req.url.clone(),
        })
    }
}

const NGINX_302: &str = "<html>\n<head><title>302 Found</title></head>\n<body>\n<center><h1>302 Found</h1></center>\n<hr><center>nginx</center>\n</body>\n</html>\n";

#[tokio::test]
async fn a_redirect_the_transport_did_not_follow_is_no_answer() {
    // A redirect off the origin is not followed, so the 302 itself is
    // what comes back. It is the same failure as an answer from
    // elsewhere — the walk moves on — not a status the provider
    // answered with.
    let client = AnidbClient::new(Answers {
        status: 302,
        body: NGINX_302,
    });
    let err = client.search("one piece").await.expect_err("no answer");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    let err = client
        .detail_year("one-piece-69")
        .await
        .expect_err("no answer");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

/// The languages listing from the origin, and the embed page as
/// `embed` says: its status, body, and the URL it came from.
struct EmbedAnswers {
    status: u16,
    body: &'static str,
    landed: Option<&'static str>,
}

#[async_trait::async_trait]
impl Fetch for EmbedAnswers {
    async fn fetch(&self, req: &FetchRequest) -> crate::error::Result<FetchResponse> {
        if req.url.contains("/languages") {
            return Ok(FetchResponse {
                status: 200,
                body: fixture("languages_op.json"),
                url: req.url.clone(),
            });
        }
        Ok(FetchResponse {
            status: self.status,
            body: self.body.to_string(),
            url: self.landed.map_or_else(|| req.url.clone(), str::to_string),
        })
    }
}

const EMBED_WITH_PLAYLIST: &str =
    "<script>jwplayer('p').setup({ file: 'https://cdn.example/op/master.m3u8' });</script>";

#[tokio::test]
async fn an_embed_page_that_is_not_the_embed_is_no_answer() {
    // The episode's languages listing names an embed; what comes back
    // from it decides only whether the walk moves on. A page without
    // the player's playlist (a parked domain), a page another origin
    // served, and a redirect the transport did not follow are all the
    // site failing, not the episode missing: each fails over like the
    // other anidb pages instead of failing the play.
    for (status, body, landed) in [
        (
            200,
            "<html><body>This domain is for sale</body></html>",
            None,
        ),
        (200, EMBED_WITH_PLAYLIST, Some("https://parked.example/")),
        (302, NGINX_302, None),
    ] {
        let err = AnidbClient::new(EmbedAnswers {
            status,
            body,
            landed,
        })
        .master_playlist_url(9001, "sub")
        .await
        .expect_err("no stream");
        assert!(
            matches!(err, AniError::ParseFailed { .. }),
            "status {status}, landed {landed:?}: {err:?}"
        );
    }
}

#[tokio::test]
async fn the_embed_page_still_yields_its_playlist() {
    let got = AnidbClient::new(EmbedAnswers {
        status: 200,
        body: EMBED_WITH_PLAYLIST,
        landed: None,
    })
    .master_playlist_url(9001, "sub")
    .await
    .expect("the embed answers");
    assert_eq!(got.master_url, "https://cdn.example/op/master.m3u8");
}

#[tokio::test]
async fn the_transport_follows_a_redirect_within_the_origin_and_no_other() {
    // Through the real transport: a redirect that stays on anidb's
    // origin is followed to its page; one that leaves it is not
    // followed at all, so the other site never sees the request.
    use wiremock::matchers::{method, path};
    let Some(curl) = crate::scraper::fetch::CurlImpersonateFetch::resolve(
        None,
        &std::env::var("PATH").unwrap_or_default(),
    ) else {
        eprintln!("no curl on PATH; skipping");
        return;
    };
    let elsewhere = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(fixture("browse_one_piece.html")),
        )
        .mount(&elsewhere)
        .await;
    let anidb = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/browse"))
        .and(wiremock::matchers::query_param("q", "one piece"))
        .respond_with(
            wiremock::ResponseTemplate::new(302).insert_header("location", "/results?page=1"),
        )
        .mount(&anidb)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/results"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(fixture("browse_one_piece.html")),
        )
        .mount(&anidb)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/browse"))
        .and(wiremock::matchers::query_param("q", "elsewhere"))
        .respond_with(
            wiremock::ResponseTemplate::new(302)
                .insert_header("location", format!("{}/", elsewhere.uri()).as_str()),
        )
        .mount(&anidb)
        .await;
    let client = AnidbClient::with_base(curl, &anidb.uri());

    let hits = client
        .search("one piece")
        .await
        .expect("a same-origin redirect is followed");
    assert_eq!(hits.len(), 3);

    let err = client
        .search("elsewhere")
        .await
        .expect_err("a redirect off the origin is no answer");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    assert!(
        elsewhere
            .received_requests()
            .await
            .expect("recorded")
            .is_empty(),
        "the other origin never hears the request"
    );
}
