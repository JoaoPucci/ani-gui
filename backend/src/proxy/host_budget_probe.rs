//! A cached row's liveness probes: a class of their own between the
//! player's requests and background traffic. A probe takes a token only
//! while no player request waits in the host's line, so an episode
//! already playing there never has its next segment queued behind a
//! row's probes; and background traffic yields to a probe while it
//! waits, so downloads never starve it.

use super::HostBudget;

/// Counts a probe as waiting at its host for as long as it waits, so
/// background traffic there yields to it; a probe dropped while it
/// waits — its check's deadline — stops counting.
struct ProbeWaiting<'a> {
    budget: &'a HostBudget,
    host: &'a str,
}

impl<'a> ProbeWaiting<'a> {
    fn new(budget: &'a HostBudget, host: &'a str) -> Self {
        *budget
            .probes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(host.to_owned())
            .or_default() += 1;
        Self { budget, host }
    }
}

impl Drop for ProbeWaiting<'_> {
    fn drop(&mut self) {
        let mut probes = self.budget.probes.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(waiting) = probes.get_mut(self.host) {
            *waiting = waiting.saturating_sub(1);
            if *waiting == 0 {
                probes.remove(self.host);
            }
        }
    }
}

impl HostBudget {
    /// A token for `host` for a cached row's liveness probe: taken only
    /// while no player request waits in the host's line, and otherwise
    /// looked for again a refill later; background traffic at the host
    /// waits while the probe does.
    pub(crate) async fn admit_probe(&self, host: &str) {
        let line = self.line(host);
        let _waiting = ProbeWaiting::new(self, host);
        loop {
            let wait = match line.try_lock() {
                Ok(_nobody_waiting) => match self.take_for(host) {
                    None => return,
                    Some(wait) => wait,
                },
                Err(_player_waiting) => self.refill,
            };
            tokio::time::sleep(wait).await;
        }
    }

    /// Whether a cached row's probe waits at `host`.
    pub(super) fn probes_waiting(&self, host: &str) -> bool {
        self.probes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(host)
            .is_some_and(|&waiting| waiting > 0)
    }
}
