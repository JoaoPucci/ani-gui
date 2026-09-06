//! The transport's own tests: binary resolution and failover
//! order, the child's argv, the subprocess itself, and log redaction.
//! Moved here with the transport; the anidb client's tests stay with
//! that client.

use super::*;

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
        candidate_names("curl_chrome136", &["", ".exe"]),
        vec![
            "curl_chrome136".to_string(),
            "curl_chrome136.exe".to_string()
        ]
    );
    assert_eq!(candidate_names("curl", &[""]), vec!["curl".to_string()]);
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
    let args = fetch_args("https://anidb.app/anime/x", Some("chrome136"));
    let i = args
        .iter()
        .position(|a| a == "--impersonate")
        .expect("the target must reach the child as a flag");
    assert_eq!(args[i + 1], "chrome136");
}

#[test]
fn a_candidate_without_a_target_gets_no_impersonate_flag() {
    let args = fetch_args("https://anidb.app/anime/x", None);
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
        let args = fetch_args("https://anidb.app/anime/x", target);
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
        let args = fetch_args("https://anidb.app/anime/x", target);
        assert!(
            !args.iter().any(|a| a == "--ca-native"),
            "the flag is a Windows accommodation, not a default"
        );
    }
}

/// The signed stream URLs the transport also fetches carry their
/// credential in the path (`/stream/<token>/…`) and sometimes in the
/// query; a failure log that prints them verbatim hands out a usable
/// stream link to anyone the log is shared with. The log line keeps
/// scheme, host and the path's short segments — enough to name the
/// failing endpoint — and elides everything credential-shaped.
#[test]
fn transport_logs_elide_credential_shaped_url_parts() {
    let signed = redacted_url(
        "https://hls.anidb.app/stream/lCu4egwqaEzuaBJYHn948CSqKItUfl9TaL1iuFw7i4ISHYbYkTw4rwtFlxQ7axN8/master.m3u8",
    );
    assert!(
        !signed.contains("lCu4egwqaEzu"),
        "a path segment that long is a token, not a name: {signed}"
    );
    assert!(
        signed.contains("hls.anidb.app") && signed.contains("master.m3u8"),
        "host and the failing endpoint must survive redaction: {signed}"
    );

    let with_query = redacted_url("https://anidb.app/browse?q=some+title");
    assert!(
        !with_query.contains("some") && !with_query.contains('?'),
        "queries carry signatures and search text; they never reach the log: {with_query}"
    );
    assert!(
        with_query.contains("/browse"),
        "the endpoint survives: {with_query}"
    );

    let ordinary = redacted_url("https://anidb.app/api/frontend/anime/1234/episodes");
    assert_eq!(
        ordinary, "https://anidb.app/api/frontend/anime/1234/episodes",
        "short-segment paths carry no credential and stay legible"
    );

    assert_eq!(
        redacted_url("not a url"),
        "<unparseable url>",
        "an unparseable input must not pass through verbatim"
    );
}

/// curl's own error lines can echo the failing operand — a glob
/// parse error prints the whole URL, signed query included — so the
/// captured stderr is the same leak the url field just closed.
/// Globbing is disabled (the client never builds `{}`/`[]`
/// sequences), and any URL echo that still reaches stderr is
/// replaced with its redaction before logging.
#[test]
fn the_child_never_globs_and_its_stderr_echo_is_redacted() {
    for target in [Some("chrome136"), None] {
        let args = fetch_args("https://anidb.app/anime/x", target);
        assert!(
            args.iter().any(|a| a == "-g"),
            "an unmatched bracket in a URL makes curl print the whole operand to stderr"
        );
    }
    let url = "https://hls.anidb.app/stream/lCu4egwqaEzuaBJYHn948CSqKItUfl9TaL1iuFw7i4ISHYbYkTw4rwtFlxQ7axN8/master.m3u8";
    let scrubbed = scrub_stderr(&format!("curl: (3) URL rejected: {url}"), url);
    assert!(
        !scrubbed.contains("lCu4egwqaEzu"),
        "the echoed operand must leave scrubbed stderr: {scrubbed}"
    );
    assert!(
        scrubbed.contains("URL rejected"),
        "curl's own diagnosis must survive the scrub: {scrubbed}"
    );
    let unrelated = scrub_stderr("curl: (6) Could not resolve host", url);
    assert_eq!(
        unrelated, "curl: (6) Could not resolve host",
        "stderr without the operand passes through untouched"
    );
}

/// `-s` alone suppresses curl's own error text along with the
/// progress meter, so the transport's failure log would capture an
/// empty stderr exactly when it matters. `-S` restores the error
/// line while keeping silent mode.
#[test]
fn the_child_reports_its_error_text_despite_silent_mode() {
    for target in [Some("chrome136"), None] {
        let args = fetch_args("https://anidb.app/anime/x", target);
        assert!(
            args.iter().any(|a| a == "-sSL"),
            "silent mode without show-error logs an empty stderr on every transfer failure"
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
        let args = fetch_args("https://anidb.app/anime/x", target);
        assert!(
            args.iter().any(|a| a == "--compressed"),
            "an advertised encoding the child does not decode is a parse failure downstream"
        );
    }
}

#[test]
fn the_url_stays_last_whether_or_not_a_target_is_passed() {
    for target in [Some("chrome136"), None] {
        let args = fetch_args("https://anidb.app/anime/x", target);
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
