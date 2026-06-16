//! `InternalSecret` — random per-process secret that gates renderer-
//! only backend paths.
//!
//! Codex P2 #3370011855: the `delete_list_cache` fallback path accepts
//! a renderer-supplied `fallback_user_id` when the bearer has expired.
//! Under the permissive CORS layer that opens a cross-origin cache-
//! wipe vector — any tab in the user's browser can send `Authorization:
//! Bearer garbage` plus a guessed user_id and clear the rows. The fix
//! is a per-process random secret only the Electron renderer learns
//! (via stdout handshake at backend startup + preload bridge into the
//! page), required as the `x-ani-gui-internal-secret` header on
//! gated paths.

use axum::http::{HeaderMap, HeaderName};
use rand::RngCore;

use crate::error::AniError;

/// Wire header name carrying the renderer-only secret.
pub const INTERNAL_SECRET_HEADER: HeaderName = HeaderName::from_static("x-ani-gui-internal-secret");

/// 32-byte random secret printed to stdout once at backend startup as
/// `ANI_GUI_INTERNAL_SECRET <hex>`. Electron parses the line, threads
/// the hex string through preload into `window.aniGui.internalSecret`,
/// and the renderer sends it as a header on the few gated paths.
#[derive(Clone)]
pub struct InternalSecret {
    hex: String,
}

impl InternalSecret {
    /// Generate a fresh secret. Called exactly once per AppState build.
    #[must_use]
    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let hex = bytes.iter().map(|b| format!("{b:02x}")).collect();
        Self { hex }
    }

    /// Test-only constructor. Reuses the same shape so tests can pin
    /// the hex value without touching `rand`.
    #[cfg(test)]
    #[must_use]
    pub fn from_hex_for_test(hex: &str) -> Option<Self> {
        if hex.chars().all(|c| c.is_ascii_hexdigit()) {
            Some(Self { hex: hex.into() })
        } else {
            None
        }
    }

    /// Hex-encoded secret for printing / wiring.
    #[must_use]
    pub fn as_hex(&self) -> &str {
        &self.hex
    }

    /// Reject headers that don't carry the matching secret. Uses
    /// `subtle::ConstantTimeEq` so timing leaks on the comparison
    /// don't gradually reveal the secret over many attempts.
    ///
    /// # Errors
    /// - [`AniError::InvalidToken`] for missing / non-utf8 / mismatched
    ///   values. The single error variant keeps the response identical
    ///   across failure modes — no oracle for whether the header was
    ///   present but malformed vs. simply wrong.
    pub fn validate_header(&self, headers: &HeaderMap) -> Result<(), AniError> {
        use subtle::ConstantTimeEq;
        let raw = headers
            .get(&INTERNAL_SECRET_HEADER)
            .ok_or(AniError::InvalidToken)?;
        let got = raw.to_str().map_err(|_| AniError::InvalidToken)?;
        if got.as_bytes().ct_eq(self.hex.as_bytes()).into() {
            Ok(())
        } else {
            Err(AniError::InvalidToken)
        }
    }
}

#[cfg(test)]
#[path = "internal_secret_test.rs"]
mod tests;
