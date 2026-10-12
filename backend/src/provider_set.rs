//! The providers an app state walks, as one value: their order, one
//! admission gate each, and where each one's client points.
//!
//! Everything a walk needs to know about *which* providers exist
//! lives here rather than as a field per provider on `AppState`, so
//! adding a provider never adds a field that every constructor in the
//! tree has to name. It is:
//!
//! - a variant on [`ProviderId`] with its label, and its entry in
//!   [`ProviderId::ALL`] — gates are built from `ALL`, so a variant
//!   left out of it has none, and [`ProviderSet::gate`] panics on it.
//!   `provider_set_test.rs` matches every variant without a wildcard,
//!   which stops a new variant compiling until whoever adds it says
//!   there which list it belongs in;
//! - its place in [`ProviderSet::shipped`]'s order;
//! - a client behind the [`Provider`](crate::scraper::provider::Provider)
//!   trait, and its arm in
//!   [`client_for`](crate::commands::providers::client_for). The
//!   requests it builds from its own site are held to that origin
//!   ([`FetchRequest::held_to_origin`](crate::scraper::fetch::FetchRequest::held_to_origin)),
//!   and an answer from elsewhere is refused, as hianime's client does;
//! - its label in the renderer's list, `LABELED` in
//!   `frontend/src/lib/history/show-key.ts`, which `StreamProvider` is
//!   derived from — left out, its show keys read there as naming no
//!   provider and its title matches are filed under the legacy
//!   segment;
//! - the sites it contacts, in `docs/PRIVACY.md`, and its place in
//!   `docs/architecture.md`'s provider section.
//!
//! Nothing else names providers one by one: [`ProviderId::from_label`]
//! and [`ShowKey::parse`](crate::scraper::provider::ShowKey::parse)
//! walk every known id, so a new variant's label is recognised in
//! stored keys as soon as it exists. Its label has to equal the
//! identifier it serializes as — the renderer sends that identifier
//! where the API reads a label — which `provider_set_test.rs` checks
//! for every member of [`ProviderId::ALL`].
//!
//! A gate exists for every known id, not just the ordered ones, so a
//! change of order never leaves a walked provider without one.

use std::collections::HashMap;
use std::sync::Arc;

use crate::scraper::gate::ScraperGate;
use crate::scraper::provider::ProviderId;

/// The providers a state walks, in order, each with its own gate
/// and — for tests — the origin its client is pointed at instead of
/// the real site.
#[derive(Clone)]
pub struct ProviderSet {
    order: Vec<ProviderId>,
    gates: HashMap<ProviderId, Arc<ScraperGate>>,
    origins: HashMap<ProviderId, String>,
    /// Providers a test answers from tables in place of the site's
    /// client — keyed by the id each answers as.
    #[cfg(test)]
    stubs: HashMap<ProviderId, Arc<crate::scraper::table_provider::TableProvider>>,
}

impl ProviderSet {
    /// The providers the app ships, in the order a walk asks them.
    #[must_use]
    pub fn shipped() -> Self {
        Self::listing(vec![ProviderId::Hianime])
    }

    /// A set walking `order`, with a fresh gate for every known
    /// provider and no origin overrides. A test lists the providers
    /// it stubs, so a stubbed outage cannot fall through to a real
    /// site.
    #[must_use]
    pub fn listing(order: Vec<ProviderId>) -> Self {
        let known = ProviderId::ALL.iter();
        #[cfg(test)]
        let known = known.chain(ProviderId::TEST);
        let gates = known
            .map(|id| (*id, Arc::new(ScraperGate::new())))
            .collect();
        Self {
            order,
            gates,
            origins: HashMap::new(),
            #[cfg(test)]
            stubs: HashMap::new(),
        }
    }

    /// The set every unit test starts from: one provider, no
    /// overrides. A test that stubs an origin or orders two
    /// providers says so on top of it.
    #[cfg(test)]
    #[must_use]
    pub fn for_tests() -> Self {
        Self::listing(vec![Self::TEST_DEFAULT])
    }

    /// The provider a unit test walks unless it says otherwise: the
    /// one [`Self::for_tests`] orders, a table answers as by default,
    /// and a fixture row names when it is meant to be served.
    #[cfg(test)]
    pub const TEST_DEFAULT: ProviderId = ProviderId::TestA;

    /// The providers a walk runs against, in order — the next is
    /// asked only when the one before it was unreachable, refusing
    /// or broken (see `commands::providers`).
    #[must_use]
    pub fn order(&self) -> &[ProviderId] {
        &self.order
    }

    /// Replace the walk order. Gates and origins are untouched.
    pub fn set_order(&mut self, order: Vec<ProviderId>) {
        self.order = order;
    }

    /// The gate `provider`'s traffic is admitted through: it paces
    /// background probes and breaks the circuit on consecutive
    /// failures, so an outage on one provider never paces or refuses
    /// traffic to another.
    ///
    /// # Panics
    /// Never for a known id — every member of [`ProviderId::ALL`]
    /// has a gate from construction.
    #[must_use]
    pub fn gate(&self, provider: ProviderId) -> &ScraperGate {
        self.gates
            .get(&provider)
            .map(Arc::as_ref)
            .unwrap_or_else(|| unreachable!("a gate exists for every provider id"))
    }

    /// Where `provider`'s client points instead of the real site, when
    /// a test pointed it somewhere. `None` in production.
    #[must_use]
    pub fn origin(&self, provider: ProviderId) -> Option<&str> {
        self.origins.get(&provider).map(String::as_str)
    }

    /// Point `provider`'s client at `base` instead of the real site.
    pub fn set_origin(&mut self, provider: ProviderId, base: impl Into<String>) {
        self.origins.insert(provider, base.into());
    }

    /// Answer as `table` wherever the walk asks the provider it
    /// answers as, instead of building that provider's client. The
    /// state's gate still admits every request to it.
    #[cfg(test)]
    pub(crate) fn set_stub(&mut self, table: Arc<crate::scraper::table_provider::TableProvider>) {
        use crate::scraper::provider::Provider;
        self.stubs.insert(table.id(), table);
    }

    /// [`Self::set_stub`], chained.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_stub(
        mut self,
        table: Arc<crate::scraper::table_provider::TableProvider>,
    ) -> Self {
        self.set_stub(table);
        self
    }

    /// The table registered for `provider`, when a test registered one.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn stub(
        &self,
        provider: ProviderId,
    ) -> Option<Arc<crate::scraper::table_provider::TableProvider>> {
        self.stubs.get(&provider).cloned()
    }

    /// [`Self::set_origin`], chained.
    #[must_use]
    pub fn with_origin(mut self, provider: ProviderId, base: impl Into<String>) -> Self {
        self.set_origin(provider, base);
        self
    }
}

#[cfg(test)]
#[path = "provider_set_test.rs"]
mod tests;
