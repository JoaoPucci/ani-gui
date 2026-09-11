//! The native resolver: search a provider, pick the right show, and
//! turn an episode into a playable URL.
//!
//! [`provider`] is the seam: the trait a stream provider implements
//! and the neutral hit and episode types the walks read. [`fetch`] is
//! the impersonating transport and [`gated`] its gate-admitting
//! decorator; [`hls`] is what every provider's playlists have in
//! common; [`anidb`] is one provider client — the parsers for that
//! site's search, episode listing, languages and embed responses over
//! the transport. [`gate`] paces the traffic and trips a breaker
//! when the provider starts refusing, [`outcome`] classifies what a
//! request's result says about the provider's health, and
//! [`reservation`] keeps concurrent callers from stampeding it.
//!
//! The commands in [`crate::commands`] compose these into the play,
//! download and availability walks; nothing here decides policy.

pub mod anidb;
pub mod fetch;
pub mod gate;
pub mod gated;
pub mod hianime;
pub mod hls;
pub mod outcome;
pub mod provider;
mod reservation;
