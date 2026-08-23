//! Shared scriptable anidb provider for the native-walk test files
//! (`play_native_resolve_test`, `play_native_walk_test`) — compiled
//! only under `cfg(test)`.

use crate::scraper::anidb::{AnidbFetch, FetchResponse};
use std::sync::Mutex;

/// Scriptable provider: browse responses keyed by query substring,
/// one shared episodes/languages/embed/master catalogue, and a URL
/// log so tests can assert how far the walk went.
pub(crate) struct Provider {
    /// `(query substring, browse HTML)` — first match wins. A `!`
    /// body means "answer 403 with the interstitial".
    browse: &'static [(&'static str, &'static str)],
    log: Mutex<Vec<String>>,
}

impl Provider {
    pub(crate) fn new(browse: &'static [(&'static str, &'static str)]) -> Self {
        Self {
            browse,
            log: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn requests(&self) -> Vec<String> {
        self.log.lock().expect("log").clone()
    }
}

pub(crate) fn browse_page(entries: &[(&str, &str)]) -> String {
    entries
        .iter()
        .map(|(slug, title)| format!("<a href=\"/anime/{slug}\"><img alt=\"{title}\"/></a>\n"))
        .collect()
}

#[async_trait::async_trait]
impl AnidbFetch for Provider {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        self.log.lock().expect("log").push(url.to_string());
        if let Some(q) = url.split("browse?q=").nth(1) {
            for (needle, body) in self.browse {
                if q.contains(needle) {
                    if *body == "!" {
                        return Ok(FetchResponse {
                            status: 403,
                            body: "Just a moment".into(),
                        });
                    }
                    return Ok(FetchResponse {
                        status: 200,
                        body: (*body).to_string(),
                    });
                }
            }
            // The genuine no-results shape: an unmatched query is
            // the provider ANSWERING absence, and the zero-hit
            // contract only reads absence off a page that shows the
            // browse shape.
            return Ok(FetchResponse {
                status: 200,
                body: r#"<div class="grid"><p>No results.</p></div>"#.to_string(),
            });
        }
        if url.contains("/api/frontend/anime/77/episodes") {
            return Ok(FetchResponse {
                status: 200,
                body: r#"{"episodes":[{"id":701,"number":1},{"id":702,"number":2},{"id":725,"number":3,"number2":2.5}]}"#
                    .into(),
            });
        }
        if url.contains("/api/frontend/anime/99/episodes") {
            // Seven listed episodes against the tests' expected 3:
            // past the distance threshold, so the pick REJECTS this
            // pool — the answered NoResults verdict — rather than
            // failing its probe. The walk-keeps-going test exercises
            // the rejection arm through this route.
            return Ok(FetchResponse {
                status: 200,
                body: r#"{"episodes":[{"id":9901,"number":1},{"id":9902,"number":2},{"id":9903,"number":3},{"id":9904,"number":4},{"id":9905,"number":5},{"id":9906,"number":6},{"id":9907,"number":7}]}"#
                    .into(),
            });
        }
        if url.contains("/api/frontend/anime/88/episodes") {
            // A continuation entry: the provider keeps the franchise's
            // cumulative numbering, so this two-episode cour lists 41
            // and 42 (the TYBW fourth-cour shape, captured live).
            return Ok(FetchResponse {
                status: 200,
                body: r#"{"episodes":[{"id":8841,"number":41},{"id":8842,"number":42}]}"#.into(),
            });
        }
        if url.contains("/api/frontend/episode/8841/languages") {
            return Ok(FetchResponse {
                status: 200,
                body: r#"{"languages":[{"code":"jpn","embed_url":"https://embed.example/e/s1"}]}"#
                    .into(),
            });
        }
        if url.contains("/api/frontend/episode/725/languages") {
            return Ok(FetchResponse {
                status: 200,
                body: r#"{"languages":[{"code":"jpn","embed_url":"https://embed.example/e/x"}]}"#
                    .into(),
            });
        }
        if url.contains("/api/frontend/episode/702/languages") {
            return Ok(FetchResponse {
                status: 200,
                body: r#"{"languages":[{"code":"jpn","embed_url":"https://embed.example/e/x"}]}"#
                    .into(),
            });
        }
        if url.contains("embed.example") {
            return Ok(FetchResponse {
                status: 200,
                body: "player.setup({ file: 'https://cdn.example/x/master.m3u8' });".into(),
            });
        }
        if url.ends_with("master.m3u8") {
            // The quality step validates the master on every path
            // now, best included — the stub serves it like the CDN.
            return Ok(FetchResponse {
                status: 200,
                body: "#EXTM3U\n".into(),
            });
        }
        Ok(FetchResponse {
            status: 404,
            body: String::new(),
        })
    }
}

pub(crate) fn the_show_browse() -> &'static str {
    // Leaked once per process; fine for tests.
    Box::leak(browse_page(&[("the-show-77", "The Show")]).into_boxed_str())
}

/// Borrowing adapter so one Provider serves a whole test.
pub(crate) struct ProviderRef<'a>(pub(crate) &'a Provider);

#[async_trait::async_trait]
impl AnidbFetch for ProviderRef<'_> {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        self.0.get(url).await
    }
}
