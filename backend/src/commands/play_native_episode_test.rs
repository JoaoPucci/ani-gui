use super::*;
use crate::scraper::fetch::{FetchRequest, FetchResponse};
use crate::scraper::provider::{BrowseHit, EpisodeRef};

/// Streams exactly one episode id in sub; every other id is not
/// found. Resolving proves which row the matcher chased.
fn only_episode(id: u64) -> crate::scraper::table_provider::TableProvider {
    crate::scraper::table_provider::TableProvider::new().stream(
        id,
        "sub",
        "https://cdn.example/x/master.m3u8",
    )
}

fn ep(id: u64, number: u32, tag: Option<&str>) -> EpisodeRef {
    EpisodeRef {
        id,
        number,
        number2: tag.map(str::to_string),
    }
}

fn show(episodes: Vec<EpisodeRef>) -> PickedShow {
    PickedShow {
        hit: BrowseHit {
            slug: "the-show-77".into(),
            title: "The Show".into(),
            kind: None,
            jname: None,
        },
        episodes,
        versions: Vec::new(),
    }
}

#[tokio::test]
async fn a_tagged_continuation_maps_per_entry_numbers() {
    // The cumulative numbering lives in the tags while slots restart
    // at 1: per-entry request 1 is display 41.
    let picked = show(vec![ep(10, 1, Some("41")), ep(11, 2, Some("42"))]);
    let client = only_episode(10);
    let url = resolve_episode(&client, &picked, "1", "sub", "best")
        .await
        .expect("per-entry 1 is display 41")
        .master_url;
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
}

#[tokio::test]
async fn a_fractional_request_maps_through_the_offset_too() {
    // The strip advertises per-entry "1.5" for the provider's
    // "41.5" recap; the click must translate back the same way the
    // advertisement translated forward.
    let picked = show(vec![
        ep(10, 1, Some("41")),
        ep(12, 2, Some("41.5")),
        ep(11, 3, Some("42")),
    ]);
    let client = only_episode(12);
    let url = resolve_episode(&client, &picked, "1.5", "sub", "best")
        .await
        .expect("per-entry 1.5 is display 41.5")
        .master_url;
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
}

#[tokio::test]
async fn a_zero_padded_integer_tag_still_answers_its_episode() {
    // The provider can file the tag in a non-canonical string form:
    // "041" is display episode 41 to the offset and the cap, which
    // parse it numerically — the matcher's byte comparison against
    // the constructed "41" is the one place it dies.
    let picked = show(vec![ep(10, 1, Some("041")), ep(11, 2, Some("042"))]);
    let client = only_episode(10);
    let url = resolve_episode(&client, &picked, "1", "sub", "best")
        .await
        .expect("per-entry 1 is display 041 = 41")
        .master_url;
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
}

#[tokio::test]
async fn a_trailing_zero_fraction_matches_the_normalized_click() {
    // Availability advertises "3.50" verbatim; the frontend parses
    // extras numerically and the click comes back as "3.5". The
    // same row answered numerically for the cap and the extras —
    // it must answer the click too.
    let picked = show(vec![
        ep(1, 1, None),
        ep(2, 2, None),
        ep(3, 3, None),
        ep(4, 4, Some("3.50")),
    ]);
    let client = only_episode(4);
    let url = resolve_episode(&client, &picked, "3.5", "sub", "best")
        .await
        .expect("the normalized click finds the padded tag")
        .master_url;
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
}

proptest::proptest! {
    /// Numeric identity over padded forms: leading zeros on the
    /// integer part and trailing zeros after the fraction never
    /// change what a tag matches, in either direction; genuinely
    /// nonnumeric tags keep exact equality.
    #[test]
    fn tag_matching_ignores_numeric_padding(
        n in 0u32..100_000,
        frac in 0u32..10,
        lead in 0usize..3,
        tail in 0usize..3,
    ) {
        let canonical_int = format!("{n}");
        let padded_int = format!("{}{n}", "0".repeat(lead));
        proptest::prop_assert!(tag_matches(&padded_int, &canonical_int));
        proptest::prop_assert!(tag_matches(&canonical_int, &padded_int));
        let canonical = format!("{n}.{frac}");
        let padded = format!("{}{n}.{frac}{}", "0".repeat(lead), "0".repeat(tail));
        proptest::prop_assert!(tag_matches(&padded, &canonical));
        proptest::prop_assert!(tag_matches(&canonical, &padded));
        proptest::prop_assert!(tag_matches("SP", "SP"));
        proptest::prop_assert!(!tag_matches("SP", "sp"));
        let different = format!("{}.{frac}", n + 1);
        proptest::prop_assert!(!tag_matches(&canonical, &different));
    }
}

#[tokio::test]
async fn an_integer_request_skips_the_recap_in_its_slot() {
    // The captured provider shape: a recap occupies integer slot 4
    // (number: 4) while its displayed identity is "3.5" (number2),
    // and the true episode 4 sits one slot later under tag "4". A
    // request for episode 4 matched by slot plays the recap; the
    // display tag is the row's identity, for integers as much as
    // for decimals.
    let picked = show(vec![
        ep(1, 1, None),
        ep(2, 2, None),
        ep(3, 3, None),
        ep(4, 4, Some("3.5")),
        ep(5, 5, Some("4")),
    ]);
    let client = only_episode(5);
    let url = resolve_episode(&client, &picked, "4", "sub", "best")
        .await
        .expect("the tag-4 row carries episode 4")
        .master_url;
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
}

/// A reversed pair such as `12-1` is not a range, so the single
/// episode path resolves it as a request in its own right: it names
/// no integer and no fractional tag, matches no row of the picked
/// show, and dead-ends as the episode's own verdict — the show was
/// found and only what was asked for is missing — never as the
/// catalogue's miss and never as something a range would download.
#[tokio::test]
async fn a_reversed_range_resolves_to_the_episodes_own_verdict() {
    let picked = show(vec![ep(1, 1, None), ep(2, 2, None), ep(12, 12, None)]);
    let client = only_episode(12);
    let ne = resolve_episode(&client, &picked, "12-1", "sub", "best")
        .await
        .expect_err("a reversed pair names no episode");
    assert!(
        matches!(ne.error, AniError::EpisodeUnavailable),
        "expected the episode's verdict, got {:?}",
        ne.error
    );
    assert!(
        !ne.clean_miss,
        "the show was found; nothing here proves absence"
    );
}

#[tokio::test]
async fn a_recap_slot_without_its_true_episode_is_a_dead_end() {
    // When the listing ends on the recap, a request for the slot's
    // integer must dead-end rather than serve the recap's stream —
    // the show has no episode 4, whatever slot the recap sits in.
    let picked = show(vec![
        ep(1, 1, None),
        ep(2, 2, None),
        ep(3, 3, None),
        ep(4, 4, Some("3.5")),
    ]);
    let client = only_episode(4);
    let ne = resolve_episode(&client, &picked, "4", "sub", "best")
        .await
        .expect_err("the recap must not answer for episode 4");
    assert!(
        matches!(ne.error, AniError::EpisodeUnavailable),
        "expected the episode's verdict, got {:?}",
        ne.error
    );
    assert!(!ne.clean_miss);
}

#[tokio::test]
async fn an_integer_display_tag_still_matches_through_the_offset() {
    // Continuation cours re-display their cumulative numbering in
    // tags; the per-entry request maps through the offset into the
    // same display space the tags live in.
    let picked = show(vec![ep(10, 41, None), ep(11, 42, Some("42"))]);
    let client = only_episode(11);
    let url = resolve_episode(&client, &picked, "2", "sub", "best")
        .await
        .expect("per-entry 2 is display 42")
        .master_url;
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
}

/// Serves hianime's chain for one episode — the server list, the
/// embed page, the master — and counts every request, so a test can
/// say how many times the master was asked for.
struct HianimeChain {
    requests: std::sync::Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl crate::scraper::fetch::Fetch for HianimeChain {
    async fn fetch(&self, req: &FetchRequest) -> crate::error::Result<FetchResponse> {
        let url = req.url.as_str().to_string();
        self.requests.lock().expect("requests").push(url.clone());
        let body = if url.contains("/api/theme/episode/servers?episodeId=10") {
            r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#.to_string()
        } else if url == "https://zokoanime.video/stream/mal/1/1/sub" {
            r#"<html><body><script>window.__P="FFYSGRYPX08KERBdBQtAWwIPGwMAFQMIFEETHhdbDAoGWQAfTAhXWE4TQ1YSHhdZDBkOABcPTGoyCQ=="</script></body></html>"#.to_string()
        } else if url == "https://cdn.example/x/master.m3u8" {
            "#EXTM3U\n".to_string()
        } else {
            return Ok(FetchResponse {
                status: 404,
                body: String::new(),
                url,
            });
        };
        Ok(FetchResponse {
            status: 200,
            body,
            url,
        })
    }
}

/// The episode step validates the stream once, inside the provider's
/// walk of the servers: the master is asked for one time, not once
/// by the walk and again by a quality step outside it.
#[tokio::test]
async fn the_episode_step_asks_hianime_for_the_master_once() {
    let picked = show(vec![ep(10, 1, None)]);
    let client = crate::scraper::hianime::HianimeClient::with_base(
        HianimeChain {
            requests: std::sync::Mutex::new(Vec::new()),
        },
        "https://hianime.test",
    );
    let url = resolve_episode(&client, &picked, "1", "sub", "best")
        .await
        .expect("resolved")
        .master_url;
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
    let masters = client
        .transport()
        .requests
        .lock()
        .expect("requests")
        .iter()
        .filter(|u| u.as_str() == "https://cdn.example/x/master.m3u8")
        .count();
    assert_eq!(masters, 1, "the master is validated once, inside the walk");
}

// ── the episode step's verdict ─────────────────────────────────────

/// The chain met a failure that is not an answer — the connection to
/// a host dropped, a host timed out, a page or payload or playlist
/// could not be read. Past the searches that found the show, none of
/// those is the user's connection or the app failing to read the
/// provider: they are the stream server not serving, and the verdict
/// says so, naming the chain as a whole when the provider named no
/// stage.
#[test]
fn a_chain_failure_that_is_not_an_answer_is_the_stream_servers_failure() {
    for error in [
        AniError::Network,
        AniError::Timeout,
        AniError::ParseFailed {
            detail: "the payload marker is missing".into(),
        },
    ] {
        let shown = format!("{error:?}");
        match episode_verdict(error) {
            AniError::StreamServerFailed {
                stage: "stream",
                host: None,
            } => {}
            other => panic!("{shown} should be the stream server's failure, got {other:?}"),
        }
    }
}

/// A provider that names the stage and host itself keeps them: the
/// episode step adds a name only where there is none.
#[test]
fn a_stream_server_failure_the_provider_named_keeps_its_stage_and_host() {
    let named = episode_verdict(AniError::StreamServerFailed {
        stage: "embed",
        host: Some("zokoanime.video".into()),
    });
    assert!(
        matches!(
            &named,
            AniError::StreamServerFailed { stage: "embed", host: Some(h) } if h == "zokoanime.video"
        ),
        "{named:?}"
    );
}

/// What the chain answered, refused or had refused by the gate passes
/// through as it is — a 400 or 401 is the host rejecting this request,
/// a block or rate limit speaks for the provider, the gate speaks for
/// itself — and absence is the episode's own verdict.
#[test]
fn answers_blocks_and_refusals_pass_through_the_episode_step_as_they_are() {
    for error in [
        AniError::Upstream { status: 400 },
        AniError::Upstream { status: 401 },
        AniError::Upstream { status: 403 },
        AniError::Upstream { status: 429 },
        AniError::Upstream { status: 503 },
        AniError::RateLimited {
            retry_after_secs: Some(3),
        },
        AniError::GateRefused,
    ] {
        let shown = format!("{error:?}");
        assert_eq!(format!("{:?}", episode_verdict(error)), shown);
    }
    for absent in [
        AniError::NoResults,
        AniError::Upstream { status: 404 },
        AniError::Upstream { status: 410 },
    ] {
        assert!(matches!(
            episode_verdict(absent),
            AniError::EpisodeUnavailable
        ));
    }
}

/// Serves the server list for one episode whose only server sits on
/// an embed host the connection to which dies.
struct DeadEmbedHost;

#[async_trait::async_trait]
impl crate::scraper::fetch::Fetch for DeadEmbedHost {
    async fn fetch(&self, req: &FetchRequest) -> crate::error::Result<FetchResponse> {
        let url = req.url.as_str().to_string();
        if url.contains("/api/theme/episode/servers?episodeId=11") {
            return Ok(FetchResponse {
                status: 200,
                body: r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#.to_string(),
                url,
            });
        }
        if url == "https://zokoanime.video/stream/mal/1/1/sub" {
            return Err(AniError::Network);
        }
        Ok(FetchResponse {
            status: 404,
            body: String::new(),
            url,
        })
    }
}

/// Serves the server list for one episode whose only server sits on
/// an embed host that answers HTTP 200 with its own not-found card —
/// zokoanime's page for an episode it has nothing for, as recorded.
struct NotFoundPage;

#[async_trait::async_trait]
impl crate::scraper::fetch::Fetch for NotFoundPage {
    async fn fetch(&self, req: &FetchRequest) -> crate::error::Result<FetchResponse> {
        let url = req.url.as_str().to_string();
        let body = if url.contains("/api/theme/episode/servers?episodeId=12") {
            r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC81NDE5Ni8xL3N1Yg==\"></div>"}"#.to_string()
        } else if url == "https://zokoanime.video/stream/mal/54196/1/sub" {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("repo root")
                .join("tests/fixtures/hianime/embed/zokoanime-not-found.html");
            std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        } else {
            return Ok(FetchResponse {
                status: 404,
                body: String::new(),
                url,
            });
        };
        Ok(FetchResponse {
            status: 200,
            body,
            url,
        })
    }
}

/// The stream server answering, on its own page, that it has nothing
/// for the episode is the episode's own verdict — the show is listed,
/// this episode has no stream — not the server failing, and not the
/// user's connection.
#[tokio::test]
async fn the_episode_step_reads_the_stream_servers_own_not_found_page_as_the_episodes_absence() {
    let picked = show(vec![ep(12, 1, None)]);
    let client =
        crate::scraper::hianime::HianimeClient::with_base(NotFoundPage, "https://hianime.test");
    let ne = resolve_episode(&client, &picked, "1", "sub", "best")
        .await
        .expect_err("the host has nothing for the episode");
    assert!(
        matches!(ne.error, AniError::EpisodeUnavailable),
        "{:?}",
        ne.error
    );
    assert!(!ne.clean_miss, "the show was found; nothing persists");
}

/// Through hianime's client the verdict names the host whose
/// connection died and the stage it died at — what the log line that
/// reports the failed play needs to make the next such report
/// traceable — and is never the transport failure under its own name.
#[tokio::test]
async fn the_episode_step_names_the_host_whose_connection_died() {
    let picked = show(vec![ep(11, 1, None)]);
    let client =
        crate::scraper::hianime::HianimeClient::with_base(DeadEmbedHost, "https://hianime.test");
    let ne = resolve_episode(&client, &picked, "1", "sub", "best")
        .await
        .expect_err("nothing served");
    assert!(
        matches!(
            &ne.error,
            AniError::StreamServerFailed { stage: "embed", host: Some(h) } if h == "zokoanime.video"
        ),
        "{:?}",
        ne.error
    );
    assert!(!ne.clean_miss);
}

/// A provider's walk can end in a timeout no attempt produced: its
/// own bound cut a server off while the gate still held the request,
/// so no host failed. The provider stamps no attempt on that
/// verdict, and the episode step leaves it its name — the deadline's
/// shape, which the breaker reads as ambiguous for a paced background
/// resolve — rather than the stream server's failure, which would say
/// a host did not serve and is distress on every priority. A timeout
/// the provider does stamp is a host that stalled on a request the
/// gate had admitted, and is the stream server's.
#[tokio::test]
async fn a_timeout_the_provider_stamps_no_attempt_on_keeps_its_name() {
    let picked = show(vec![ep(702, 1, None)]);
    let timed_out = || {
        crate::scraper::table_provider::TableProvider::new().stream_from(
            702,
            "sub",
            crate::scraper::table_provider::Answer::fails(|| AniError::Timeout),
        )
    };
    let ne = resolve_episode(&timed_out(), &picked, "1", "sub", "best")
        .await
        .expect_err("nothing served");
    assert!(
        matches!(ne.error, AniError::Timeout),
        "unstamped, the timeout is the deadline's shape: {:?}",
        ne.error
    );
    assert!(!ne.clean_miss);
    let stalled = timed_out().stamped(tokio::time::Instant::now());
    let ne = resolve_episode(&stalled, &picked, "1", "sub", "best")
        .await
        .expect_err("nothing served");
    assert!(
        matches!(
            ne.error,
            AniError::StreamServerFailed {
                stage: "stream",
                host: None
            }
        ),
        "stamped, a host stalled: {:?}",
        ne.error
    );
}

proptest::proptest! {
    /// The chain-failure decision table over every error shape: a
    /// provider block or gate refusal stops the walk with the error
    /// intact, answered dead ends (the episode's own verdict,
    /// NoResults, not-found-shaped upstream statuses) move to the
    /// next alias, and everything else — the stream server's failure,
    /// transport weather that reached the walk unnamed, and an
    /// upstream status that is neither a block nor absence — stays
    /// transient, with the error intact too: an exhausted walk
    /// surfaces an answered status as the status, which the breaker
    /// reads as the provider answering, not as a transport failure.
    #[test]
    fn chain_failures_classify_by_the_decision_table(
        kind in 0u8..8,
        status in 100u16..600,
    ) {
        let error = match kind {
            0 => AniError::Upstream { status },
            1 => AniError::GateRefused,
            2 => AniError::NoResults,
            3 => AniError::Network,
            4 => AniError::Timeout,
            5 => AniError::EpisodeUnavailable,
            6 => AniError::StreamServerFailed {
                stage: "embed",
                host: Some("zokoanime.video".into()),
            },
            _ => AniError::RateLimited {
                retry_after_secs: None,
            },
        };
        let stops = error.is_provider_block() || matches!(error, AniError::GateRefused);
        let dead_end = !stops
            && (matches!(error, AniError::EpisodeUnavailable | AniError::NoResults)
                || matches!(error, AniError::Upstream { status } if status == 404 || status == 410));
        let shown = format!("{error:?}");
        let ne = NativeError {
            error,
            clean_miss: false,
            failed_at: None,
            refused_by_name: false,
        };
        match classify_chain_failure(ne) {
            ChainOutcome::Stop(kept) => {
                proptest::prop_assert!(stops, "only blocks and refusals stop the walk");
                proptest::prop_assert!(
                    kept.error.is_provider_block()
                        || matches!(kept.error, AniError::GateRefused),
                    "the stop keeps the error's identity"
                );
            }
            ChainOutcome::DeadEnd => proptest::prop_assert!(dead_end),
            ChainOutcome::Transient(kept) => {
                proptest::prop_assert!(!stops && !dead_end);
                proptest::prop_assert_eq!(
                    format!("{:?}", kept.error),
                    shown,
                    "the transient outcome keeps the error's identity"
                );
            }
        }
    }
}

// ── the listing's bound is the episode step's ─────────────────────

/// Lists one episode whose stream carries the given subtitle tracks.
struct ListedTracks(Vec<crate::scraper::provider::SubtitleTrack>);

#[async_trait::async_trait]
impl crate::scraper::provider::Provider for ListedTracks {
    fn id(&self) -> crate::scraper::provider::ProviderId {
        crate::scraper::provider::ProviderId::TestA
    }
    async fn search(&self, _q: &str) -> crate::error::Result<Vec<BrowseHit>> {
        Ok(vec![BrowseHit {
            slug: "the-show-77".into(),
            title: "The Show".into(),
            kind: None,
            jname: None,
        }])
    }
    async fn episodes(&self, _s: &str) -> crate::error::Result<Vec<EpisodeRef>> {
        Ok(vec![ep(1, 1, None)])
    }
    async fn has_mode(&self, _e: u64, _m: &str) -> crate::error::Result<bool> {
        Ok(true)
    }
    async fn master_playlist_url(
        &self,
        _e: u64,
        _m: &str,
    ) -> crate::error::Result<crate::scraper::provider::StreamSource> {
        Ok(crate::scraper::provider::StreamSource {
            master_url: "https://cdn.example/x/master.m3u8".into(),
            referer: Some("https://embed.example/".into()),
            subtitles: self.0.clone(),
        })
    }
    async fn playlist(&self, _u: &str, _r: Option<&str>) -> crate::error::Result<String> {
        Ok("#EXTM3U\n".into())
    }
    async fn detail_year(&self, _s: &str) -> crate::error::Result<Option<u32>> {
        Ok(None)
    }
    fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        None
    }
}

/// The episode step has two callers — the play resolve and the range
/// download — and the range download took the listing as the
/// provider gave it, so a default the listing put past the cap
/// reached the sidecar writer, which keeps the first cap-many, and
/// the ranged download lost the provider's own choice while the
/// single-episode download kept it. The bound is the episode step's,
/// so every caller shares it.
#[tokio::test]
async fn the_episode_step_bounds_the_listing_and_keeps_its_default() {
    use crate::proxy::upstream::SUBTITLE_TRACK_CAP;
    let total = SUBTITLE_TRACK_CAP + 5;
    let tracks: Vec<_> = (0..total)
        .map(|n| crate::scraper::provider::SubtitleTrack {
            lang: format!("l{n}"),
            label: format!("Language {n}"),
            default: n == total - 1,
            url: format!("https://cdn.example/subs/l{n}.vtt"),
        })
        .collect();
    let resolved = resolve_episode(
        &ListedTracks(tracks),
        &show(vec![ep(1, 1, None)]),
        "1",
        "sub",
        "best",
    )
    .await
    .expect("resolved");
    assert_eq!(resolved.subtitles.len(), SUBTITLE_TRACK_CAP);
    let kept: Vec<&str> = resolved.subtitles.iter().map(|t| t.lang.as_str()).collect();
    let expected: Vec<String> = (0..SUBTITLE_TRACK_CAP - 1)
        .map(|n| format!("l{n}"))
        .chain(std::iter::once(format!("l{}", total - 1)))
        .collect();
    assert_eq!(
        kept, expected,
        "the first cap-many in listing order, the default in the last slot"
    );
    assert!(
        resolved.subtitles.last().is_some_and(|t| t.default),
        "the provider's default is kept past the cap"
    );
}

mod verdict_props {
    use super::episode_verdict;
    use crate::commands::providers::fails_over;
    use crate::error::AniError;
    use proptest::prelude::*;

    /// What the episode step makes of a failure its chain met.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Verdict {
        /// Left as it came.
        Kept,
        /// The episode's own absence.
        Absent,
        /// The stream server not serving, named for the chain as a
        /// whole.
        StreamServer,
    }

    /// A failure the episode chain can hand the verdict, paired with
    /// what the verdict makes of it. The set is what the step can
    /// actually meet: the kinds the provider layer constructs —
    /// transport failure, timeout, gate refusal, rate limit, parse
    /// failure, upstream status, empty answer, the stream server's
    /// own named failure — plus the verdict itself, which the step
    /// raises on a listing without the row and which a second
    /// classification must leave alone.
    ///
    /// The expectation is stated by the rule rather than asked of the
    /// production helpers: an answered dead end — a title miss or a
    /// not-found-shaped status, 404 or 410 — becomes the episode's
    /// absence; a failure that is not an answer — a transport
    /// failure, a timeout, a parse failure — becomes the stream
    /// server not serving, since past the searches that found the
    /// show it is neither the connection nor the app failing to read
    /// the provider; everything else stays as it came. A block or a
    /// gate refusal speaks for the provider or the gate; any other
    /// upstream status is the provider rejecting this request — a 400
    /// or 401 — and told to try another episode the user would be
    /// misled; a stream server failure the provider named keeps its
    /// name.
    fn error_and_the_verdicts_reading() -> impl Strategy<Value = (AniError, Verdict)> {
        prop_oneof![
            any::<()>().prop_map(|()| (AniError::Network, Verdict::StreamServer)),
            any::<()>().prop_map(|()| (AniError::Timeout, Verdict::StreamServer)),
            any::<()>().prop_map(|()| (AniError::GateRefused, Verdict::Kept)),
            prop::option::of(0u64..100_000).prop_map(|retry_after_secs| {
                (AniError::RateLimited { retry_after_secs }, Verdict::Kept)
            }),
            "[a-z ]{0,24}"
                .prop_map(|detail| (AniError::ParseFailed { detail }, Verdict::StreamServer)),
            (0u16..1000).prop_map(|status| {
                // Only a not-found-shaped status is an answer about
                // the episode; a block, and a 400 or 401 rejecting
                // this request, say nothing about it and pass through.
                let absent = status == 404 || status == 410;
                (
                    AniError::Upstream { status },
                    if absent {
                        Verdict::Absent
                    } else {
                        Verdict::Kept
                    },
                )
            }),
            any::<()>().prop_map(|()| (AniError::NoResults, Verdict::Absent)),
            any::<()>().prop_map(|()| (AniError::EpisodeUnavailable, Verdict::Kept)),
            prop::option::of("[a-z.]{1,12}").prop_map(|host| {
                (
                    AniError::StreamServerFailed {
                        stage: "embed",
                        host,
                    },
                    Verdict::Kept,
                )
            }),
        ]
    }

    proptest! {
        /// An answered dead end — a title miss or a not-found-shaped
        /// status — becomes the episode's absence; a failure that is
        /// not an answer becomes the stream server not serving; a
        /// block, a gate refusal, any other status and a failure the
        /// provider already named pass through as they came. And
        /// whichever it is, the walk's reading of the failure survives
        /// it: one the walk would have moved on from still moves it
        /// on, so no classification here can strand a caller on a
        /// provider that has stopped working.
        #[test]
        fn the_episode_step_reads_each_failure_by_the_rule_and_nothing_else_changes(
            (error, reading) in error_and_the_verdicts_reading(),
        ) {
            let before = format!("{error:?}");
            let moved_on = fails_over(&error);
            let after = episode_verdict(error);
            let after_dbg = format!("{after:?}");
            match reading {
                Verdict::Kept => prop_assert_eq!(&after_dbg, &before),
                Verdict::Absent => prop_assert!(
                    matches!(after, AniError::EpisodeUnavailable),
                    "{before} -> {after_dbg}"
                ),
                Verdict::StreamServer => prop_assert!(
                    matches!(after, AniError::StreamServerFailed { stage: "stream", host: None }),
                    "{before} -> {after_dbg}"
                ),
            }
            prop_assert_eq!(
                fails_over(&after),
                moved_on,
                "the verdict may not change whether the walk moves on: {} -> {}",
                before,
                after_dbg
            );
        }
    }
}
