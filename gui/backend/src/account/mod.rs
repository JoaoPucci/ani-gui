//! Account integration — AniList + MyAnimeList + future in-house
//! provider.
//!
//! The [`provider::UserListProvider`] trait is the abstraction every
//! concrete provider implements; surfaces (rails, write-back,
//! `/account` page) call the trait, never a concrete type. See
//! `.planning/account-integration.md` for the full design + rationale.
//!
//! Submodule layout:
//!
//! - [`credentials`] — public OAuth client ids/secrets (ship in binary)
//! - [`pkce`] — RFC 7636 verifier/challenge generators (plain + S256)
//! - [`status`] — unified `ListStatus` enum + provider-native translations
//! - [`provider`] — `UserListProvider` trait + shared types
//!
//! Tokens NEVER live in this module's process. The Rust backend asks
//! Electron's `safeStorage` IPC for the decrypted bearer when it needs
//! to call upstream — see `.planning/account-integration.md` §3.4.

pub mod cache;
pub mod credentials;
pub mod internal_secret;
pub mod pkce;
pub mod provider;
pub mod status;

pub use internal_secret::{InternalSecret, INTERNAL_SECRET_HEADER};
