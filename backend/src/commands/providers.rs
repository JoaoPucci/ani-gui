//! Which provider a walk runs against, and what happens when it
//! cannot: the failover orchestrator.
//!
//! A walk — the play resolve, the availability probe, the download's
//! pick — is written for any [`Provider`]. This module decides which
//! one it runs against: the providers in order, moving to the next
//! only when the current one was unreachable, refusing, or broken,
//! never when it answered. Every attempt is bounded and attributed:
//! the primary gets [`PRIMARY_ATTEMPT_BUDGET`] so a stalled outage
//! cannot spend the whole deadline before the fallback is asked, and
//! each attempt's outcome lands on its own provider's breaker.

use std::time::Duration;

use crate::app::AppState;
use crate::commands::play_native_outcome::breaker_outcome;
use crate::commands::play_native_resolve::{
    resolve_native, NativeError, NativeResolveRequest, NativeResolved, RESOLVE_DEADLINE,
};
use crate::commands::progress::ProgressLine;
use crate::error::AniError;
use crate::scraper::gate::{ScrapePriority, ScraperGate};
use crate::scraper::provider::{Provider, ProviderId};

/// The budget a provider gets when another remains behind it. Well
/// under the resolve deadline, so the fallback still has most of it.
pub const PRIMARY_ATTEMPT_BUDGET: Duration = Duration::from_secs(20);

/// One walk, shaped for whichever provider it is run against.
#[async_trait::async_trait]
pub trait Attempt: Send {
    /// What the walk produces.
    type Output: Send;

    /// Run the walk against `provider`.
    ///
    /// # Errors
    /// The walk's own verdicts, as [`NativeError`].
    async fn run(&mut self, provider: &dyn Provider) -> Result<Self::Output, NativeError>;
}

/// What an attempt produced, who produced it, and the client that
/// did — a caller that continues the walk (a ranged download
/// resolving its episodes) continues against the same provider.
pub struct Attempted<'a, T> {
    /// The provider that answered.
    pub provider: ProviderId,
    /// The walk's output.
    pub value: T,
    /// The client the answer came from.
    pub client: Box<dyn Provider + 'a>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for Attempted<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Attempted")
            .field("provider", &self.provider)
            .field("value", &self.value)
            .finish_non_exhaustive()
    }
}

/// Whether a failed attempt is the provider being unreachable,
/// refusing, or broken — the shapes that send the walk to the next
/// provider — as opposed to an answer. A parse failure is the site
/// having changed shape: broken, not answering.
#[must_use]
pub fn fails_over(error: &AniError) -> bool {
    matches!(
        error,
        AniError::Network
            | AniError::Timeout
            | AniError::GateRefused
            | AniError::RateLimited { .. }
            | AniError::ParseFailed { .. }
    ) || error.is_provider_block()
}

/// Run `attempt` against `order`'s providers until one answers.
///
/// A provider whose breaker is open is skipped while another remains;
/// the last is always tried. Every attempt but the last is bounded by
/// `attempt_budget`, and all of them together by `total_budget`. Each
/// attempt's outcome is recorded on its provider's gate, timestamped
/// with the instant that observed it. A clean miss reached after an
/// unreachable provider is demoted to a plain miss: absence on the
/// fallback proves nothing about a primary that never answered.
///
/// # Errors
/// The first answer that is not a failover — a miss — or, when no
/// provider answered, the first unreachable error: the primary's.
pub async fn with_failover<'c, 'g, A, C, G>(
    order: &[ProviderId],
    priority: ScrapePriority,
    total_budget: Duration,
    attempt_budget: Duration,
    mut client_for: C,
    gate_of: G,
    attempt: &mut A,
) -> Result<Attempted<'c, A::Output>, NativeError>
where
    A: Attempt,
    C: FnMut(ProviderId) -> crate::error::Result<Box<dyn Provider + 'c>>,
    G: Fn(ProviderId) -> &'g ScraperGate,
{
    let overall = tokio::time::Instant::now();
    let mut first_unreachable: Option<NativeError> = None;
    let mut any_unreachable = false;
    let count = order.len();
    for (i, &provider) in order.iter().enumerate() {
        let last = i + 1 == count;
        if !last && gate_of(provider).is_open() {
            any_unreachable = true;
            continue;
        }
        let remaining = total_budget.saturating_sub(overall.elapsed());
        if remaining.is_zero() {
            break;
        }
        let budget = if last {
            remaining
        } else {
            attempt_budget.min(remaining)
        };
        let client = match client_for(provider) {
            Ok(c) => c,
            Err(error) => {
                any_unreachable = true;
                first_unreachable.get_or_insert(NativeError {
                    error,
                    clean_miss: false,
                    failed_at: None,
                });
                continue;
            }
        };
        let started = tokio::time::Instant::now();
        let result = match tokio::time::timeout(budget, attempt.run(&*client)).await {
            Ok(result) => result,
            Err(_elapsed) => Err(NativeError {
                error: AniError::Timeout,
                clean_miss: false,
                failed_at: None,
            }),
        };
        if let Some(outcome) = breaker_outcome(priority, &result) {
            let observed_at = result
                .as_ref()
                .err()
                .and_then(|ne| ne.failed_at)
                .or_else(|| client.last_attempt_at())
                .unwrap_or(started);
            gate_of(provider).record(outcome, observed_at);
        }
        match result {
            Ok(value) => {
                return Ok(Attempted {
                    provider,
                    value,
                    client,
                })
            }
            Err(ne) if fails_over(&ne.error) => {
                any_unreachable = true;
                first_unreachable.get_or_insert(ne);
            }
            Err(mut ne) => {
                if ne.clean_miss && any_unreachable {
                    ne.clean_miss = false;
                }
                return Err(ne);
            }
        }
    }
    Err(first_unreachable.unwrap_or(NativeError {
        error: AniError::Network,
        clean_miss: false,
        failed_at: None,
    }))
}

/// Where each provider's client points: the state's overrides, or a
/// caller's own (the availability command's test seam).
#[derive(Debug, Clone, Copy, Default)]
pub struct Origins<'a> {
    /// The anidb origin, when not the real site.
    pub anidb: Option<&'a str>,
    /// The hianime origin, when not the real site.
    pub hianime: Option<&'a str>,
}

impl<'a> Origins<'a> {
    /// The state's overrides — `None` for both in production.
    #[must_use]
    pub fn of(state: &'a AppState) -> Self {
        Self {
            anidb: state.anidb_base.as_deref(),
            hianime: state.hianime_base.as_deref(),
        }
    }
}

/// The gate `provider`'s traffic is admitted through.
#[must_use]
pub fn gate_of(state: &AppState, provider: ProviderId) -> &ScraperGate {
    match provider {
        ProviderId::Anidb => &state.anidb_gate,
        ProviderId::Hianime => &state.hianime_gate,
    }
}

/// The production client for `provider`: the curl-impersonate
/// transport resolved through the bundled directory then PATH,
/// pointed at the site (or its override in `origins`), with every
/// request admitted through the provider's own gate at `priority` —
/// the walk fans out into candidate probes and the episode chain,
/// and each of those is a provider request the pacing contract
/// covers, not just the search.
///
/// # Errors
/// [`AniError::Network`] when no curl binary resolves at all — the
/// host cannot reach any provider by any transport.
pub fn client_for<'a>(
    state: &'a AppState,
    origins: Origins<'_>,
    provider: ProviderId,
    priority: ScrapePriority,
) -> crate::error::Result<Box<dyn Provider + 'a>> {
    let path_env = std::env::var("PATH").unwrap_or_default();
    let fetch = crate::scraper::fetch::CurlImpersonateFetch::resolve(
        state.bundled_bin.as_deref(),
        &path_env,
    )
    .ok_or_else(|| {
        tracing::error!("no curl binary found for the provider transport");
        AniError::Network
    })?;
    let fetch =
        crate::scraper::gated::GatedFetch::new(fetch, Some(gate_of(state, provider)), priority);
    Ok(match provider {
        ProviderId::Anidb => match origins.anidb {
            Some(base) => Box::new(crate::scraper::anidb::AnidbClient::with_base(fetch, base)),
            None => Box::new(crate::scraper::anidb::AnidbClient::new(fetch)),
        },
        ProviderId::Hianime => match origins.hianime {
            Some(base) => Box::new(crate::scraper::hianime::HianimeClient::with_base(
                fetch, base,
            )),
            None => Box::new(crate::scraper::hianime::HianimeClient::new(fetch)),
        },
    })
}

/// Run `attempt` against the state's providers in order, under the
/// resolve deadline, each on its own gate. See [`with_failover`].
///
/// # Errors
/// As [`with_failover`].
pub async fn run<'a, A: Attempt>(
    state: &'a AppState,
    priority: ScrapePriority,
    attempt: &mut A,
) -> Result<Attempted<'a, A::Output>, NativeError> {
    run_at(state, Origins::of(state), priority, attempt).await
}

/// [`run`] with the providers' origins named by the caller.
///
/// # Errors
/// As [`with_failover`].
pub async fn run_at<'a, A: Attempt>(
    state: &'a AppState,
    origins: Origins<'_>,
    priority: ScrapePriority,
    attempt: &mut A,
) -> Result<Attempted<'a, A::Output>, NativeError> {
    with_failover(
        &state.provider_order,
        priority,
        RESOLVE_DEADLINE,
        PRIMARY_ATTEMPT_BUDGET,
        |provider| client_for(state, origins, provider, priority),
        |provider| gate_of(state, provider),
        attempt,
    )
    .await
}

/// The play resolve as an attempt: alias walk, bounded probing,
/// episode-to-master resolution, reporting progress as it goes.
pub struct ResolveAttempt<'r, F> {
    /// What to resolve.
    pub request: NativeResolveRequest<'r>,
    /// Where the walk's progress lines go.
    pub on_progress: &'r mut F,
}

#[async_trait::async_trait]
impl<F: FnMut(ProgressLine) + Send> Attempt for ResolveAttempt<'_, F> {
    type Output = NativeResolved;

    async fn run(&mut self, provider: &dyn Provider) -> Result<NativeResolved, NativeError> {
        resolve_native(provider, self.request, self.on_progress).await
    }
}

#[cfg(test)]
#[path = "providers_test.rs"]
mod tests;
