//! The availability warm's intake: what a list view hands the
//! backend to probe in the background.

use std::sync::Arc;

use super::availability::{warm, AvailabilityArgs};
use crate::app::AppState;

/// The warm items handed in and not yet walked.
#[derive(Clone, Default)]
pub struct WarmQueue;

impl WarmQueue {
    /// Hand `items` to the warm. Returns at once.
    pub fn push(&self, state: &Arc<AppState>, items: Vec<AvailabilityArgs>) {
        let state = Arc::clone(state);
        tokio::spawn(async move {
            warm(state, items).await;
        });
    }
}
