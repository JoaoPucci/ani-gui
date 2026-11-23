//! What a lane logs at the default level for each request it sends: one
//! line, once the answer lands, with the kind, which request, how many
//! ids it carried, how it ended and how long it took — and no ids.

use super::*;
use crate::meta::anilist_batch::Want;
use crate::meta::anilist_gate::in_background;
use crate::meta::anilist_streaming_eps_fake_test::{FakeAniList, FakeMedia};

/// Everything the capturing subscriber wrote.
#[derive(Clone, Default)]
struct Captured(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}

/// Run `f` on a fresh runtime with the app's default filter writing to
/// a capture; the lines the lanes logged.
fn lines_of<F: Future>(f: impl FnOnce() -> F) -> Vec<String> {
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(
            crate::DEFAULT_LOG_FILTER,
        ))
        .with_ansi(false)
        // No timestamp: the lines are searched for ids, and a clock's
        // digits would match one.
        .without_time()
        .with_writer(captured.clone())
        .finish();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    // See `anilist_gate_test.rs`: a second dispatcher kept alive makes
    // every callsite registration ask this test's subscriber too.
    let _second = tracing::Dispatch::new(tracing_subscriber::registry());
    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        runtime.block_on(f());
    });
    let out =
        String::from_utf8_lossy(&captured.0.lock().unwrap_or_else(|e| e.into_inner())).into_owned();
    out.lines()
        .filter(|l| l.contains("anilist lane request answered"))
        .map(str::to_owned)
        .collect()
}

fn fake() -> FakeAniList {
    FakeAniList {
        media: vec![
            FakeMedia {
                id: 101,
                mal: Some(1),
                listed: 1,
            },
            FakeMedia {
                id: 102,
                mal: Some(2),
                listed: 1,
            },
            FakeMedia {
                id: 109,
                mal: Some(9),
                listed: 1,
            },
            FakeMedia {
                id: 30,
                mal: None,
                listed: 1,
            },
        ],
    }
}

async fn serve(respond: impl wiremock::Respond + 'static) -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(respond)
        .mount(&server)
        .await;
    server
}

fn by_mal(mal: u32) -> Want {
    Want {
        mal: Some(mal),
        anilist: None,
    }
}

/// A strip's lookups through `lookup`: show 9 opens the lane, then two
/// MAL-mapped shows and one mapped to AniList alone gather behind it, in
/// the same turn — polled in order, on one thread, before the lane's
/// driver can take a window.
async fn strip<F, Fut, T>(lookup: F)
where
    F: Fn(Want) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let lookup = &lookup;
    let rest = async {
        tokio::join!(
            lookup(by_mal(1)),
            lookup(by_mal(2)),
            lookup(Want {
                mal: None,
                anilist: Some(30),
            })
        )
    };
    let (first, (a, b, c)) = tokio::join!(lookup(by_mal(9)), rest);
    assert!(first.is_ok() && a.is_ok() && b.is_ok() && c.is_ok());
}

fn has(line: &str, field: &str) -> bool {
    line.split_whitespace().any(|w| w == field)
}

#[test]
fn each_request_a_stills_lane_sends_logs_one_line_with_its_page_and_size() {
    let lines = lines_of(|| async {
        let server = serve(fake()).await;
        let base = server.uri();
        let client = reqwest::Client::new();
        strip(|want| {
            in_background(
                crate::meta::anilist_streaming_eps::streaming_listing_coalesced(
                    &client,
                    "log-stills",
                    want,
                    Some(&base),
                ),
            )
        })
        .await;
    });
    assert_eq!(lines.len(), 3, "{lines:#?}");
    let single = lines
        .iter()
        .find(|l| l.contains("request=\"single\""))
        .expect("the opener's line");
    assert!(
        has(single, "kind=\"stills\"") && has(single, "ids=1"),
        "{single}"
    );
    let mal = lines
        .iter()
        .find(|l| l.contains("request=\"mal_page\""))
        .expect("the MAL page's line");
    assert!(has(mal, "kind=\"stills\"") && has(mal, "ids=2"), "{mal}");
    let id = lines
        .iter()
        .find(|l| l.contains("request=\"anilist_id_page\""))
        .expect("the AniList-id page's line");
    assert!(has(id, "ids=1"), "{id}");
    for line in &lines {
        assert!(line.contains(" INFO "), "{line}");
        assert!(has(line, "status=\"ok\""), "{line}");
        assert!(
            line.split_whitespace().any(|w| w.starts_with("ms=")),
            "{line}"
        );
        for id in ["101", "102", "109", "=30", "=9 "] {
            assert!(!line.contains(id), "a show's id in the log: {line}");
        }
    }
}

#[test]
fn a_banner_lane_logs_its_kind() {
    let lines = lines_of(|| async {
        let server = serve(fake()).await;
        let base = server.uri();
        let client = reqwest::Client::new();
        strip(|want| {
            in_background(crate::meta::anilist_media::banner_coalesced(
                &client,
                "log-banner",
                want,
                Some(&base),
            ))
        })
        .await;
    });
    assert_eq!(lines.len(), 3, "{lines:#?}");
    assert!(
        lines.iter().all(|l| has(l, "kind=\"banner\"")),
        "{lines:#?}"
    );
}

#[test]
fn a_refused_request_logs_how_it_ended() {
    let lines = lines_of(|| async {
        let server =
            serve(wiremock::ResponseTemplate::new(429).insert_header("retry-after", "60")).await;
        let base = server.uri();
        let got = in_background(
            crate::meta::anilist_streaming_eps::streaming_listing_coalesced(
                &reqwest::Client::new(),
                "log-refused",
                by_mal(1),
                Some(&base),
            ),
        )
        .await;
        assert!(got.is_err());
    });
    assert_eq!(lines.len(), 1, "{lines:#?}");
    assert!(
        has(&lines[0], "status=\"error.network.rate_limited\""),
        "{}",
        lines[0]
    );
}
