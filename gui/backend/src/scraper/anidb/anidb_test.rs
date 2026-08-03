use super::*;

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root")
        .join("tests/fixtures/anidb")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

// ── browse parsing ──────────────────────────────────────────────────

#[test]
fn parse_browse_extracts_slug_and_title_pairs_in_order() {
    let hits = parse_browse(&fixture("browse_one_piece.html"));
    assert_eq!(hits.len(), 3);
    assert_eq!(hits[0].slug, "one-piece-69");
    assert_eq!(hits[0].title, "One Piece");
    assert_eq!(hits[1].slug, "one-piece-film-red-9021");
    assert_eq!(hits[1].title, "One Piece Film: Red");
}

#[test]
fn parse_browse_decodes_html_entities_in_titles() {
    let hits = parse_browse(&fixture("browse_one_piece.html"));
    assert_eq!(hits[2].slug, "gintama-the-movie-4425");
    assert_eq!(hits[2].title, "Gintama': The Movie");
}

#[test]
fn parse_browse_reads_the_type_badge_when_the_card_carries_one() {
    // Live cards carry a `badge badge-*` span naming the entry's
    // format (TV / Movie / OVA...). The picker uses it to disprove
    // single-video candidates against multi-episode expectations,
    // so the parse must surface it — and a card without a badge
    // (the fixture's third entry) must read as unknown, never
    // excluded.
    let hits = parse_browse(&fixture("browse_one_piece.html"));
    assert_eq!(hits[0].kind.as_deref(), Some("TV"));
    assert_eq!(hits[1].kind.as_deref(), Some("Movie"));
    assert_eq!(hits[2].kind, None);
}

#[test]
fn parse_browse_yields_empty_on_a_result_less_page() {
    assert!(parse_browse(&fixture("browse_empty.html")).is_empty());
}

#[test]
fn cloudflare_interstitial_is_recognized_and_content_is_not() {
    assert!(is_cloudflare_interstitial(&fixture(
        "browse_cloudflare.html"
    )));
    assert!(!is_cloudflare_interstitial(&fixture(
        "browse_one_piece.html"
    )));
}

// ── slug + query plumbing ───────────────────────────────────────────

#[test]
fn numeric_id_is_the_slug_tail() {
    let hit = BrowseHit {
        kind: None,
        slug: "one-piece-69".into(),
        title: "One Piece".into(),
    };
    assert_eq!(hit.numeric_id(), Some(69));
    let no_tail = BrowseHit {
        kind: None,
        slug: "no-numeric-tail".into(),
        title: "x".into(),
    };
    assert_eq!(no_tail.numeric_id(), None);
}

#[test]
fn detail_year_is_the_season_links_year() {
    // The detail page names its premiere season as a browse link
    // ("Fall 1999" → /browse?season=fall&year=1999). That year is the
    // picker's identity signal for cour siblings whose episode counts
    // tie — the browse cards themselves carry no year at all.
    assert_eq!(
        parse_detail_year(&fixture("detail_one_piece.html")),
        Some(1999)
    );
    assert_eq!(parse_detail_year("<html>no season link</html>"), None);
}

#[test]
fn encode_query_form_urlencodes_the_title() {
    // The script's naive `sed 's| |+|g'` sends reserved characters
    // raw, and the provider 400s on them — "ChäoS;HEAd" was
    // unsearchable, and the 400 read as an upstream block that
    // stopped the whole walk. Full form-urlencoding keeps the
    // space→+ shape for plain titles and makes every title sendable.
    assert_eq!(encode_query("one piece"), "one+piece");
    assert_eq!(encode_query("Ch\u{e4}oS;HEAd"), "Ch%C3%A4oS%3BHEAd");
    assert_eq!(
        encode_query("D.Gray-man + \u{2606}"),
        "D.Gray-man+%2B+%E2%98%86"
    );
}

// ── episodes + languages + embed parsing ────────────────────────────

#[test]
fn parse_episodes_yields_id_number_pairs_in_order() {
    let eps = parse_episodes(&fixture("episodes_one_piece.json")).expect("parses");
    assert_eq!(
        eps,
        vec![
            EpisodeRef {
                id: 9001,
                number: 1
            },
            EpisodeRef {
                id: 9002,
                number: 2
            },
            EpisodeRef {
                id: 9003,
                number: 3
            },
        ]
    );
}

#[test]
fn parse_episodes_rejects_non_array_bodies() {
    assert!(parse_episodes("<html>rate limited</html>").is_err());
}

#[test]
fn parse_languages_yields_embeds_and_unescapes_urls() {
    let embeds = parse_languages(&fixture("languages_op.json")).expect("parses");
    assert_eq!(embeds.len(), 2);
    assert_eq!(embeds[0].language, "jpn");
    assert_eq!(embeds[0].embed_url, "https://embed.example/e/op-jpn");
}

#[test]
fn preferred_embed_maps_sub_to_jpn_and_dub_to_eng() {
    let embeds = parse_languages(&fixture("languages_op.json")).expect("parses");
    assert_eq!(
        preferred_embed(&embeds, "sub").map(|e| e.embed_url.as_str()),
        Some("https://embed.example/e/op-jpn")
    );
    assert_eq!(
        preferred_embed(&embeds, "dub").map(|e| e.embed_url.as_str()),
        Some("https://embed.example/e/op-eng")
    );
    assert!(preferred_embed(&[], "sub").is_none());
}

#[test]
fn extract_master_url_reads_the_jwplayer_file_assignment() {
    assert_eq!(
        extract_master_url(&fixture("embed_op.html")).as_deref(),
        Some("https://cdn.example/op/master.m3u8")
    );
    assert!(extract_master_url("<html>no player here</html>").is_none());
}

// ── transport resolution ────────────────────────────────────────────

#[cfg(unix)]
fn stage_exe(dir: &std::path::Path, name: &str) {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join(name);
    std::fs::write(&p, "#!/bin/sh\nexit 0\n").expect("write stub");
    let mut perms = std::fs::metadata(&p).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&p, perms).expect("chmod");
}

#[cfg(unix)]
#[test]
fn resolve_prefers_impersonate_names_over_plain_curl() {
    let dir = tempfile::tempdir().expect("tmp");
    stage_exe(dir.path(), "curl");
    stage_exe(dir.path(), "curl_chrome136");
    let path_env = dir.path().display().to_string();
    let fetch = CurlImpersonateFetch::resolve(None, &path_env).expect("resolved");
    assert!(fetch.exe().ends_with("curl_chrome136"));
}

#[cfg(unix)]
#[test]
fn resolve_falls_back_to_plain_curl_and_then_to_none() {
    let dir = tempfile::tempdir().expect("tmp");
    stage_exe(dir.path(), "curl");
    let path_env = dir.path().display().to_string();
    let fetch = CurlImpersonateFetch::resolve(None, &path_env).expect("resolved");
    assert!(fetch.exe().ends_with("curl"));

    let empty = tempfile::tempdir().expect("tmp");
    let none_path = empty.path().display().to_string();
    assert!(CurlImpersonateFetch::resolve(None, &none_path).is_none());
}

#[cfg(unix)]
#[test]
fn resolve_prefers_the_bundled_dir_over_path() {
    let bundled = tempfile::tempdir().expect("tmp");
    let on_path = tempfile::tempdir().expect("tmp");
    stage_exe(bundled.path(), "curl_firefox135");
    stage_exe(on_path.path(), "curl_firefox135");
    let path_env = on_path.path().display().to_string();
    let fetch = CurlImpersonateFetch::resolve(Some(bundled.path()), &path_env).expect("resolved");
    assert!(fetch.exe().starts_with(bundled.path()));
}

// ── the subprocess transport itself ─────────────────────────────────

#[cfg(unix)]
fn stage_curl_stub(dir: &std::path::Path, script: &str) -> CurlImpersonateFetch {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join("curl_firefox135");
    std::fs::write(&p, format!("#!/bin/sh\n{script}\n")).expect("write stub");
    let mut perms = std::fs::metadata(&p).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&p, perms).expect("chmod");
    CurlImpersonateFetch::resolve(Some(dir), "").expect("resolve stub")
}

#[cfg(unix)]
#[tokio::test]
async fn get_splits_the_status_trailer_from_the_body() {
    let dir = tempfile::tempdir().expect("tmp");
    let fetch = stage_curl_stub(dir.path(), "printf 'hello body\n200'");
    let resp = fetch.get("https://example.test/x").await.expect("get");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, "hello body");
}

#[cfg(unix)]
#[tokio::test]
async fn get_maps_curls_transfer_failure_marker_to_network() {
    // curl writes 000 as the status when the transfer itself failed.
    let dir = tempfile::tempdir().expect("tmp");
    let fetch = stage_curl_stub(dir.path(), "printf '\n000'");
    let err = fetch.get("https://example.test/x").await.expect_err("000");
    assert!(matches!(err, AniError::Network));
}

// ── client flow over a fixture-backed fetch ─────────────────────────

struct FixtureFetch;

#[async_trait::async_trait]
impl AnidbFetch for FixtureFetch {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        let body = if url.contains("browse?q=nohit") {
            fixture("browse_empty.html")
        } else if url.contains("browse?q=cloudflare") {
            fixture("browse_cloudflare.html")
        } else if url.contains("browse?q=") {
            fixture("browse_one_piece.html")
        } else if url.contains("/api/frontend/anime/69/episodes") {
            fixture("episodes_one_piece.json")
        } else if url.ends_with("/anime/one-piece-69") {
            fixture("detail_one_piece.html")
        } else if url.contains("/api/frontend/episode/9001/languages") {
            fixture("languages_op.json")
        } else if url.contains("embed.example") {
            fixture("embed_op.html")
        } else {
            return Ok(FetchResponse {
                status: 404,
                body: String::new(),
            });
        };
        let status = if body.contains("Just a moment") {
            403
        } else {
            200
        };
        Ok(FetchResponse { status, body })
    }
}

#[tokio::test]
async fn client_search_returns_hits_and_empty_for_no_results() {
    let client = AnidbClient::new(FixtureFetch);
    let hits = client.search("one piece").await.expect("hits");
    assert_eq!(hits.len(), 3);
    let none = client.search("nohit").await.expect("empty ok");
    assert!(none.is_empty());
}

#[tokio::test]
async fn client_search_surfaces_the_interstitial_as_upstream() {
    let client = AnidbClient::new(FixtureFetch);
    let err = client.search("cloudflare").await.expect_err("blocked");
    assert!(matches!(err, AniError::Upstream { status: 403 }));
}

#[tokio::test]
async fn client_episodes_keys_the_request_on_the_numeric_tail() {
    let client = AnidbClient::new(FixtureFetch);
    let eps = client.episodes("one-piece-69").await.expect("episodes");
    assert_eq!(eps.len(), 3);
    assert!(client.episodes("no-numeric-tail").await.is_err());
}

#[tokio::test]
async fn client_detail_year_reads_the_fixture_and_soft_misses() {
    let client = AnidbClient::new(FixtureFetch);
    assert_eq!(client.detail_year("one-piece-69").await, Some(1999));
    // A slug with no detail route 404s; the year is a hint, so the
    // miss is a soft None rather than an error.
    assert_eq!(client.detail_year("no-such-show-1").await, None);
}

#[tokio::test]
async fn client_resolves_the_master_playlist_for_sub_and_dies_clean_without_embed() {
    let client = AnidbClient::new(FixtureFetch);
    let url = client
        .master_playlist_url(9001, "sub")
        .await
        .expect("master url");
    assert_eq!(url, "https://cdn.example/op/master.m3u8");
    // Episode 9002 has no languages fixture route → 404 body → no embeds.
    assert!(client.master_playlist_url(9002, "sub").await.is_err());
}
