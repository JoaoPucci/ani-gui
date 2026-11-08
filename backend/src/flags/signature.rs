//! The flags file's signature.
//!
//! The maintainer signs `flags.json` with minisign (Ed25519 over a
//! BLAKE2b-512 hash) and publishes the detached signature beside it as
//! `flags.json.minisig` on the same branch. The backend verifies the
//! file's exact bytes against the public keys compiled into the build,
//! [`TRUSTED_KEYS`]. Any of them will do, so a rotation ships the next
//! key a release before the maintainer starts signing with it. The
//! trusted comment is covered by the signature as well, so it cannot be
//! edited either. The private key never enters this repository; the
//! steps are in `docs/development.md`, "Signing the flags file".

use minisign_verify::{PublicKey, Signature};

/// The base64 minisign public keys the flags file is trusted under:
/// the second line of a `minisign.pub`. Empty until the maintainer
/// generates the signing key and adds its public half in a release;
/// with none, no file verifies.
pub const TRUSTED_KEYS: &[&str] = &[];

/// Appended to the file's address to get its signature's.
pub const SIGNATURE_SUFFIX: &str = ".minisig";

/// What the signature says about the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Signed by a key this build trusts, over these exact bytes.
    Verified,
    /// No signature was found.
    Unsigned,
    /// A signature that does not verify: another key, other bytes, an
    /// edited trusted comment, or not a signature at all.
    Invalid,
}

/// Verify `body` against `signature` under any of `keys`. Only the
/// prehashed algorithm minisign uses by default is accepted, not the
/// legacy one.
#[must_use]
pub fn verify(body: &[u8], signature: Option<&str>, keys: &[&str]) -> Verdict {
    let Some(signature) = signature else {
        return Verdict::Unsigned;
    };
    let Ok(signature) = Signature::decode(signature) else {
        return Verdict::Invalid;
    };
    let trusted = keys
        .iter()
        .filter_map(|k| PublicKey::from_base64(k.trim()).ok())
        .any(|k| k.verify(body, &signature, false).is_ok());
    if trusted {
        Verdict::Verified
    } else {
        Verdict::Invalid
    }
}

#[cfg(test)]
#[path = "signature_test.rs"]
mod tests;

#[cfg(test)]
#[path = "signature_minisign_test.rs"]
mod minisign_tests;
