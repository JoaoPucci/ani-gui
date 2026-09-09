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

    /// The walk is about to return a miss, and `provider` is the one
    /// whose miss it is — not necessarily the last one asked: a
    /// skipped provider's half-open trial that fails leaves the saved
    /// miss standing, and the saved miss keeps its author.
    fn missed_by(&mut self, provider: ProviderId);
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
/// A provider that is refusing — its breaker open, or an advertised
/// rate-limit window still running — is skipped while another
/// remains; the last is always tried. Skipping a pausing provider is
/// the same call for both priorities: an interactive attempt would
/// only be told to come back later, and a background one would wait
/// inside the gate for the window, its attempt budget spent before
/// the fallback is asked. Every attempt but the last is bounded by
/// `attempt_budget`, and all of them together by `total_budget`. Each
/// attempt's outcome is recorded on its provider's gate, timestamped
/// with the instant that observed it. A miss is reported as the
/// provider gave it, a clean miss included: whose verdict it is, and
/// while whom it may be served, is the availability cache's rule,
/// which names the provider on the row.
///
/// `remembered` names the provider a positive availability row put
/// first. Its answered miss is not the walk's verdict — the row
/// proves the show and its mode at the show's level, not that every
/// episode has an embed — so the walk goes on to the rest of the
/// order, and the miss stands, as given, only when the rest were
/// unreachable.
///
/// On an interactive walk the gate admits a click through an open
/// breaker as its half-open trial, so the skip is only the fast
/// path: when every provider that was tried answered a miss or was
/// unreachable, the skipped ones are asked before the miss or the
/// error surfaces — a skipped provider may have recovered, and its
/// answer or its miss is then the walk's. Background traffic keeps
/// the skip.
///
/// # Errors
/// The first answer that is not a failover — a miss — or, when no
/// provider answered, the first unreachable error: the primary's
/// when it was tried.
#[allow(clippy::too_many_arguments)]
pub async fn with_failover<'c, 'g, A, C, G>(
    order: &[ProviderId],
    remembered: Option<ProviderId>,
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
    let mut walk = Walk {
        first_unreachable: None,
        any_unreachable: false,
        skipped: Vec::new(),
    };
    let mut affinity_miss: Option<(NativeError, ProviderId)> = None;
    let count = order.len();
    for (i, &provider) in order.iter().enumerate() {
        let last = i + 1 == count;
        if !last && gate_of(provider).is_refusing() {
            walk.any_unreachable = true;
            walk.skipped.push(provider);
            continue;
        }
        // The skipped providers are owed their trial on an interactive
        // walk, so an attempt budget stays in reserve for each of them;
        // a background walk never retries and reserves nothing.
        let owed = if matches!(priority, ScrapePriority::Interactive) {
            walk.skipped.len()
        } else {
            0
        };
        let Some(budget) = walk_budget(overall, total_budget, attempt_budget, last, owed) else {
            break;
        };
        match try_provider(
            provider,
            budget,
            priority,
            &mut client_for,
            &gate_of,
            attempt,
            &mut walk,
        )
        .await
        {
            Tried::Answered(answer) => return Ok(answer),
            Tried::FailedOver => {}
            Tried::Missed(miss, by) => {
                if i == 0 && remembered == Some(provider) {
                    affinity_miss = Some((miss, by));
                    continue;
                }
                let verdict = match affinity_miss.take() {
                    Some((set_aside, set_by)) => {
                        firmer_verdict((set_aside, Some(set_by)), (miss, Some(by)))
                    }
                    None => (miss, Some(by)),
                };
                return retry_skipped(
                    verdict,
                    overall,
                    total_budget,
                    attempt_budget,
                    priority,
                    &mut client_for,
                    &gate_of,
                    attempt,
                    &mut walk,
                )
                .await;
            }
        }
    }
    // Nothing answered: the remembered provider's miss set aside, or
    // the first unreachable error. Either way the skipped providers
    // get their trial before it surfaces.
    let verdict = affinity_miss
        .map(|(miss, by)| (miss, Some(by)))
        .or_else(|| walk.first_unreachable.take().map(|e| (e, None)))
        .unwrap_or((
            NativeError {
                error: AniError::Network,
                clean_miss: false,
                failed_at: None,
            },
            None,
        ));
    retry_skipped(
        verdict,
        overall,
        total_budget,
        attempt_budget,
        priority,
        &mut client_for,
        &gate_of,
        attempt,
        &mut walk,
    )
    .await
}

/// What a walk has learned so far.
struct Walk {
    /// The first unreachable error, to surface when nobody answers.
    first_unreachable: Option<NativeError>,
    /// Whether any provider so far was unreachable, refusing, broken
    /// or skipped.
    any_unreachable: bool,
    /// The providers skipped for refusing — an open breaker or a
    /// running pause — in order.
    skipped: Vec<ProviderId>,
}

/// How one attempt ended: an answer, a failover, or a miss with the
/// provider whose miss it is.
enum Tried<'c, T> {
    Answered(Attempted<'c, T>),
    FailedOver,
    Missed(NativeError, ProviderId),
}

/// The budget for the next attempt. The last provider with nobody
/// still owed an attempt gets the whole remainder; every other
/// attempt gets the attempt budget, out of what is left once one
/// attempt budget per provider still owed (`owed`) is held back —
/// a stalled attempt must not eat the trial a skipped provider is
/// due. None once the total, or what is left after the reserve, is
/// spent.
fn walk_budget(
    overall: tokio::time::Instant,
    total_budget: Duration,
    attempt_budget: Duration,
    last: bool,
    owed: usize,
) -> Option<Duration> {
    let remaining = total_budget.saturating_sub(overall.elapsed());
    if remaining.is_zero() {
        return None;
    }
    if last && owed == 0 {
        return Some(remaining);
    }
    let reserve = attempt_budget.saturating_mul(u32::try_from(owed).unwrap_or(u32::MAX));
    let free = remaining.saturating_sub(reserve);
    if free.is_zero() {
        return None;
    }
    Some(attempt_budget.min(free))
}

/// One bounded attempt against `provider`, its outcome recorded on
/// the provider's gate.
async fn try_provider<'c, 'g, A, C, G>(
    provider: ProviderId,
    budget: Duration,
    priority: ScrapePriority,
    client_for: &mut C,
    gate_of: &G,
    attempt: &mut A,
    walk: &mut Walk,
) -> Tried<'c, A::Output>
where
    A: Attempt,
    C: FnMut(ProviderId) -> crate::error::Result<Box<dyn Provider + 'c>>,
    G: Fn(ProviderId) -> &'g ScraperGate,
{
    let client = match client_for(provider) {
        Ok(c) => c,
        Err(error) => {
            walk.any_unreachable = true;
            walk.first_unreachable.get_or_insert(NativeError {
                error,
                clean_miss: false,
                failed_at: None,
            });
            return Tried::FailedOver;
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
        Ok(value) => Tried::Answered(Attempted {
            provider,
            value,
            client,
        }),
        Err(ne) if fails_over(&ne.error) => {
            walk.any_unreachable = true;
            walk.first_unreachable.get_or_insert(ne);
            Tried::FailedOver
        }
        Err(ne) => Tried::Missed(ne, provider),
    }
}

/// The walk's verdict so far — a miss, or the first unreachable
/// error when nothing answered — stands, unless providers were
/// skipped for refusing on an interactive walk, which the gate would
/// have admitted anyway — an open breaker's half-open trial, a pause
/// it ignores for a click. Those
/// are asked now: an answer is the walk's, a miss of theirs — the
/// last answer given — replaces the verdict they were asked for,
/// author and all, and one unreachable too leaves it standing. The
/// attempt is told whose miss the verdict is before it surfaces.
///
/// # Errors
/// The verdict that stands.
#[allow(clippy::too_many_arguments)]
async fn retry_skipped<'c, 'g, A, C, G>(
    verdict: (NativeError, Option<ProviderId>),
    overall: tokio::time::Instant,
    total_budget: Duration,
    attempt_budget: Duration,
    priority: ScrapePriority,
    client_for: &mut C,
    gate_of: &G,
    attempt: &mut A,
    walk: &mut Walk,
) -> Result<Attempted<'c, A::Output>, NativeError>
where
    A: Attempt,
    C: FnMut(ProviderId) -> crate::error::Result<Box<dyn Provider + 'c>>,
    G: Fn(ProviderId) -> &'g ScraperGate,
{
    let mut verdict = verdict;
    if matches!(priority, ScrapePriority::Interactive) && !walk.skipped.is_empty() {
        let skipped = std::mem::take(&mut walk.skipped);
        let count = skipped.len();
        for (i, provider) in skipped.into_iter().enumerate() {
            // Each retry after this one is still owed its attempt.
            let owed = count - i - 1;
            let Some(budget) = walk_budget(overall, total_budget, attempt_budget, owed == 0, owed)
            else {
                break;
            };
            match try_provider(
                provider, budget, priority, client_for, gate_of, attempt, walk,
            )
            .await
            {
                Tried::Answered(answer) => return Ok(answer),
                Tried::FailedOver => {}
                Tried::Missed(ne, by) => verdict = firmer_verdict(verdict, (ne, Some(by))),
            }
        }
    }
    let (error, by) = verdict;
    if let Some(by) = by {
        attempt.missed_by(by);
    }
    Err(error)
}

/// Whether the earlier of two misses is the one to keep. An episode
/// verdict outranks a title miss: a provider that found the show and
/// not the episode has said something a later title miss cannot
/// unsay — the later provider lacks the show, which says nothing
/// about the episode. Between two verdicts of the same kind the later
/// one stands, as it did before.
fn keeps_earlier(earlier: &NativeError, later: &NativeError) -> bool {
    let episode = |ne: &NativeError| matches!(ne.error, AniError::EpisodeUnavailable);
    episode(earlier) && !episode(later)
}

/// The verdict to keep when two providers both missed, with the
/// provider whose verdict it is, so the provider travels with the
/// verdict that is kept — the row a miss writes names the provider
/// that missed, not the last one asked.
fn firmer_verdict(
    earlier: (NativeError, Option<ProviderId>),
    later: (NativeError, Option<ProviderId>),
) -> (NativeError, Option<ProviderId>) {
    if keeps_earlier(&earlier.0, &later.0) {
        earlier
    } else {
        later
    }
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
    run_at(
        state,
        Origins::of(state),
        &state.provider_order,
        None,
        priority,
        attempt,
    )
    .await
}

/// [`run`] starting from `remembered` — the provider a positive
/// availability row named — when the state lists it. A show the
/// fallback proved playable during a primary outage stays playable
/// after the primary recovers: its clean miss would otherwise end
/// the walk on a show the row says is there.
///
/// # Errors
/// As [`with_failover`].
pub async fn run_from<'a, A: Attempt>(
    state: &'a AppState,
    remembered: Option<ProviderId>,
    priority: ScrapePriority,
    attempt: &mut A,
) -> Result<Attempted<'a, A::Output>, NativeError> {
    let order = order_with_affinity(&state.provider_order, remembered);
    let remembered = remembered.filter(|r| order.first() == Some(r));
    run_at(
        state,
        Origins::of(state),
        &order,
        remembered,
        priority,
        attempt,
    )
    .await
}

/// `order` with `remembered` moved to the front when it is listed;
/// the rest keep their places. A provider the state does not list is
/// not asked on a row's say-so.
#[must_use]
pub fn order_with_affinity(
    order: &[ProviderId],
    remembered: Option<ProviderId>,
) -> Vec<ProviderId> {
    match remembered {
        Some(first) if order.contains(&first) => std::iter::once(first)
            .chain(order.iter().copied().filter(|p| *p != first))
            .collect(),
        _ => order.to_vec(),
    }
}

/// [`run`] with the providers' origins and order named by the caller,
/// and the provider a positive row put first, when one did.
///
/// # Errors
/// As [`with_failover`].
pub async fn run_at<'a, A: Attempt>(
    state: &'a AppState,
    origins: Origins<'_>,
    order: &[ProviderId],
    remembered: Option<ProviderId>,
    priority: ScrapePriority,
    attempt: &mut A,
) -> Result<Attempted<'a, A::Output>, NativeError> {
    with_failover(
        order,
        remembered,
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
    /// The provider whose miss the walk returned, for the negative
    /// row to name — set by the walk, which knows whose verdict
    /// survived, not by the attempt, which only knows who was asked
    /// last.
    pub answered_by: Option<ProviderId>,
}

#[async_trait::async_trait]
impl<F: FnMut(ProgressLine) + Send> Attempt for ResolveAttempt<'_, F> {
    type Output = NativeResolved;

    async fn run(&mut self, provider: &dyn Provider) -> Result<NativeResolved, NativeError> {
        resolve_native(provider, self.request, self.on_progress).await
    }

    fn missed_by(&mut self, provider: ProviderId) {
        self.answered_by = Some(provider);
    }
}

#[cfg(test)]
#[path = "providers_test.rs"]
mod tests;
