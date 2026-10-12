//! A page hianime's origin did not serve is no answer from hianime.
//!
//! anidb.app began answering its search with a redirect to an
//! unrelated site; the transport followed it, and the parser read
//! that site's page as an empty search — a clean miss the walk
//! persisted as the provider's verdict. Every request hianime's client
//! builds from its origin — the search, the show page, the episode
//! list, the servers listing — is held to that origin: a redirect
//! that stays on it is followed, one that leaves it is not followed
//! at all, and the redirect itself, or a page another origin served
//! all the same, is a failure the walk moves past rather than an
//! answer. The embed host, its sources endpoint and the CDN are not
//! the origin and follow their redirects as before.

use super::*;
use crate::scraper::fetch::{Fetch, FetchRequest, FetchResponse};

const BASE: &str = "https://hianime.example";

const NO_RESULTS: &str = r#"<html><body><div class="film_list film_list-grid"><p>No animes found.</p></div><div id="main-sidebar"></div></body></html>"#;

const ONE_RESULT: &str = r##"<html><body>
<div class="tab-content"><div class="block_area-content block_area-list film_list film_list-grid film_list-wfeature">
<div class="film_list-wrap">
  <div class="flw-item">
    <div class="film-detail">
      <h3 class="film-name"><a href="https://hianime.at/cowboy-bebop-1281" title="Cowboy Bebop" class="dynamic-name">Cowboy Bebop</a></h3>
    </div>
  </div>
</div>
</div></div>
<div id="main-sidebar"></div>
</body></html>"##;

const EPISODES: &str = r#"{"status":true,"totalItems":1,"html":"<div class=\"ss-list\"><a title=\"Episode 1\" class=\"ssl-item ep-item\" data-number=\"1\" data-id=\"21418\" href=\"https://hianime.at/watch/cowboy-bebop-1281?ep=21418\"></a></div>"}"#;

const NO_SERVERS: &str = r#"{"status":true,"html":""}"#;

const DETAIL: &str = r#"<div class="item item-title"><span class="item-head">Aired:</span><span class="name">Apr 3, 1998 to Apr 24, 1999</span></div>"#;

/// Answers every request with `body` and a 200, reporting that the
/// transfer ended on `landed` — what a transport that followed a
/// redirect says.
struct Landed {
    landed: String,
    body: &'static str,
}

#[async_trait::async_trait]
impl Fetch for Landed {
    async fn fetch(&self, _req: &FetchRequest) -> crate::error::Result<FetchResponse> {
        Ok(FetchResponse {
            status: 200,
            body: self.body.to_string(),
            url: self.landed.clone(),
        })
    }
}

fn landed_elsewhere(body: &'static str) -> HianimeClient<Landed> {
    HianimeClient::with_base(
        Landed {
            landed: "https://unrelated.example/".into(),
            body,
        },
        BASE,
    )
}

fn parse_failure<T: std::fmt::Debug>(got: crate::error::Result<T>, what: &str) {
    match got {
        Err(AniError::ParseFailed { .. }) => {}
        other => panic!("{what}: expected a parse failure the walk moves past, got {other:?}"),
    }
}

#[tokio::test]
async fn a_search_answered_from_another_origin_is_no_answer() {
    // Even a body that reads exactly like hianime's own no-results
    // page is not hianime's verdict when another origin served it.
    for body in [NO_RESULTS, ONE_RESULT] {
        parse_failure(landed_elsewhere(body).search("cowboy bebop").await, body);
    }
}

#[tokio::test]
async fn every_origin_endpoint_answered_from_another_origin_fails() {
    parse_failure(
        landed_elsewhere(EPISODES)
            .episodes("cowboy-bebop-1281")
            .await,
        "episodes",
    );
    parse_failure(
        landed_elsewhere(NO_SERVERS).has_mode(21418, "sub").await,
        "servers",
    );
    parse_failure(
        landed_elsewhere(NO_SERVERS)
            .master_playlist_url(21418, "sub")
            .await,
        "servers for a stream",
    );
    // The year is a soft hint where the page is genuinely missing; a
    // page from another site is not a missing page.
    parse_failure(
        landed_elsewhere(DETAIL)
            .detail_year("cowboy-bebop-1281")
            .await,
        "show page",
    );
}

#[tokio::test]
async fn a_redirect_within_the_origin_is_still_the_origin_answering() {
    let client = HianimeClient::with_base(
        Landed {
            landed: format!("{BASE}/search?keyword=nohit&page=1"),
            body: NO_RESULTS,
        },
        BASE,
    );
    assert!(client
        .search("nohit")
        .await
        .expect("the origin answered")
        .is_empty());
}

/// Answers every request with `status` from the URL it was asked for
/// — what the transport reports for a redirect it did not follow.
struct Answers {
    status: u16,
}

#[async_trait::async_trait]
impl Fetch for Answers {
    async fn fetch(&self, req: &FetchRequest) -> crate::error::Result<FetchResponse> {
        Ok(FetchResponse {
            status: self.status,
            body: "<html><head><title>302 Found</title></head><body><center><h1>302 Found</h1></center></body></html>".into(),
            url: req.url.clone(),
        })
    }
}

#[tokio::test]
async fn a_redirect_the_transport_did_not_follow_is_no_answer() {
    // The redirect off the origin comes back as itself. It is the same
    // failure as an answer from elsewhere — the walk moves on and the
    // gate hears distress — not a status the provider answered with.
    for status in [301, 302, 307, 308] {
        let client = HianimeClient::with_base(Answers { status }, BASE);
        parse_failure(client.search("cowboy bebop").await, "search");
        parse_failure(client.episodes("cowboy-bebop-1281").await, "episodes");
        parse_failure(client.has_mode(21418, "sub").await, "servers");
        parse_failure(client.detail_year("cowboy-bebop-1281").await, "show page");
    }
}

#[tokio::test]
async fn the_transport_follows_a_redirect_within_the_origin_and_no_other() {
    // Through the real transport: a redirect that stays on hianime's
    // origin is followed to its page; one that leaves it is not
    // followed at all, so the other site never sees the request.
    use wiremock::matchers::{method, path, query_param};
    let Some(curl) = crate::scraper::fetch::CurlImpersonateFetch::resolve(
        None,
        &std::env::var("PATH").unwrap_or_default(),
    ) else {
        eprintln!("no curl on PATH; skipping");
        return;
    };
    let elsewhere = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(NO_RESULTS))
        .mount(&elsewhere)
        .await;
    let origin = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("keyword", "cowboy bebop"))
        .respond_with(
            wiremock::ResponseTemplate::new(302).insert_header("location", "/results?page=1"),
        )
        .mount(&origin)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/results"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(ONE_RESULT))
        .mount(&origin)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("keyword", "elsewhere"))
        .respond_with(
            wiremock::ResponseTemplate::new(302)
                .insert_header("location", format!("{}/", elsewhere.uri()).as_str()),
        )
        .mount(&origin)
        .await;
    let client = HianimeClient::with_base(curl, &origin.uri());

    let hits = client
        .search("cowboy bebop")
        .await
        .expect("a same-origin redirect is followed");
    assert_eq!(hits.len(), 1);

    parse_failure(
        client.search("elsewhere").await,
        "search redirected off the origin",
    );
    assert!(
        elsewhere
            .received_requests()
            .await
            .expect("recorded")
            .is_empty(),
        "the other origin never hears the request"
    );
}

/// Answers the servers listing with one sub row per hash in `hashes`
/// and records every request; anything else is refused.
struct Listing {
    hashes: Vec<&'static str>,
    asked: std::sync::Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl Fetch for Listing {
    async fn fetch(&self, req: &FetchRequest) -> crate::error::Result<FetchResponse> {
        self.asked.lock().expect("log").push(req.url.clone());
        if !req.url.contains("/api/theme/episode/servers") {
            return Ok(FetchResponse {
                status: 404,
                body: String::new(),
                url: req.url.clone(),
            });
        }
        let rows: String = self
            .hashes
            .iter()
            .map(|h| {
                format!(
                    r#"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"{h}\"></div>"#
                )
            })
            .collect();
        Ok(FetchResponse {
            status: 200,
            body: format!(r#"{{"status":true,"html":"{rows}"}}"#),
            url: req.url.clone(),
        })
    }
}

#[tokio::test]
async fn an_embed_url_that_names_no_origin_is_never_requested() {
    // An embed URL that does not parse as absolute — protocol-relative,
    // a bare path, a host without a scheme — names no origin. A
    // request for it would be resolved against whatever the transport
    // makes of it and follow its redirects anywhere; it is never sent.
    // The mode stays uncertain: the row may have been the playable
    // server, so this is a parse failure, never the mode's absence.
    for hash in [
        "Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvMS9zdWI=",
        "L3N0cmVhbS9tYWwvMS8xL3N1Yg==",
        "bWVnYXBsYXkuYnV6ei9zdHJlYW0vcy0yLzEvc3Vi",
    ] {
        let client = HianimeClient::with_base(
            Listing {
                hashes: vec![hash],
                asked: std::sync::Mutex::new(Vec::new()),
            },
            BASE,
        );
        parse_failure(client.has_mode(21418, "sub").await, hash);
        parse_failure(client.master_playlist_url(21418, "sub").await, hash);
        let asked = client.transport().asked.lock().expect("log").clone();
        assert!(
            asked
                .iter()
                .all(|u| u.starts_with(&format!("{BASE}/api/theme/episode/servers"))),
            "{hash}: only the listing is asked, never the embed: {asked:?}"
        );
    }
}
