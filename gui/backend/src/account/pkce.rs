//! PKCE (RFC 7636) verifier + challenge generators.
//!
//! Two variants:
//!
//! - [`Pkce::new_plain`] — `code_challenge_method=plain`. Required for
//!   MAL: the spec at <https://myanimelist.net/apiconfig/references/authorization>
//!   states "Currently, only the `plain` method is supported." A code
//!   sweep that "fixes" this to S256 will silently break MAL login.
//! - [`Pkce::new_s256`] — `code_challenge_method=S256`. Standard for
//!   native OAuth clients per RFC 7636 §4.2. AniList ignores PKCE
//!   entirely, but we generate one for trait symmetry.

use base64::Engine;
use rand::Rng;

/// PKCE verifier + challenge pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    /// Long random secret kept client-side. Used at token-exchange time.
    pub verifier: String,
    /// Derived value sent to the authorize endpoint.
    pub challenge: String,
    /// Method advertised to the authorize endpoint.
    pub method: PkceMethod,
}

/// PKCE challenge method per RFC 7636 §4.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkceMethod {
    /// `code_challenge_method=plain`. Challenge equals verifier.
    Plain,
    /// `code_challenge_method=S256`. Challenge = base64url(sha256(verifier)).
    S256,
}

impl PkceMethod {
    /// Wire string used in the authorize URL query.
    #[must_use]
    pub fn as_param(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::S256 => "S256",
        }
    }
}

impl Pkce {
    /// Generate a `plain`-method PKCE pair. Required for MyAnimeList.
    #[must_use]
    pub fn new_plain() -> Self {
        let verifier = random_verifier();
        Self {
            challenge: verifier.clone(),
            verifier,
            method: PkceMethod::Plain,
        }
    }

    /// Generate an `S256`-method PKCE pair. AniList ignores PKCE but
    /// the trait shape is symmetric.
    #[must_use]
    pub fn new_s256() -> Self {
        use sha2::{Digest, Sha256};
        let verifier = random_verifier();
        let digest = Sha256::digest(verifier.as_bytes());
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
        Self {
            verifier,
            challenge,
            method: PkceMethod::S256,
        }
    }
}

/// RFC 7636 §4.1 — verifier is 43..=128 chars from
/// `[A-Z][a-z][0-9]-._~`. 64 chars is comfortably mid-range and gives
/// 384 bits of entropy.
fn random_verifier() -> String {
    const LEN: usize = 64;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut rng = rand::thread_rng();
    (0..LEN)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}
