//! The flags file's signature. Stub.

use super::resolve::Document;

/// Public keys whose signature the flags file is trusted under.
pub const TRUSTED_KEYS: &[&str] = &[];

/// Appended to the file's address to get its signature's.
pub const SIGNATURE_SUFFIX: &str = ".minisig";

/// What the signature says about the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Signed by a trusted key.
    Verified,
    /// No signature.
    Unsigned,
    /// A signature that does not verify.
    Invalid,
}

/// Stub.
#[must_use]
pub fn verify(body: &[u8], signature: Option<&str>, keys: &[&str]) -> Verdict {
    let _ = (body, signature, keys);
    Verdict::Unsigned
}

/// Stub.
#[must_use]
pub fn admit(doc: Document, verdict: Verdict) -> Document {
    let _ = verdict;
    doc
}

#[cfg(test)]
#[path = "signature_test.rs"]
mod tests;
