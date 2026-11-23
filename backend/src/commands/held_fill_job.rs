//! One held-back fill's retry ([`super`]): wait out the pause, take the
//! endpoint's retry turn, run the fill again, and judge by the cache
//! whether it landed. Split from the registry and the routes' side so
//! each file stays inside the complexity bar.

use std::time::Duration;

use tokio::time::Instant;

use super::registry::{after_run, Step, MAX_RETRIES};
use super::Fill;
use crate::app::AppState;
use crate::cache::meta_cache_get;
use crate::meta::anilist_gate::retry_wait;

/// How long a retry may wait for room before it gives up: a session
/// busy enough to keep the window full this long has moved on, and the
/// next view asks again.
const GIVE_UP_AFTER: Duration = Duration::from_secs(300);

/// A held-back fill, owned for its retry.
pub(super) struct Job {
    pub(super) state: AppState,
    pub(super) fill: Fill,
    pub(super) kitsu_id: String,
    pub(super) anilist_base: Option<String>,
    /// The ticket pages wait on.
    pub(super) ticket: u64,
}

impl Job {
    fn url(&self) -> &str {
        super::url(self.anilist_base.as_deref())
    }

    /// Whether the fill is in the cache: the only place a fill keeps a
    /// real answer, and the only one it keeps.
    fn landed(&self) -> bool {
        let key = match self.fill {
            Fill::Thumbs => {
                crate::commands::anilist_eps_thumbs::anilist_eps_thumbs_key(&self.kitsu_id)
            }
            Fill::Detail => crate::commands::kitsu::anime_detail_key(&self.kitsu_id),
        };
        matches!(meta_cache_get(&self.state.cache_pool, &key), Ok(Some(_)))
    }

    /// Run the fill once more, as the read that was held back ran it.
    /// Returns whether the gate or AniList held it back again.
    async fn run(&self) -> bool {
        let base = self.anilist_base.as_deref();
        let run = async {
            match self.fill {
                Fill::Thumbs => {
                    let _ = crate::commands::anilist_eps_thumbs::thumbs_for_show_with_anilist_base(
                        &self.state,
                        &self.kitsu_id,
                        base,
                    )
                    .await;
                }
                Fill::Detail => {
                    let _ = crate::commands::kitsu::kitsu_anime_detail_with_anilist_base(
                        &self.state,
                        &self.kitsu_id,
                        base,
                    )
                    .await;
                }
            }
        };
        let run = async {
            run.await;
            super::RETRYING.with(std::cell::Cell::get)
        };
        crate::meta::anilist_gate::as_retry(super::RETRYING.scope(std::cell::Cell::new(false), run))
            .await
    }

    /// Retry until the fill lands, is held back past [`MAX_RETRIES`]
    /// pauses, fails for another reason, or waits [`GIVE_UP_AFTER`] for
    /// its turn. Returns whether it landed.
    pub(super) async fn retry(&self) -> bool {
        let deadline = Instant::now() + GIVE_UP_AFTER;
        let turn = super::turn(self.url());
        for attempt in 1..=MAX_RETRIES {
            // A view since the hold, or since the last run, may have
            // filled it.
            if self.landed() {
                return true;
            }
            // The endpoint's one retry turn, taken only when this retry
            // ranks to go and the gate has room for it; held while it
            // runs.
            let Some(_turn) = self.my_turn(&turn, deadline).await else {
                return false;
            };
            // A view while it waited for its turn may have filled it.
            if self.landed() {
                return true;
            }
            tracing::info!(
                provider = "anilist",
                fill = ?self.fill,
                kitsu_id = %self.kitsu_id,
                ticket = self.ticket,
                attempt,
                on_screen = super::registry().on_screen(self.ticket, std::time::Instant::now()),
                "retrying a held-back fill"
            );
            // Held back again — by a pause, or by its rank behind a
            // request of another class — as the run itself reports.
            let held = self.run().await;
            match after_run(self.landed(), held, attempt) {
                Step::Landed => return true,
                Step::Again => {}
                Step::GiveUp => return false,
            }
        }
        false
    }

    /// Wait until this retry may run: it ranks to go (a fill on screen,
    /// or none on screen waits), the gate would let a retry through, and
    /// no other retry runs. `None` past `deadline`.
    async fn my_turn<'a>(
        &self,
        turn: &'a tokio::sync::Mutex<()>,
        deadline: Instant,
    ) -> Option<tokio::sync::MutexGuard<'a, ()>> {
        loop {
            let ranks = super::registry().may_go(self.ticket, std::time::Instant::now());
            let wait = retry_wait(self.url());
            if ranks && wait.is_zero() {
                if let Ok(guard) = turn.try_lock() {
                    return Some(guard);
                }
            }
            let nap = wait.clamp(MIN_NAP, MAX_NAP);
            if Instant::now() + nap > deadline {
                return None;
            }
            tokio::time::sleep(nap).await;
        }
    }
}

/// How often a waiting retry looks again, at least and at most: its
/// rank and the gate's room change without telling it.
const MIN_NAP: Duration = Duration::from_millis(100);
const MAX_NAP: Duration = Duration::from_secs(1);
