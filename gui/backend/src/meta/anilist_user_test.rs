use super::*;
use crate::account::pkce::Pkce;
use crate::account::status::ListStatus;
use crate::error::AniError;

/// Build the wiremock-backed provider used across the network tests.
/// `api_uri` / `token_uri` are wiremock server URIs.
fn make_provider(api_uri: &str, token_uri: &str) -> AniListProvider {
    AniListProvider::with_bases(
        reqwest::Client::new(),
        api_uri.to_string(),
        token_uri.to_string(),
    )
}

/// Tokens with a non-zero `expires_at_epoch_s` so the placeholder
/// passes the trait's expiry checks. Tests that exercise the network
/// layer don't care about the actual expiry value.
fn dummy_tokens() -> Tokens {
    Tokens {
        access_token: "test-access-token".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

/// AniList's consent URL is fixed-shape — the `state` we generate has
/// to round-trip through the browser back into our loopback handler,
/// so the URL must carry it verbatim. Pin the query keys + literal
/// values (client_id, redirect_uri, response_type) too because a
/// silent rename would make the live OAuth flow fail without the
/// trait surface changing.
#[test]
fn auth_url_builds_anilist_consent_url_with_state() {
    let provider = AniListProvider::new(reqwest::Client::new());
    // AniList ignores PKCE entirely, but the trait shape passes one in
    // for symmetry with MAL. Build a plain pair just so we have one.
    let pkce = Pkce::new_plain();
    let url = provider
        .auth_url(&pkce, "csrf-token-xyz")
        .expect("anilist auth_url");

    assert!(
        url.starts_with("https://anilist.co/api/v2/oauth/authorize?"),
        "auth_url must point at AniList's authorize endpoint: {url}"
    );
    assert!(
        url.contains("client_id=43143"),
        "auth_url must include the configured client_id: {url}"
    );
    assert!(
        url.contains("response_type=code"),
        "auth_url must request the code grant: {url}"
    );
    assert!(
        url.contains("state=csrf-token-xyz"),
        "auth_url must carry the state verbatim for CSRF round-trip: {url}"
    );
    // AniList allows exactly one redirect URI per app, so any mismatch
    // here surfaces as a hard upstream rejection. URL-encoded form is
    // what reqwest's url crate produces.
    assert!(
        url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A53682%2Fcallback"),
        "auth_url must include the encoded loopback redirect_uri: {url}"
    );
    // AniList ignores PKCE — pin that we do NOT advertise a challenge,
    // so a future "fix" that copies MAL's PKCE handling doesn't sneak
    // an invalid param into the live URL.
    assert!(
        !url.contains("code_challenge"),
        "auth_url must NOT advertise PKCE (AniList ignores it): {url}"
    );
}

/// Real shape from AniList's documented token-exchange response. The
/// 1-year `expires_in` is what makes their refresh flow superfluous;
/// no `refresh_token` field exists in the doc'd response either.
const TOKEN_RESPONSE_BODY: &str = r#"{
    "token_type": "Bearer",
    "expires_in": 31536000,
    "access_token": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.test-access-token",
    "refresh_token": null
}"#;

#[tokio::test]
async fn exchange_code_parses_access_token_and_expiry() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": "43143",
            "client_secret": "B5ay8XNwslA819aukVNC8ejQBvUXtpuLiPf1yvL7",
            "redirect_uri": "http://localhost:53682/callback",
            "code": "the-auth-code"
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(TOKEN_RESPONSE_BODY))
        .mount(&server)
        .await;

    let provider = make_provider("http://unused-api", &server.uri());
    let pkce = Pkce::new_plain();
    let now_floor = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let tokens = provider
        .exchange_code("the-auth-code", &pkce)
        .await
        .expect("exchange ok");
    assert_eq!(
        tokens.access_token,
        "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.test-access-token"
    );
    // AniList doesn't return a refresh_token; trait carries None.
    assert!(tokens.refresh_token.is_none());
    // expires_at must be ~now + 1 year. Allow a generous lower-bound
    // window so a slow CI runner doesn't flake. Upper bound catches
    // a future bug where the conversion accidentally double-counts.
    let expected_min = now_floor + 31_536_000 - 5;
    let expected_max = now_floor + 31_536_000 + 60;
    assert!(
        tokens.expires_at_epoch_s >= expected_min && tokens.expires_at_epoch_s <= expected_max,
        "expires_at_epoch_s ({}) must be within [{}, {}]",
        tokens.expires_at_epoch_s,
        expected_min,
        expected_max
    );
}

/// Build a JWT-shaped access token whose payload contains a known
/// `exp` claim, so a test can pin that the parser pulls the expiry
/// out of the JWT when the wire response omits `expires_in`.
///
/// Header + signature are intentionally fixed garbage — we never
/// verify the signature, we only base64url-decode the payload.
/// Codex P1 #3371176290.
fn fake_anilist_jwt_with_exp(exp_epoch_s: i64) -> String {
    use base64::Engine;
    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
    let payload_json = format!(r#"{{"sub":"5921","exp":{exp_epoch_s}}}"#);
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
    // The signature segment is base64url too, but we never decode it.
    // A literal "sig" is fine; the parser splits on '.' and only
    // touches the middle segment.
    format!("{header}.{payload}.sig")
}

/// AniList's documented Authorization-Code response carries
/// `expires_in`. Their *live* response sometimes omits it (their
/// tokens are essentially non-expiring and the field is documented
/// inconsistently). The earlier required-`expires_in` decoder would
/// reject an otherwise valid exchange as `ParseFailed` and the
/// Connect flow would fail before persisting the token. Codex P1
/// #3371176290.
///
/// This test fixture's body omits `expires_in` entirely; the access
/// token IS a JWT-shaped string with a known `exp` claim, so the
/// parser must fall back to JWT decoding to recover the expiry.
#[tokio::test]
async fn exchange_code_falls_back_to_jwt_exp_when_expires_in_missing() {
    let exp = 2_000_000_000_i64; // 2033-05-18 — well past any test wall clock
    let access_token = fake_anilist_jwt_with_exp(exp);
    let body = format!(
        r#"{{"token_type":"Bearer","access_token":"{access_token}","refresh_token":null}}"#
    );
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(&body))
        .mount(&server)
        .await;
    let provider = make_provider("http://unused-api", &server.uri());
    let tokens = provider
        .exchange_code("the-code", &Pkce::new_plain())
        .await
        .expect("exchange must succeed even without expires_in");
    assert_eq!(
        tokens.expires_at_epoch_s, exp,
        "must read expiry from JWT exp claim"
    );
}

/// Defensive fallback: when AniList ever returns a non-JWT access
/// token AND omits `expires_in`, the parser uses a 1-year sentinel
/// rather than failing the exchange. Codex P1 #3371176290 — the user
/// would otherwise be stuck on a stale connect flow on every retry
/// even though the token itself is fine.
#[tokio::test]
async fn exchange_code_falls_back_to_one_year_sentinel_when_jwt_decode_fails() {
    let body = r#"{
        "token_type": "Bearer",
        "access_token": "not-a-jwt-just-an-opaque-string",
        "refresh_token": null
    }"#;
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    let provider = make_provider("http://unused-api", &server.uri());
    let now_floor = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let tokens = provider
        .exchange_code("the-code", &Pkce::new_plain())
        .await
        .expect("exchange must succeed; expiry falls back to sentinel");
    // Sentinel = ~1 year. Generous bounds so a slow CI doesn't flake.
    let expected_min = now_floor + 31_536_000 - 5;
    let expected_max = now_floor + 31_536_000 + 60;
    assert!(
        tokens.expires_at_epoch_s >= expected_min && tokens.expires_at_epoch_s <= expected_max,
        "sentinel expiry ({}) must be ~1y from now [{}, {}]",
        tokens.expires_at_epoch_s,
        expected_min,
        expected_max
    );
}

#[tokio::test]
async fn exchange_code_surfaces_upstream_4xx_as_upstream_error() {
    // AniList returns 400 on a stale / replayed code with a JSON
    // error body; the trait surface collapses that to Upstream so
    // the route layer can map it to a re-auth prompt.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(400).set_body_string(r#"{"error":"invalid_request"}"#),
        )
        .mount(&server)
        .await;
    let provider = make_provider("http://unused-api", &server.uri());
    let err = provider
        .exchange_code("stale-code", &Pkce::new_plain())
        .await
        .expect_err("400 must surface as Err");
    assert!(
        matches!(err, AniError::Upstream { status: 400 }),
        "expected Upstream {{ status: 400 }}, got {err:?}"
    );
    // Suppress dead-code warnings until later pairs land.
    let _ = dummy_tokens();
}

/// Real-shape response to `query Viewer { Viewer { … } }`. Numeric
/// `id` (not stringy), avatar bag, statistics with anime stats
/// nested two levels deep. AniList's `meanScore` is already on the
/// 0..=10 scale here — pass through.
const VIEWER_RESPONSE_BODY: &str = r#"{
    "data": {
        "Viewer": {
            "id": 5921,
            "name": "pucci",
            "avatar": {
                "large": "https://s4.anilist.co/file/anilistcdn/user/avatar/large/b5921-x.png",
                "medium": "https://s4.anilist.co/file/anilistcdn/user/avatar/medium/b5921-x.png"
            },
            "statistics": {
                "anime": {
                    "count": 312,
                    "meanScore": 74.0
                }
            }
        }
    }
}"#;

#[tokio::test]
async fn me_parses_viewer_query_response() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/"))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer test-access-token",
        ))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(VIEWER_RESPONSE_BODY))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let profile = provider.me(&dummy_tokens()).await.expect("me ok");

    assert_eq!(profile.provider, ProviderKind::AniList);
    // AniList ids are numeric on the wire; UserProfile.user_id is a
    // String so the same shape fits MAL's "@me" id later.
    assert_eq!(profile.user_id, "5921");
    assert_eq!(profile.username, "pucci");
    assert_eq!(
        profile.avatar_url.as_deref(),
        Some("https://s4.anilist.co/file/anilistcdn/user/avatar/large/b5921-x.png")
    );
    let stats = profile.stats.expect("stats present");
    assert_eq!(stats.anime_count, 312);
    // AniList's meanScore wire format is 0..=100 (percentage points)
    // regardless of the user's chosen scoring system; UserStats's
    // contract is 0..=10, so 74.0 from the wire rescales to 7.4 here.
    // Codex P2 #3370087028: prior pass-through surfaced 65.5/10 for a
    // 100-point user.
    assert!(
        matches!(stats.mean_score_0_to_10, Some(v) if (v - 7.4).abs() < 0.001),
        "mean_score_0_to_10: {:?}",
        stats.mean_score_0_to_10
    );
}

#[tokio::test]
async fn me_surfaces_401_as_invalid_token() {
    // A 401 on the Viewer query means the bearer is bad — almost
    // always because the user revoked the app from anilist.co or
    // (rare) because the 1-year JWT silently expired. The route
    // layer surfaces this distinctly from generic Upstream so the
    // /account page can show "Sign in again" instead of a retry.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(401)
                .set_body_string(r#"{"errors":[{"message":"Invalid token"}]}"#),
        )
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let err = provider
        .me(&dummy_tokens())
        .await
        .expect_err("401 must surface as Err");
    assert!(
        matches!(err, AniError::InvalidToken),
        "expected AniError::InvalidToken, got {err:?}"
    );
}

/// Minimal but realistic single-chunk MediaListCollection — two
/// entries across two status buckets, hitting the optional fields
/// (idMal Some + None, score 0.0 → None, score 75.0 → Some(75), title
/// fallback chain). `hasNextChunk: false` ends the pagination loop
/// in one round-trip.
const LIST_CHUNK_1_BODY: &str = r#"{
    "data": {
        "MediaListCollection": {
            "hasNextChunk": false,
            "lists": [
                {
                    "status": "CURRENT",
                    "entries": [
                        {
                            "mediaId": 21,
                            "status": "CURRENT",
                            "progress": 1043,
                            "score": 75.0,
                            "updatedAt": 1700000000,
                            "repeat": 0,
                            "media": {
                                "idMal": 21,
                                "title": {
                                    "romaji": "ONE PIECE",
                                    "english": null,
                                    "userPreferred": "One Piece"
                                }
                            }
                        }
                    ]
                },
                {
                    "status": "PLANNING",
                    "entries": [
                        {
                            "mediaId": 999999,
                            "status": "PLANNING",
                            "progress": 0,
                            "score": 0.0,
                            "updatedAt": 1700000001,
                            "repeat": 0,
                            "media": {
                                "idMal": null,
                                "title": {
                                    "romaji": "Lonely Anime",
                                    "english": null,
                                    "userPreferred": null
                                }
                            }
                        }
                    ]
                }
            ]
        }
    }
}"#;

/// Mount a Viewer response on the wiremock server so list_all can
/// resolve the user id internally. Matches the request body by the
/// `Viewer` keyword in the query — narrower than method+path alone,
/// looser than a literal whole-body match (which would lock the
/// helper to the exact whitespace in VIEWER_GQL).
async fn mount_viewer(server: &wiremock::MockServer) {
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("query Viewer"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(VIEWER_RESPONSE_BODY))
        .mount(server)
        .await;
}

#[tokio::test]
async fn list_all_parses_single_chunk_with_status_score_and_mal_id() {
    let server = wiremock::MockServer::start().await;
    mount_viewer(&server).await;
    // Chunk 1 only — hasNextChunk=false ends the loop immediately.
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "variables": { "userId": 5921, "chunk": 1 }
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(LIST_CHUNK_1_BODY))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let entries = provider
        .list_all(&dummy_tokens())
        .await
        .expect("list_all ok");

    assert_eq!(entries.len(), 2);

    // First entry — ONE PIECE, CURRENT, score 75.0 → Some(75) on the
    // unified 0..=100 scale. The GraphQL query requests
    // `score(format: POINT_100)` so this value is independent of the
    // user's AniList scoring preference (per Codex P2 fix), mal_id
    // present, updated timestamp passes through, title falls back to
    // userPreferred when present.
    let watching = entries
        .iter()
        .find(|e| e.media_id == ProviderMediaId(21))
        .expect("watching entry");
    assert_eq!(watching.provider, ProviderKind::AniList);
    assert_eq!(watching.status, ListStatus::Watching);
    assert_eq!(watching.progress_episodes, 1043);
    assert_eq!(watching.score_0_to_100, Some(75));
    assert_eq!(watching.mal_id, Some(21));
    assert_eq!(watching.updated_at_epoch_s, 1_700_000_000);
    assert_eq!(watching.title, "One Piece");

    // Second entry — PLANNING, score 0.0 → None (unrated), mal_id
    // None falls through gracefully, title falls back from
    // userPreferred → romaji.
    let planning = entries
        .iter()
        .find(|e| e.media_id == ProviderMediaId(999_999))
        .expect("planning entry");
    assert_eq!(planning.status, ListStatus::Planning);
    assert!(
        planning.score_0_to_100.is_none(),
        "score 0.0 unrated → None"
    );
    assert!(planning.mal_id.is_none());
    assert_eq!(planning.title, "Lonely Anime");
}

#[tokio::test]
async fn list_all_paginates_until_has_next_chunk_false() {
    // Two chunks: the first carries one entry + hasNextChunk=true,
    // the second carries another entry + hasNextChunk=false. The
    // paginator must increment the chunk variable and stop when the
    // flag flips off — otherwise it would loop forever on a busy
    // user with hundreds of entries.
    let server = wiremock::MockServer::start().await;
    mount_viewer(&server).await;
    let chunk_1 = r#"{
        "data": {
            "MediaListCollection": {
                "hasNextChunk": true,
                "lists": [{
                    "status": "COMPLETED",
                    "entries": [{
                        "mediaId": 1,
                        "status": "COMPLETED",
                        "progress": 12,
                        "score": 8.0,
                        "updatedAt": 1600000000,
                        "repeat": 1,
                        "media": { "idMal": 100, "title": { "romaji": "First Chunk", "english": null, "userPreferred": null } }
                    }]
                }]
            }
        }
    }"#;
    let chunk_2 = r#"{
        "data": {
            "MediaListCollection": {
                "hasNextChunk": false,
                "lists": [{
                    "status": "COMPLETED",
                    "entries": [{
                        "mediaId": 2,
                        "status": "COMPLETED",
                        "progress": 24,
                        "score": 9.0,
                        "updatedAt": 1600000001,
                        "repeat": 0,
                        "media": { "idMal": 200, "title": { "romaji": "Second Chunk", "english": null, "userPreferred": null } }
                    }]
                }]
            }
        }
    }"#;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "variables": { "chunk": 1 }
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(chunk_1))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "variables": { "chunk": 2 }
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(chunk_2))
        .mount(&server)
        .await;

    let provider = make_provider(&server.uri(), "http://unused-token");
    let entries = provider
        .list_all(&dummy_tokens())
        .await
        .expect("list_all ok");
    let ids: Vec<u32> = entries.iter().map(|e| e.media_id.0).collect();
    assert_eq!(ids, vec![1, 2], "both chunks merged in order");
}

#[tokio::test]
async fn list_all_status_translation_covers_every_anilist_variant() {
    // Pins the from_anilist translation in context — a bad mapping
    // would silently bucket entries into the wrong rail (Plan-to-
    // Watch showing entries the user marked Completed).
    let server = wiremock::MockServer::start().await;
    mount_viewer(&server).await;
    let body = r#"{
        "data": {
            "MediaListCollection": {
                "hasNextChunk": false,
                "lists": [
                    { "status": "CURRENT", "entries": [{ "mediaId": 1, "status": "CURRENT", "progress": 1, "score": 0.0, "updatedAt": 1, "repeat": 0, "media": { "idMal": null, "title": { "romaji": "A", "english": null, "userPreferred": null } } }] },
                    { "status": "PLANNING", "entries": [{ "mediaId": 2, "status": "PLANNING", "progress": 0, "score": 0.0, "updatedAt": 1, "repeat": 0, "media": { "idMal": null, "title": { "romaji": "B", "english": null, "userPreferred": null } } }] },
                    { "status": "COMPLETED", "entries": [{ "mediaId": 3, "status": "COMPLETED", "progress": 12, "score": 0.0, "updatedAt": 1, "repeat": 0, "media": { "idMal": null, "title": { "romaji": "C", "english": null, "userPreferred": null } } }] },
                    { "status": "PAUSED", "entries": [{ "mediaId": 4, "status": "PAUSED", "progress": 5, "score": 0.0, "updatedAt": 1, "repeat": 0, "media": { "idMal": null, "title": { "romaji": "D", "english": null, "userPreferred": null } } }] },
                    { "status": "DROPPED", "entries": [{ "mediaId": 5, "status": "DROPPED", "progress": 3, "score": 0.0, "updatedAt": 1, "repeat": 0, "media": { "idMal": null, "title": { "romaji": "E", "english": null, "userPreferred": null } } }] },
                    { "status": "REPEATING", "entries": [{ "mediaId": 6, "status": "REPEATING", "progress": 7, "score": 0.0, "updatedAt": 1, "repeat": 2, "media": { "idMal": null, "title": { "romaji": "F", "english": null, "userPreferred": null } } }] }
                ]
            }
        }
    }"#;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "variables": { "chunk": 1 }
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let provider = make_provider(&server.uri(), "http://unused-token");
    let entries = provider
        .list_all(&dummy_tokens())
        .await
        .expect("list_all ok");
    let by_id: std::collections::HashMap<u32, ListStatus> =
        entries.iter().map(|e| (e.media_id.0, e.status)).collect();
    assert_eq!(by_id.get(&1), Some(&ListStatus::Watching));
    assert_eq!(by_id.get(&2), Some(&ListStatus::Planning));
    assert_eq!(by_id.get(&3), Some(&ListStatus::Completed));
    assert_eq!(by_id.get(&4), Some(&ListStatus::Paused));
    assert_eq!(by_id.get(&5), Some(&ListStatus::Dropped));
    assert_eq!(by_id.get(&6), Some(&ListStatus::Rewatching));
}

#[tokio::test]
async fn list_all_drops_entries_with_unknown_status() {
    // AniList occasionally returns an empty status bucket on draft /
    // half-saved entries; the unified enum has no slot for that, so
    // the row should be skipped rather than panicking the rail
    // renderer. Pin the skip behaviour.
    let server = wiremock::MockServer::start().await;
    mount_viewer(&server).await;
    let body = r#"{
        "data": {
            "MediaListCollection": {
                "hasNextChunk": false,
                "lists": [{
                    "status": "WAT",
                    "entries": [
                        { "mediaId": 10, "status": "WAT", "progress": 0, "score": 0.0, "updatedAt": 1, "repeat": 0, "media": { "idMal": 10, "title": { "romaji": "Unknown", "english": null, "userPreferred": null } } },
                        { "mediaId": 11, "status": "COMPLETED", "progress": 24, "score": 0.0, "updatedAt": 1, "repeat": 0, "media": { "idMal": 11, "title": { "romaji": "Real", "english": null, "userPreferred": null } } }
                    ]
                }]
            }
        }
    }"#;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "variables": { "chunk": 1 }
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let provider = make_provider(&server.uri(), "http://unused-token");
    let entries = provider
        .list_all(&dummy_tokens())
        .await
        .expect("list_all ok");
    let ids: Vec<u32> = entries.iter().map(|e| e.media_id.0).collect();
    assert_eq!(ids, vec![11], "unknown status row dropped, real row kept");
}

/// Canonical `SaveMediaListEntry` response body — mirrors the shape
/// `list_all` reads so the same `parse_entry` helper handles both.
const SAVE_ENTRY_RESPONSE_BODY: &str = r#"{
    "data": {
        "SaveMediaListEntry": {
            "id": 999,
            "mediaId": 21,
            "status": "CURRENT",
            "progress": 1100,
            "score": 90,
            "updatedAt": 1735689600,
            "repeat": 0,
            "media": {
                "idMal": 21,
                "title": { "romaji": "One Piece", "english": null, "userPreferred": "One Piece" }
            }
        }
    }
}"#;

#[tokio::test]
#[ignore = "red; green commit lands the SaveMediaListEntry impl"]
async fn update_entry_posts_save_mutation_with_variables_and_returns_entry() {
    use wiremock::matchers::{body_string_contains, method};
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .and(body_string_contains("SaveMediaListEntry"))
        .and(body_string_contains("\"mediaId\":21"))
        .and(body_string_contains("\"progress\":1100"))
        .and(body_string_contains("\"status\":\"CURRENT\""))
        .and(body_string_contains("\"scoreRaw\":90"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(SAVE_ENTRY_RESPONSE_BODY),
        )
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let update = EntryUpdate {
        status: Some(ListStatus::Watching),
        progress_episodes: Some(1100),
        score_0_to_100: Some(90),
        repeat_count: None,
    };
    let entry = provider
        .update_entry(&dummy_tokens(), ProviderMediaId(21), update)
        .await
        .expect("update_entry ok");
    assert_eq!(entry.media_id.0, 21);
    assert_eq!(entry.status, ListStatus::Watching);
    assert_eq!(entry.progress_episodes, 1100);
    assert_eq!(entry.score_0_to_100, Some(90));
}

#[tokio::test]
#[ignore = "red; green commit unignores"]
async fn update_entry_surfaces_401_as_invalid_token() {
    use wiremock::matchers::method;
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let err = provider
        .update_entry(
            &dummy_tokens(),
            ProviderMediaId(21),
            EntryUpdate {
                progress_episodes: Some(1),
                ..Default::default()
            },
        )
        .await
        .expect_err("401 must surface");
    assert!(
        matches!(err, AniError::InvalidToken),
        "expected InvalidToken, got {err:?}"
    );
}

#[tokio::test]
#[ignore = "red; green commit lands the DeleteMediaListEntry two-step impl"]
async fn delete_entry_queries_for_id_then_calls_delete_mutation() {
    // AniList's DeleteMediaListEntry takes the MediaList row id, not
    // the mediaId. We first run a `MediaList(mediaId, userId)` query
    // to resolve the row id, then dispatch the delete mutation. Both
    // POST against the same GraphQL endpoint — wiremock can't
    // distinguish them by URL, so we register two mocks that match
    // on the query name in the body.
    use wiremock::matchers::{body_string_contains, method};
    let server = wiremock::MockServer::start().await;
    // Step 1: viewer + media-list-id lookup. Provider calls Viewer
    // first to get the user id, then MediaList for the entry id.
    wiremock::Mock::given(method("POST"))
        .and(body_string_contains("Viewer"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Viewer":{"id":4242,"name":"shiro","avatar":{"large":null,"medium":null},"statistics":{"anime":{"count":0,"meanScore":0}}}}}"#,
        ))
        .mount(&server)
        .await;
    wiremock::Mock::given(method("POST"))
        .and(body_string_contains("MediaList("))
        .and(body_string_contains("\"mediaId\":21"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"MediaList":{"id":7777}}}"#),
        )
        .mount(&server)
        .await;
    // Step 2: the actual delete.
    wiremock::Mock::given(method("POST"))
        .and(body_string_contains("DeleteMediaListEntry"))
        .and(body_string_contains("\"id\":7777"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"DeleteMediaListEntry":{"deleted":true}}}"#),
        )
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    provider
        .delete_entry(&dummy_tokens(), ProviderMediaId(21))
        .await
        .expect("delete_entry ok");
}

#[tokio::test]
#[ignore = "red; green commit unignores"]
async fn delete_entry_surfaces_401_as_invalid_token() {
    use wiremock::matchers::method;
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let provider = make_provider(&server.uri(), "http://unused-token");
    let err = provider
        .delete_entry(&dummy_tokens(), ProviderMediaId(21))
        .await
        .expect_err("401 must surface");
    assert!(
        matches!(err, AniError::InvalidToken),
        "expected InvalidToken, got {err:?}"
    );
}

#[tokio::test]
async fn refresh_returns_metadata_error_no_network_call() {
    // AniList does not issue refresh tokens — their 1-year JWT has
    // no refresh flow and no revocation endpoint. The trait method
    // must surface this as AniError::Metadata so the route layer can
    // distinguish "no flow" from a transient upstream / network
    // failure (which would be Network / Upstream).
    //
    // Build the provider with bogus endpoints to also prove the impl
    // doesn't attempt a network round-trip — any HTTP call against
    // these unbound URIs would surface as Network, not Metadata.
    let provider = AniListProvider::with_bases(
        reqwest::Client::new(),
        "http://127.0.0.1:1/no-api".into(),
        "http://127.0.0.1:1/no-token".into(),
    );
    let err = provider
        .refresh("never-issued")
        .await
        .expect_err("refresh must always error");
    assert!(
        matches!(err, AniError::Metadata),
        "expected AniError::Metadata, got {err:?}"
    );
}
