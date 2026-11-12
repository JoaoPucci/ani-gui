//! The flags file's signature.
//!
//! The maintainer signs `flags.json` with minisign (Ed25519 over a
//! BLAKE2b-512 hash) and publishes the detached signature beside it as
//! `flags.json.minisig` on the same branch. The backend verifies the
//! file's exact bytes against the public keys compiled into the build,
//! [`TRUSTED_KEYS`]. Any of them will do, so a rotation ships the next
//! key a release before the maintainer starts signing with it. The
//! trusted comment is covered by the signature as well, so it cannot be
//! edited either.
//!
//! What a verdict decides is narrow. Every flag ships off, and the
//! only value that moves one away from that is `true`, which turns a
//! feature on. [`admit`] keeps a verified file whole, and drops every
//! `true` from any other before the flags are decided or cached. The
//! rest of an unverified file is kept: its `false` values say no more
//! than the defaults do. The private key never enters this repository;
//! the steps are in `docs/development.md`, "Signing the flags file".

use minisign_verify::{PublicKey, Signature};

use super::resolve::Document;

/// The base64 minisign public keys the flags file is trusted under:
/// the second line of a `minisign.pub`. Empty until the maintainer
/// generates the signing key and adds its public half in a release;
/// with none, no file verifies.
pub const TRUSTED_KEYS: &[&str] = &[];

/// The keys a test run's file (`ANI_GUI_TEST_FLAGS_FILE`) is trusted
/// under as well as [`TRUSTED_KEYS`], so an end-to-end test can turn a
/// feature on. Only a backend built with the `test-run-key` cargo
/// feature has one: the fixture test-run key, whose secret anyone can
/// derive from `tests/fixtures/flags-signature/make.py`. No packaged
/// build enables the feature (`tests/arch/test_run_key.sh`), and the
/// live file is never trusted under these keys in any build.
#[cfg(feature = "test-run-key")]
pub const TEST_RUN_KEYS: &[&str] = &[include_str!(
    "../../tests/fixtures/flags-signature/test-run-key.pub"
)];

/// See the `test-run-key` build's definition: a build without the
/// feature trusts a test run's file under [`TRUSTED_KEYS`] alone.
#[cfg(not(feature = "test-run-key"))]
pub const TEST_RUN_KEYS: &[&str] = &[];

/// Appended to the file's address to get its signature's.
pub const SIGNATURE_SUFFIX: &str = ".minisig";

/// The largest signature read. A minisign signature file is a few
/// hundred bytes.
pub const MAX_SIGNATURE_BYTES: usize = 4 * 1024;

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

/// The document as this launch may use it: whole when verified.
/// Otherwise every entry whose value is `true` is dropped, so its flag
/// falls back to its default, off. This reads no declaration: for every
/// kind, `true` is the only value a signature alone may select
/// ([`super::decl::FlagDecl::signed_only`]), so the rule holds for a
/// flag declared by any build, including one that reads a cache this
/// build wrote. Entries no declaration accepts are left for resolution,
/// which ignores them.
#[must_use]
pub fn admit(mut doc: Document, verdict: Verdict) -> Document {
    if verdict != Verdict::Verified {
        doc.retain(|_, raw| !turns_on(raw));
    }
    doc
}

/// Whether `doc` holds what only a verified file keeps through
/// [`admit`]: a value that turns a flag on. A document that went
/// through admission and holds one is known to have verified.
#[must_use]
pub fn holds_signed_content(doc: &Document) -> bool {
    doc.values().any(turns_on)
}

/// Whether `raw` is the value that turns a flag on.
fn turns_on(raw: &serde_json::Value) -> bool {
    *raw == serde_json::Value::Bool(true)
}

#[cfg(test)]
#[path = "signature_test.rs"]
mod tests;

#[cfg(test)]
#[path = "signature_turn_on_test.rs"]
mod turn_on_tests;

#[cfg(test)]
#[path = "signature_minisign_test.rs"]
mod minisign_tests;
