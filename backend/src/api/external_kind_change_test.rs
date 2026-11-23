//! `GET /api/external-player/after-kind-change?command=&from=&to=`
//! answers what Settings saves as the external player's command when
//! the user changes the player's type
//! ([`crate::commands::external_player::command_after_kind_change`]).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::TempDir;
use tower::ServiceExt;

use super::test_router;
use super::tests::{body_string, test_app_state};

async fn ask(query: &str) -> (StatusCode, String) {
    let td = TempDir::new().expect("tempdir");
    let router = test_router(Arc::new(test_app_state(&td)));
    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/external-player/after-kind-change?{query}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    let status = response.status();
    (status, body_string(response).await)
}

#[tokio::test]
async fn the_previous_types_bare_command_follows_the_type() {
    let (status, body) = ask("command=mpv&from=mpv&to=vlc").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "\"vlc\"");
}

#[tokio::test]
async fn a_command_the_user_wrote_comes_back_as_it_was_with_its_spaces() {
    let (status, body) = ask("command=%2Fopt%2Fmy%20mpv%2Fmpv%20--fs&from=mpv&to=iina").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "\"/opt/my mpv/mpv --fs\"");
}

#[tokio::test]
async fn a_blank_command_is_answered() {
    let (status, body) = ask("command=&from=custom&to=mpv").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "\"\"");
}

#[tokio::test]
async fn an_unknown_type_is_refused() {
    let (status, _) = ask("command=mpv&from=mpv&to=winamp").await;
    assert!(status.is_client_error(), "{status}");
}
