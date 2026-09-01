use super::*;

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/anidb")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

// ── browse parsing ──────────────────────────────────────────────────

#[test]
fn parse_browse_extracts_slug_and_title_pairs_in_order() {
    let hits = parse_browse(&fixture("browse_one_piece.html")).expect("parses");
    assert_eq!(hits.len(), 3);
    assert_eq!(hits[0].slug, "one-piece-69");
    assert_eq!(hits[0].title, "One Piece");
    assert_eq!(hits[1].slug, "one-piece-film-red-9021");
    assert_eq!(hits[1].title, "One Piece Film: Red");
}

#[test]
fn parse_browse_decodes_html_entities_in_titles() {
    let hits = parse_browse(&fixture("browse_one_piece.html")).expect("parses");
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
    let hits = parse_browse(&fixture("browse_one_piece.html")).expect("parses");
    assert_eq!(hits[0].kind.as_deref(), Some("TV"));
    assert_eq!(hits[1].kind.as_deref(), Some("Movie"));
    assert_eq!(hits[2].kind, None);
}

#[test]
fn parse_browse_yields_empty_on_a_result_less_page() {
    assert!(parse_browse(&fixture("browse_empty.html"))
        .expect("a genuine no-results page is absence, not failure")
        .is_empty());
}

#[test]
fn zero_hit_html_without_a_browse_marker_is_a_parse_failure() {
    // A 200 maintenance or WAF page that is not cloudflare's
    // interstitial parses to the same zero hits as the genuine empty
    // grid; if every alias answers that way the walk calls it a
    // clean miss, records breaker success, and writes a negative
    // availability row that hides the show for the TTL. Absence has
    // to come from a page that shows the browse shape — its results
    // grid or its no-results copy; zero-hit HTML with neither is a
    // parse failure, loud and transient, never cached.
    let maintenance = "<html><head><title>Maintenance</title></head>\
         <body><h1>Service temporarily unavailable</h1></body></html>";
    assert!(matches!(
        parse_browse(maintenance),
        Err(AniError::ParseFailed { .. })
    ));
}

#[test]
fn parse_browse_skips_anchors_that_are_not_result_cards() {
    // Real pages carry nav/footer anchors between the cards; each
    // rejection arm must drop its anchor without derailing the scan.
    let html = concat!(
        r##"<a href="/about">About</a>"##, // no anime/ href
        r#"<a href="https://anidb.app/anime/One-Piece-69">bad</a>"#, // uppercase slug
        r#"<a href="https://anidb.app/anime/one-piece">no id</a>"#, // no numeric tail
        r#"<a href="https://anidb.app/anime/one-piece-69">no alt image</a>"#,
        r#"<a href="https://anidb.app/anime/one-piece-69"><img alt="One Piece"></a>"#,
    );
    let hits = parse_browse(html).expect("parses");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].slug, "one-piece-69");
    assert_eq!(hits[0].title, "One Piece");
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

#[test]
fn parse_browse_reads_the_slug_from_the_href_alone() {
    // An anime/ path in nested markup — an image src here — is not
    // where the anchor points: the href names /news, and emitting the
    // image's slug fabricates a candidate the disambiguation can then
    // pick over the real show.
    let html = concat!(
        r#"<a href="https://anidb.app/news">"#,
        r#"<img alt="Foo" src="https://cdn.anidb.app/anime/one-piece-69"></a>"#,
        r#"<a href="https://anidb.app/anime/gintama-4425"><img alt="Gintama"></a>"#,
    );
    let hits = parse_browse(html).expect("parses");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].slug, "gintama-4425");
}

#[test]
fn parse_browse_reads_nothing_past_a_cards_closing_anchor() {
    // Markup between one card's </a> and the next anchor belongs to
    // no card: an anchor without its own alt must be skipped, not
    // titled by a stray image, and a stray badge is nobody's kind —
    // a wrong kind feeds the type-based disambiguation directly.
    let html = concat!(
        r#"<a href="https://anidb.app/anime/one-piece-69"></a>"#,
        r#"<img alt="Stray Title"><span class="badge badge-orange">Movie</span>"#,
        r#"<a href="https://anidb.app/anime/gintama-4425"><img alt="Gintama"></a>"#,
        r#"<span class="badge badge-orange">OVA</span>"#,
    );
    let hits = parse_browse(html).expect("parses");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].slug, "gintama-4425");
    assert_eq!(hits[0].title, "Gintama");
    assert_eq!(hits[0].kind, None);
}

#[test]
fn cloudflare_interstitial_matches_case_insensitively() {
    // The script greps -qi: challenge pages have varied the title's
    // capitalization, and a missed marker reads as an empty result
    // list instead of the typed upstream error.
    assert!(is_cloudflare_interstitial(
        "<title>JUST A MOMENT...</title>"
    ));
    assert!(is_cloudflare_interstitial("<title>just a moment</title>"));
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
fn slug_search_term_is_the_words_without_the_id_tail() {
    // The reverse-resolver searches Kitsu with this text when a
    // history row keys on a slug, so the derivation must reject
    // everything that isn't slug-shaped: legacy allanime ids
    // (hyphenless mixed-case) and word-only strings keep their own
    // resolve path, and a bare number carries no searchable words.
    assert_eq!(
        slug_search_term("one-piece-69").as_deref(),
        Some("one piece")
    );
    assert_eq!(
        slug_search_term("86-eighty-six-1234").as_deref(),
        Some("86 eighty six")
    );
    assert_eq!(slug_search_term("ReooPAxPMsHM4KPMY"), None);
    assert_eq!(slug_search_term("no-numeric-tail"), None);
    assert_eq!(slug_search_term("12345"), None);
    assert_eq!(slug_search_term("-69"), None);
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
fn parse_episodes_surfaces_the_decimal_tag() {
    // The provider lists recaps and specials under decimal tags in
    // number2 while number carries the integer slot. Dropping
    // number2 made every fractional episode unresolvable natively:
    // the strip advertises "1061.5", the click sends it, and the
    // resolver's integer parse rejects it as NoResults.
    let eps = parse_episodes(
        r#"{"episodes":[{"id":1,"number":3,"number2":null},{"id":2,"number":4,"number2":3.5}]}"#,
    )
    .expect("parses");
    assert_eq!(eps[0].number2, None);
    assert_eq!(eps[1].number2.as_deref(), Some("3.5"));
}

#[test]
fn parse_episodes_yields_id_number_pairs_in_order() {
    let eps = parse_episodes(&fixture("episodes_one_piece.json")).expect("parses");
    assert_eq!(
        eps,
        vec![
            EpisodeRef {
                id: 9001,
                number: 1,
                number2: None,
            },
            EpisodeRef {
                id: 9002,
                number: 2,
                number2: None,
            },
            EpisodeRef {
                id: 9003,
                number: 3,
                number2: None,
            },
        ]
    );
}

#[test]
fn parse_episodes_rejects_non_array_bodies() {
    assert!(parse_episodes("<html>rate limited</html>").is_err());
}

#[test]
fn parse_languages_rejects_non_array_bodies() {
    assert!(parse_languages("<html>rate limited</html>").is_err());
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
    // A player stanza with an empty source is a miss, not Some("").
    assert!(extract_master_url("file: ''").is_none());
}

#[test]
fn a_malformed_file_value_is_no_master_url() {
    // A nonempty but malformed file value rode out of here as a
    // "resolved" master URL: the orchestrator records breaker
    // success, stamps availability, and writes the episode to
    // history before its own Url::parse fails — the user sees a
    // playback error on a show now marked available and watched.
    // Only an absolute http(s) URL is something every consumer can
    // actually use; anything else is the same miss as an empty one.
    assert!(extract_master_url("file: 'not a url'").is_none());
    assert!(extract_master_url("file: '/relative/master.m3u8'").is_none());
    assert!(extract_master_url("file: 'javascript:alert(1)'").is_none());
    assert_eq!(
        extract_master_url("file: 'https://cdn.example/op/master.m3u8'").as_deref(),
        Some("https://cdn.example/op/master.m3u8")
    );
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

#[cfg(windows)]
#[test]
fn resolve_finds_the_exe_suffixed_curl_on_windows() {
    // The system curl ships as curl.exe; a resolve that only checks
    // the bare failover names never finds it and the transport dies
    // with Network on every Windows machine.
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(dir.path().join("curl.exe"), "MZ").expect("write stub");
    let path_env = dir.path().display().to_string();
    let fetch = CurlImpersonateFetch::resolve(None, &path_env).expect("resolved");
    assert!(fetch.exe().ends_with("curl.exe"));
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

#[test]
fn candidate_names_expand_the_platform_suffixes_bare_name_first() {
    assert_eq!(
        fetch::candidate_names("curl_chrome136", &["", ".exe"]),
        vec![
            "curl_chrome136".to_string(),
            "curl_chrome136.exe".to_string()
        ]
    );
    assert_eq!(
        fetch::candidate_names("curl", &[""]),
        vec!["curl".to_string()]
    );
}

#[cfg(unix)]
#[test]
fn resolve_finds_the_suffixed_binaries_windows_ships() {
    // The Windows arm of the suffix table, driven explicitly so the
    // behavior is provable on every platform: only `.exe`-shaped
    // files exist, and resolution still names one.
    let dir = tempfile::tempdir().expect("tmp");
    stage_exe(dir.path(), "curl_chrome136.exe");
    let path_env = dir.path().display().to_string();
    let fetch = CurlImpersonateFetch::resolve_with_suffixes(None, &path_env, &["", ".exe"])
        .expect("resolved");
    assert!(fetch.exe().ends_with("curl_chrome136.exe"));
}

#[cfg(unix)]
#[test]
fn resolve_keeps_the_failover_order_above_any_suffix_match() {
    // Plain `curl` exists bare while a better impersonate name exists
    // only suffixed — the failover order still decides.
    let dir = tempfile::tempdir().expect("tmp");
    stage_exe(dir.path(), "curl");
    stage_exe(dir.path(), "curl_firefox135.exe");
    let path_env = dir.path().display().to_string();
    let fetch = CurlImpersonateFetch::resolve_with_suffixes(None, &path_env, &["", ".exe"])
        .expect("resolved");
    assert!(fetch.exe().ends_with("curl_firefox135.exe"));
}

#[cfg(unix)]
#[test]
fn resolve_exhausts_the_bundled_dir_before_path() {
    // The bundled directory is the packaged, known-compatible
    // transport: ANY bundled failover name outranks every PATH
    // binary, or a system install silently bypasses the transport
    // the package validated and shipped.
    let bundled = tempfile::tempdir().expect("tmp");
    let on_path = tempfile::tempdir().expect("tmp");
    stage_exe(bundled.path(), "curl_chrome136");
    stage_exe(on_path.path(), "curl_firefox135");
    let path_env = on_path.path().display().to_string();
    let fetch = CurlImpersonateFetch::resolve(Some(bundled.path()), &path_env).expect("resolved");
    assert!(fetch.exe().starts_with(bundled.path()));
    assert!(fetch.exe().ends_with("curl_chrome136"));
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
        } else if url.ends_with("/anime/cloudflare-1") {
            // A detail page behind the interstitial — the refusal
            // shape, distinct from the 404 soft miss below.
            fixture("browse_cloudflare.html")
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
    assert_eq!(
        client.detail_year("one-piece-69").await.expect("fetched"),
        Some(1999)
    );
    // A slug with no detail route 404s: the page is genuinely
    // missing, so the year stays a soft None rather than an error.
    assert_eq!(
        client.detail_year("no-such-show-1").await.expect("soft"),
        None
    );
}

#[tokio::test]
async fn client_detail_year_propagates_refusals() {
    // A cloudflare interstitial on a detail page is the provider
    // refusing this client, not a page without a year: reading it as
    // an unknown year lets the picker keep requesting the rest of
    // the pool's detail pages and select year-blind through a block.
    let client = AnidbClient::new(FixtureFetch);
    let err = client
        .detail_year("cloudflare-1")
        .await
        .expect_err("refused");
    assert!(matches!(err, AniError::Upstream { status: 403 }));
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

// ── fixture manifest ────────────────────────────────────────────────

/// The manifest pins every fixture byte-for-byte, in both directions:
/// each listed digest matches its file, and each file is listed. A
/// fixture edited without its digest is invisible in review — the
/// diff shows new response shapes while the manifest still vouches
/// for the old ones.
#[test]
fn fixture_manifest_matches_the_fixtures() {
    use sha2::Digest as _;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/anidb");
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("MANIFEST.json")).expect("read manifest"),
    )
    .expect("parse manifest");
    let entries = manifest.as_object().expect("manifest is an object");
    let mut listed = std::collections::BTreeSet::new();
    for (name, entry) in entries {
        listed.insert(name.clone());
        let want = entry["sha256"].as_str().expect("sha256 entry");
        let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"));
        let have = format!("{:x}", sha2::Sha256::digest(&bytes));
        assert_eq!(
            have, *want,
            "{name}: fixture bytes do not match the manifest digest"
        );
    }
    for file in std::fs::read_dir(&dir).expect("list fixtures") {
        let file = file.expect("dir entry").file_name();
        let file = file.to_string_lossy();
        if file == "MANIFEST.json" {
            continue;
        }
        assert!(
            listed.contains(file.as_ref()),
            "{file} is not in MANIFEST.json"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_hung_transport_child_dies_with_the_dropped_deadline() {
    // kill_on_drop is the difference between a timeout and a leak:
    // the dropped output() future must take the child with it, or
    // every timed-out request parks a curl for its full hang.
    let dir = tempfile::tempdir().expect("tmp");
    let pidfile = dir.path().join("child.pid");
    let fetch = stage_curl_stub(
        dir.path(),
        &format!("echo $$ >'{}'\nsleep 600", pidfile.display()),
    )
    .with_deadline(std::time::Duration::from_millis(300));
    let err = fetch
        .get("https://example.test/x")
        .await
        .expect_err("deadline");
    assert!(matches!(err, AniError::Timeout));
    let pid = std::fs::read_to_string(&pidfile)
        .expect("pidfile")
        .trim()
        .to_string();
    // The kill lands at drop; give the reap a moment, then require
    // the pid gone.
    let mut alive = true;
    for _ in 0..50 {
        alive = std::process::Command::new("kill")
            .args(["-0", &pid])
            .status()
            .expect("kill -0")
            .success();
        if !alive {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(
        !alive,
        "the timed-out curl child survived its dropped future"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn the_transport_child_runs_under_the_mandated_environment() {
    // §5's subprocess rule: TERM=dumb and NO_COLOR=1, so no wrapper
    // or future curl variant can shape its output by the launch
    // environment. The stub reports what it actually received.
    let dir = tempfile::tempdir().expect("tmp");
    let fetch = stage_curl_stub(dir.path(), "printf '%s %s\\n200' \"$TERM\" \"$NO_COLOR\"");
    let resp = fetch.get("https://example.test/x").await.expect("get");
    assert_eq!(resp.body, "dumb 1");
}

#[cfg(unix)]
#[tokio::test]
async fn a_failed_transfer_with_a_parsed_trailer_is_refused() {
    // curl's -w trailer reports the last HTTP status even when the
    // transfer itself then fails — exit 28 is the operation-timeout
    // arm — so a truncated body arrives with a plausible 200 trailer.
    // The exit status is the only signal separating that from a
    // complete response.
    let dir = tempfile::tempdir().expect("tmp");
    let fetch = stage_curl_stub(dir.path(), "printf 'partial body\n200'\nexit 28");
    let err = fetch.get("https://example.test/x").await.expect_err("28");
    assert!(matches!(err, AniError::Network));
}

#[cfg(all(unix, target_os = "macos"))]
#[tokio::test]
async fn the_transport_pins_darwins_cipher_suites() {
    // The provider's TLS fingerprinting reads the cipher list as much
    // as the user agent; the script pins both suites on Darwin
    // (ani-cli's cipher_flag) because macOS curl builds negotiate
    // defaults the provider rejects. The stub reports the arguments
    // it was launched with.
    let dir = tempfile::tempdir().expect("tmp");
    let fetch = stage_curl_stub(dir.path(), "printf '%s\\n' \"$@\"\nprintf '\\n200'");
    let resp = fetch.get("https://example.test/x").await.expect("get");
    assert!(resp.body.contains("--ciphers"));
    assert!(resp
        .body
        .contains("ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256"));
    assert!(resp.body.contains("--tls13-ciphers"));
    assert!(resp
        .body
        .contains("TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256"));
}

#[cfg(all(unix, not(target_os = "macos")))]
#[tokio::test]
async fn the_transport_leaves_ciphers_to_curl_off_darwin() {
    // Everywhere else the impersonate build's own defaults ARE the
    // fingerprint — the script only pins ciphers inside its Darwin
    // case, and adding them elsewhere would change the fingerprint
    // the impersonation exists to present.
    let dir = tempfile::tempdir().expect("tmp");
    let fetch = stage_curl_stub(dir.path(), "printf '%s\\n' \"$@\"\nprintf '\\n200'");
    let resp = fetch.get("https://example.test/x").await.expect("get");
    assert!(!resp.body.contains("--ciphers"));
}

#[cfg(unix)]
#[tokio::test]
async fn the_transport_outlives_a_briefly_busy_executable() {
    // The suite stages executable stubs from many threads, and a
    // fork elsewhere in the process can still hold a stub's write fd
    // when this transport execs it — the kernel answers ETXTBSY and
    // the whole run flakes on transient weather. Holding the file
    // open for writing reproduces that race deterministically: the
    // spawn must wait out the writer instead of reporting Network.
    let dir = tempfile::tempdir().expect("tmp");
    let fetch = stage_curl_stub(dir.path(), "printf '\\n200'");
    let writer = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.path().join("curl_firefox135"))
        .expect("hold the stub open for writing");
    let handle = tokio::spawn(async move { fetch.get("https://example.test/x").await });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    drop(writer);
    let resp = handle
        .await
        .expect("join")
        .expect("get retries past ETXTBSY");
    assert_eq!(resp.status, 200);
}

#[cfg(unix)]
#[tokio::test]
async fn the_transport_disables_curlrc_first() {
    // A user's ~/.curlrc can redirect output or append transfers,
    // corrupting the body this code parses. curl only honors
    // -q/--disable as the FIRST argument, so that position is the
    // contract.
    let dir = tempfile::tempdir().expect("tmp");
    let fetch = stage_curl_stub(dir.path(), "printf '%s\\n' \"$1\"\nprintf '\\n200'");
    let resp = fetch.get("https://example.test/x").await.expect("get");
    assert_eq!(resp.body.lines().next(), Some("-q"));
}

// ── master-playlist quality selection ───────────────────────────────

#[test]
fn master_variants_parse_sorted_by_height() {
    let variants = parse_master_variants(&fixture("master_op.m3u8"));
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].height, 1080);
    assert_eq!(variants[0].url, "https://cdn.example/op/1080/index.m3u8");
    assert_eq!(variants[1].height, 720);
    assert_eq!(variants[1].url, "https://cdn.example/op/720/index.m3u8");
}

#[test]
fn variant_selection_mirrors_the_scripts_quality_arms() {
    let variants = parse_master_variants(&fixture("master_op.m3u8"));
    assert_eq!(
        select_variant(&variants, "best").map(|v| v.height),
        Some(1080)
    );
    assert_eq!(
        select_variant(&variants, "worst").map(|v| v.height),
        Some(720)
    );
    assert_eq!(
        select_variant(&variants, "720").map(|v| v.height),
        Some(720)
    );
    // A height nobody serves is a miss, not a guess — the caller
    // falls back to the adaptive master.
    assert!(select_variant(&variants, "480").is_none());
    assert!(select_variant(&[], "best").is_none());
}

/// A fetch serving only the master playlist, counting its fetches.
struct MasterOnly {
    fetches: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait::async_trait]
impl AnidbFetch for MasterOnly {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        if url == "https://cdn.example/op/master.m3u8" {
            self.fetches
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return Ok(FetchResponse {
                status: 200,
                body: fixture("master_op.m3u8"),
            });
        }
        if url == "https://cdn.example/op/720/index.m3u8" {
            self.fetches
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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

#[tokio::test]
async fn quality_selection_returns_the_matching_variant() {
    let fetches = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let client = AnidbClient::new(MasterOnly {
        fetches: fetches.clone(),
    });
    let url = client
        .quality_stream_url("https://cdn.example/op/master.m3u8", "720")
        .await
        .expect("served master");
    assert_eq!(url, "https://cdn.example/op/720/index.m3u8");
    // Master and the selected rendition each validated once: a
    // rendition returned unfetched can be dead while the master is
    // healthy, and the resolver would record success and cache a
    // session the proxy cannot load.
    assert_eq!(fetches.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_dead_rendition_falls_back_to_the_served_master() {
    // 1080 is listed in the master but the stub serves only the 720
    // rendition: the selected 1080 URL 404s. The master itself was
    // served, so playback stays adaptive on the master instead of
    // reporting success with a dead rendition — or failing a play
    // the adaptive stream could carry.
    let fetches = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let client = AnidbClient::new(MasterOnly {
        fetches: fetches.clone(),
    });
    let url = client
        .quality_stream_url("https://cdn.example/op/master.m3u8", "1080")
        .await
        .expect("the served master carries the play");
    assert_eq!(url, "https://cdn.example/op/master.m3u8");
}

/// A healthy master whose selected rendition the provider refuses:
/// the block must keep its identity instead of dissolving into the
/// adaptive fallback.
struct BlockedRendition;

#[async_trait::async_trait]
impl AnidbFetch for BlockedRendition {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        if url == "https://cdn.example/op/master.m3u8" {
            return Ok(FetchResponse {
                status: 200,
                body: fixture("master_op.m3u8"),
            });
        }
        Ok(FetchResponse {
            status: 429,
            body: String::new(),
        })
    }
}

#[tokio::test]
async fn a_blocked_rendition_propagates_instead_of_masking() {
    // The soft fallback exists for an answered miss — a rendition
    // the CDN says isn't there. A 429 (or any refusal) on the
    // rendition is the upstream blocking this client, and falling
    // back to the master would record breaker success and stamp
    // availability, history, and a cached session while hls.js is
    // about to request renditions through the same blocked front.
    let client = AnidbClient::new(BlockedRendition);
    let err = client
        .quality_stream_url("https://cdn.example/op/master.m3u8", "720")
        .await
        .expect_err("a blocked rendition must keep its identity");
    assert!(
        matches!(err, crate::error::AniError::Upstream { status: 429 }),
        "expected the refusal verbatim, got {err:?}"
    );
}

/// Answers every URL with HTTP 200 and an HTML error page — the
/// shape a CDN serves when the playlist path is wrong or expired.
struct HtmlAnswers;

#[async_trait::async_trait]
impl AnidbFetch for HtmlAnswers {
    async fn get(&self, _url: &str) -> crate::error::Result<FetchResponse> {
        Ok(FetchResponse {
            status: 200,
            body: "<html><body>Not here.</body></html>".into(),
        })
    }
}

proptest::proptest! {
    /// The predicate accepts exactly the bodies whose trimmed prefix
    /// is the HLS marker: any leading whitespace is tolerated, and a
    /// body built NOT to open with the marker is refused whatever it
    /// contains further in.
    #[test]
    fn hls_predicate_accepts_exactly_marker_prefixed_bodies(
        ws in "[ \t\r\n]{0,8}",
        rest in "[a-zA-Z0-9 #:=,\n-]{0,64}",
        junk in "[a-zA-Z0-9<][a-zA-Z0-9 #:=,\n-]{0,64}",
    ) {
        let playlist = format!("{ws}#EXTM3U{rest}");
        let page = format!("{ws}{junk}");
        proptest::prop_assert!(super::quality::is_hls_playlist(&playlist));
        proptest::prop_assert!(!super::quality::is_hls_playlist(&page));
        proptest::prop_assert!(!super::quality::is_hls_playlist(&ws));
    }
}

#[tokio::test]
async fn a_masters_html_answer_is_not_a_playlist() {
    // 200 with an HTML body passes the status check and the
    // interstitial check, so best-quality reported success for a
    // stream hls.js cannot load — recording breaker health and
    // stamping availability, history, and a cached session. A
    // master must actually be an HLS playlist.
    let client = AnidbClient::new(HtmlAnswers);
    let err = client
        .quality_stream_url("https://cdn.example/op/master.m3u8", "best")
        .await
        .expect_err("an HTML page is not a playlist");
    assert!(
        matches!(err, crate::error::AniError::ParseFailed { .. }),
        "expected the parse verdict, got {err:?}"
    );
}

/// A served, valid master whose selected rendition answers 200 with
/// HTML instead of a playlist.
struct HtmlRendition;

#[async_trait::async_trait]
impl AnidbFetch for HtmlRendition {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        if url == "https://cdn.example/op/master.m3u8" {
            return Ok(FetchResponse {
                status: 200,
                body: fixture("master_op.m3u8"),
            });
        }
        Ok(FetchResponse {
            status: 200,
            body: "<html><body>Not here.</body></html>".into(),
        })
    }
}

#[tokio::test]
async fn a_renditions_html_answer_falls_back_to_the_served_master() {
    // The rendition answered, but with a page, not a playlist — an
    // answered miss like a 404: the validated adaptive master
    // carries the play.
    let client = AnidbClient::new(HtmlRendition);
    let url = client
        .quality_stream_url("https://cdn.example/op/master.m3u8", "720")
        .await
        .expect("the served master carries the play");
    assert_eq!(url, "https://cdn.example/op/master.m3u8");
}

#[tokio::test]
async fn best_quality_keeps_the_adaptive_master_it_validated() {
    // The default path returned the extracted URL without ever
    // requesting it: a dead master still recorded breaker success,
    // stamped availability and history, and cached a session that
    // fails the moment the proxy loads it — the non-best qualities
    // were fixed, the DEFAULT was not. best now fetches the master
    // once like every other quality and keeps the adaptive URL only
    // when the playlist was actually served. This flips the old
    // no-fetch pin, which asserted the gap itself.
    let fetches = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let client = AnidbClient::new(MasterOnly {
        fetches: fetches.clone(),
    });
    let url = client
        .quality_stream_url("https://cdn.example/op/master.m3u8", "best")
        .await
        .expect("served master");
    assert_eq!(url, "https://cdn.example/op/master.m3u8");
    assert_eq!(fetches.load(std::sync::atomic::Ordering::SeqCst), 1);
    let err = client
        .quality_stream_url("https://cdn.example/op/missing.m3u8", "best")
        .await
        .expect_err("a dead master cannot report success");
    assert!(matches!(err, AniError::Upstream { status: 404 }));
}

#[tokio::test]
async fn an_unserved_quality_falls_back_to_the_fetched_master() {
    let fetches = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let client = AnidbClient::new(MasterOnly {
        fetches: fetches.clone(),
    });
    // Served master, unserved height → adaptive master, not a guess.
    let url = client
        .quality_stream_url("https://cdn.example/op/master.m3u8", "480")
        .await
        .expect("the playlist itself was served");
    assert_eq!(url, "https://cdn.example/op/master.m3u8");
}

#[tokio::test]
async fn a_failed_master_fetch_propagates_instead_of_reporting_success() {
    // The soft fallback exists for a SERVED playlist that lacks the
    // requested height. Extending it to a failed fetch returns the
    // URL that just failed: the resolver then reports success,
    // stamps availability and history, and caches a session the
    // player cannot load — and a swallowed 429 records breaker
    // health instead of opening the rate-limit pause.
    let fetches = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let client = AnidbClient::new(MasterOnly {
        fetches: fetches.clone(),
    });
    let err = client
        .quality_stream_url("https://cdn.example/op/missing.m3u8", "720")
        .await
        .expect_err("the playlist fetch itself failed");
    assert!(
        matches!(err, AniError::Upstream { status: 404 }),
        "got {err:?}"
    );
}

// ── the bare impersonate build and its target ───────────────────────
//
// The per-browser entries upstream ships are wrapper scripts that
// encode a fingerprint in their own flags, and on Windows they are
// `.bat` files the resolver deliberately will not name. The patched
// binary itself takes `--impersonate <target>`, which is the path
// that works on every platform we package.

#[test]
fn the_failover_list_pairs_the_bare_build_with_an_impersonation_target() {
    let bare = CURL_FAILOVER
        .iter()
        .find(|c| c.name == "curl-impersonate")
        .expect("the bare impersonate build must be a transport candidate");
    assert_eq!(
        bare.impersonate,
        Some("chrome136"),
        "the bare binary carries no fingerprint of its own — it needs the target passed"
    );
}

#[test]
fn the_bare_build_yields_to_the_wrappers_and_outranks_plain_curl() {
    let at = |n: &str| {
        CURL_FAILOVER
            .iter()
            .position(|c| c.name == n)
            .unwrap_or_else(|| panic!("{n} missing from the failover list"))
    };
    // A wrapper is preferred where one exists: it is what the Linux
    // packages stage and what the script itself reaches for, so this
    // ordering keeps the working platform's behavior unchanged.
    assert!(at("curl_firefox135") < at("curl-impersonate"));
    // But an impersonating build of any shape beats plain curl, which
    // the provider answers with its interstitial.
    assert!(at("curl-impersonate") < at("curl"));
}

#[test]
fn every_wrapper_and_plain_curl_carry_no_target() {
    for name in [
        "curl_firefox135",
        "curl_chrome136",
        "curl_chrome116",
        "curl_ff117",
        "curl",
    ] {
        let c = CURL_FAILOVER
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("{name} missing from the failover list"));
        assert_eq!(
            c.impersonate, None,
            "{name} encodes its fingerprint itself, or has none to encode"
        );
    }
}

#[test]
fn a_candidate_with_a_target_passes_it_on_the_command_line() {
    let args = fetch::fetch_args("https://anidb.app/anime/x", Some("chrome136"));
    let i = args
        .iter()
        .position(|a| a == "--impersonate")
        .expect("the target must reach the child as a flag");
    assert_eq!(args[i + 1], "chrome136");
}

#[test]
fn a_candidate_without_a_target_gets_no_impersonate_flag() {
    let args = fetch::fetch_args("https://anidb.app/anime/x", None);
    assert!(
        !args.iter().any(|a| a == "--impersonate"),
        "a wrapper already carries its fingerprint; passing a target too would fight it"
    );
}

/// The Windows impersonate builds link BoringSSL, which carries no
/// default CA bundle path on Windows and does not consult the system
/// certificate store on its own — every TLS verify fails (curl exit
/// 60) and each fetch surfaces as Network. `--ca-native` points the
/// child at the Windows store.
#[cfg(windows)]
#[test]
fn the_windows_child_reads_the_native_certificate_store() {
    for target in [Some("chrome136"), None] {
        let args = fetch::fetch_args("https://anidb.app/anime/x", target);
        assert!(
            args.iter().any(|a| a == "--ca-native"),
            "without the flag a BoringSSL child on Windows fails every TLS verify"
        );
    }
}

/// Elsewhere the builds find the platform's CA store by their own
/// defaults, and the Linux packages' wrapper scripts predate the
/// flag — passing it would be at best redundant and at worst an
/// unknown-option failure.
#[cfg(not(windows))]
#[test]
fn other_platforms_keep_the_builds_own_verification_defaults() {
    for target in [Some("chrome136"), None] {
        let args = fetch::fetch_args("https://anidb.app/anime/x", target);
        assert!(
            !args.iter().any(|a| a == "--ca-native"),
            "the flag is a Windows accommodation, not a default"
        );
    }
}

/// An impersonating build advertises the browser's Accept-Encoding
/// (gzip, br, zstd) as part of the fingerprint, so the provider
/// answers compressed. Without `--compressed` curl hands the raw
/// bytes through and the page parser sees a zstd frame instead of
/// HTML — "zero hits in an unrecognized page shape" on every search.
/// The upstream wrapper scripts pass the flag themselves; the bare
/// build the Windows package stages does not, so the argv must.
#[test]
fn the_child_decodes_the_content_encoding_it_advertises() {
    for target in [Some("chrome136"), None] {
        let args = fetch::fetch_args("https://anidb.app/anime/x", target);
        assert!(
            args.iter().any(|a| a == "--compressed"),
            "an advertised encoding the child does not decode is a parse failure downstream"
        );
    }
}

#[test]
fn the_url_stays_last_whether_or_not_a_target_is_passed() {
    for target in [Some("chrome136"), None] {
        let args = fetch::fetch_args("https://anidb.app/anime/x", target);
        assert_eq!(
            args.last().map(String::as_str),
            Some("https://anidb.app/anime/x")
        );
    }
}

#[cfg(unix)]
#[test]
fn resolve_carries_the_matched_candidates_target() {
    // Windows stages only the bare binary, so resolution must land on
    // it and remember what to impersonate. Driven through the Windows
    // suffix table so the behavior is provable from any platform.
    let dir = tempfile::tempdir().expect("tmp");
    stage_exe(dir.path(), "curl-impersonate.exe");
    let path_env = dir.path().display().to_string();
    let fetch = CurlImpersonateFetch::resolve_with_suffixes(None, &path_env, &["", ".exe"])
        .expect("resolved");
    assert!(fetch.exe().ends_with("curl-impersonate.exe"));
    assert_eq!(fetch.impersonate(), Some("chrome136"));
}

#[cfg(unix)]
#[test]
fn resolve_carries_no_target_for_a_wrapper() {
    let dir = tempfile::tempdir().expect("tmp");
    stage_exe(dir.path(), "curl_firefox135");
    let path_env = dir.path().display().to_string();
    let fetch = CurlImpersonateFetch::resolve(None, &path_env).expect("resolved");
    assert_eq!(fetch.impersonate(), None);
}
