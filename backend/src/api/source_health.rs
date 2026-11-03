//! Axum handler for `GET /api/source-health`: whether each streaming
//! provider is answering, from the outcomes its gate has recorded (see
//! `crate::scraper::health`). A read of what the app already saw; it
//! sends nothing to any provider.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::app::AppState;
use crate::scraper::health::Health;
use crate::scraper::provider::ProviderId;

#[derive(Serialize)]
pub(super) struct ProviderHealth {
    provider: ProviderId,
    #[serde(flatten)]
    health: Health,
}

#[derive(Serialize)]
pub(super) struct SourceHealthResponse {
    /// One entry per provider a walk asks, in that order.
    providers: Vec<ProviderHealth>,
}

pub(super) async fn get_source_health(
    State(state): State<Arc<AppState>>,
) -> Json<SourceHealthResponse> {
    let providers = state
        .providers
        .order()
        .iter()
        .map(|&provider| ProviderHealth {
            provider,
            health: state.providers.gate(provider).health(),
        })
        .collect();
    Json(SourceHealthResponse { providers })
}
