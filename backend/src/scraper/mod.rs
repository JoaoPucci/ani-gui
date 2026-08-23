//! The native resolver: search a provider, pick the right show, and
//! turn an episode into a playable URL.
//!
//! [`anidb`] is the client — an impersonating transport plus the
//! parsers for the provider's search, episode listing, languages and
//! embed responses. [`gate`] paces the traffic and trips a breaker
//! when the provider starts refusing, [`outcome`] classifies what a
//! request's result says about the provider's health, and
//! [`reservation`] keeps concurrent callers from stampeding it.
//!
//! The commands in [`crate::commands`] compose these into the play,
//! download and availability walks; nothing here decides policy.

pub mod anidb;
pub mod gate;
pub mod outcome;
mod reservation;
