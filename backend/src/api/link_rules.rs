//! The change-show flow's rules, asked by the renderer: the picker's
//! candidates, the line each Continue card stands for, and whether a
//! surface asks about a card's line. Each is a thin handler over its
//! command.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;

use crate::app::AppState;
use crate::commands::card_line::{card_lines, CardLine, CardRow};
use crate::commands::link_candidates::{link_candidates, CandidatesArgs};
use crate::commands::link_rules::LinkCandidate;
use crate::commands::link_verdict::{verdict, Verdict, VerdictArgs};
use crate::error::AniError;

/// The candidates for a guessed row
/// ([`crate::commands::link_candidates::link_candidates`]).
pub(super) async fn post_link_candidates(
    State(state): State<Arc<AppState>>,
    Json(args): Json<CandidatesArgs>,
) -> Result<Json<Vec<LinkCandidate>>, AniError> {
    Ok(Json(link_candidates(&state, &args).await?))
}

/// The rows Continue Watching shows, as it matches them.
#[derive(Debug, Deserialize)]
pub(super) struct CardLinesArgs {
    rows: Vec<CardRow>,
}

/// The line each card stands for ([`crate::commands::card_line`]).
pub(super) async fn post_card_lines(
    Json(args): Json<CardLinesArgs>,
) -> Json<HashMap<String, CardLine>> {
    Json(card_lines(&args.rows))
}

/// Whether a surface asks about a card's line
/// ([`crate::commands::link_verdict::verdict`]).
pub(super) async fn post_link_verdict(Json(args): Json<VerdictArgs>) -> Json<Verdict> {
    Json(verdict(&args))
}

#[cfg(test)]
#[path = "link_rules_test.rs"]
mod tests;
