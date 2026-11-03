//! The flags file's signature, and what it decides.
//!
//! Most of what the file says needs no signature: by the rule in
//! [`super`], a kill switch or a fallback can only move toward the
//! safer side. Two things are different. Turning a feature on (a
//! [`super::decl::FlagKind::Rollout`]'s `true`) is riskier than the
//! build's own default. The notices the file may carry (see
//! [`crate::notices`]) are content: text and a link shown to every
//! user, and a number that makes the app ask for something. Both are
//! trusted only when the file is signed.
//!
//! The maintainer signs `flags.json` with minisign (Ed25519 over a
//! BLAKE2b-512 hash) and publishes the detached signature beside it as
//! `flags.json.minisig` on the same branch. The launch fetches both;
//! the backend verifies the file's exact bytes against the public keys
//! compiled into the build, [`TRUSTED_KEYS`]. Any of them will do, so a
//! rotation ships the next key a release before the maintainer starts
//! signing with it. The trusted comment is covered by the signature as
//! well, so it cannot be edited either.
//!
//! What a verdict decides is narrow: [`admit`] keeps a verified file
//! whole, and from any other drops the notices block and every flag
//! value only a signature may select. The rest of its flags are kept
//! whatever the verdict, under the safer-only rule they always had.
//! The private key never enters this repository; the steps are in
//! `docs/development.md`, "Signing the flags file".

use minisign_verify::{PublicKey, Signature};

use super::decl::FlagDecl;
use super::resolve::Document;
use crate::notices::NOTICES_KEY;

/// The base64 minisign public keys the flags file is trusted under:
/// the second line of a `minisign.pub`. Empty until the maintainer
/// generates the signing key and adds its public half in a release;
/// with none, no file verifies and no remote content is used.
pub const TRUSTED_KEYS: &[&str] = &[];

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

/// The document as this launch may use it, under the build's own
/// declarations ([`super::DECLARED`]); see [`admit_under`].
#[must_use]
pub fn admit(doc: Document, verdict: Verdict) -> Document {
    admit_under(doc, verdict, super::DECLARED)
}

/// The document as a launch declaring `decls` may use it: whole when
/// verified. Otherwise its notices block is dropped, and so is every
/// entry that names a value only a signature may select (a rollout
/// turned on), so the flag falls back to its default. Entries no
/// declaration accepts are left for resolution, which ignores them.
#[must_use]
pub fn admit_under(mut doc: Document, verdict: Verdict, decls: &[FlagDecl]) -> Document {
    if verdict == Verdict::Verified {
        return doc;
    }
    doc.remove(NOTICES_KEY);
    for decl in decls {
        let signed_only = doc
            .get(decl.name)
            .and_then(|raw| decl.accept(raw))
            .is_some_and(|v| decl.signed_only(v));
        if signed_only {
            doc.remove(decl.name);
        }
    }
    doc
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
