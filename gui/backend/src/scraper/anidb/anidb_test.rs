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
    let hits = parse_browse(html);
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
    let hits = parse_browse(html);
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
    let hits = parse_browse(html);
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
        .and_then(std::path::Path::parent)
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
        .await;
    assert_eq!(url, "https://cdn.example/op/720/index.m3u8");
    assert_eq!(fetches.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn best_quality_keeps_the_adaptive_master_without_a_fetch() {
    let fetches = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let client = AnidbClient::new(MasterOnly {
        fetches: fetches.clone(),
    });
    let url = client
        .quality_stream_url("https://cdn.example/op/master.m3u8", "best")
        .await;
    assert_eq!(url, "https://cdn.example/op/master.m3u8");
    assert_eq!(fetches.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_unserved_or_unfetchable_quality_falls_back_to_the_master() {
    let fetches = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let client = AnidbClient::new(MasterOnly {
        fetches: fetches.clone(),
    });
    // Served master, unserved height → adaptive master, not a guess.
    let url = client
        .quality_stream_url("https://cdn.example/op/master.m3u8", "480")
        .await;
    assert_eq!(url, "https://cdn.example/op/master.m3u8");
    // Unfetchable master → the master URL still plays adaptively.
    let url = client
        .quality_stream_url("https://cdn.example/op/missing.m3u8", "720")
        .await;
    assert_eq!(url, "https://cdn.example/op/missing.m3u8");
}
