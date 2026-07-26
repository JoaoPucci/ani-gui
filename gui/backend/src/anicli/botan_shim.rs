//! In-process stand-in for the botan(1) CLI.
//!
//! ani-cli 4.15 hard-requires a botan binary for allanime's encrypted
//! transport (AES-256-GCM request signing + response decryption).
//! Rather than making users install Botan, the backend answers the
//! four invocations ani-cli makes when started as
//! `ani-gui-backend --botan-shim <args…>`, and provisions a tiny
//! `botan` wrapper script onto the tail of ani-cli's composed PATH —
//! so a real Botan installation, when present, still wins.
//!
//! The four invocations (Botan-3 syntax; the shim's `--version` first
//! character selects it in ani-cli):
//!
//! ```text
//! botan --version
//! botan hash --no-fsname                     # SHA-256, uppercase hex
//! botan hex_dec -                            # hex text -> raw bytes
//! botan cipher --cipher=AES-256/GCM [--decrypt] \
//!       --key=<hex> --nonce=<hex> -          # ct||tag framing
//! ```
//!
//! Everything here is pure over byte slices except [`run_shim`], which
//! is generic over its streams for testability. Exit codes mirror a
//! real CLI: 0 success, 1 operation failure (e.g. GCM auth), 2 usage.

use std::io::{Read, Write};

/// SHA-256 of `data` as uppercase hex — botan's `hash` output format
/// (the trailing newline is added by the dispatcher).
#[must_use]
pub fn sha256_hex_upper(data: &[u8]) -> String {
    let _ = data;
    todo!("green commit implements the shim")
}

/// Decode hex text into raw bytes, ignoring ASCII whitespace (botan's
/// `hex_dec` reads the `hash` output including its newline).
///
/// # Errors
/// A description when a non-hex digit remains or the digit count is odd.
pub fn hex_decode(text: &str) -> Result<Vec<u8>, String> {
    let _ = text;
    todo!("green commit implements the shim")
}

/// AES-256-GCM encrypt: returns ciphertext with the 16-byte tag
/// appended, exactly how botan's `cipher` writes it.
///
/// # Errors
/// A description when the key is not 32 bytes or the nonce not 12.
pub fn gcm_encrypt(key: &[u8], nonce: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let _ = (key, nonce, plaintext);
    todo!("green commit implements the shim")
}

/// AES-256-GCM decrypt of a `ciphertext||tag(16)` blob.
///
/// # Errors
/// A description on bad key/nonce lengths, a too-short blob, or GCM
/// authentication failure (tampered ciphertext or wrong key).
pub fn gcm_decrypt(key: &[u8], nonce: &[u8], ct_and_tag: &[u8]) -> Result<Vec<u8>, String> {
    let _ = (key, nonce, ct_and_tag);
    todo!("green commit implements the shim")
}

/// Parsed `cipher` arguments.
#[derive(Debug, PartialEq, Eq)]
pub struct CipherArgs {
    pub decrypt: bool,
    pub key: Vec<u8>,
    pub nonce: Vec<u8>,
}

/// Parse the argument list following `cipher`.
///
/// # Errors
/// A description when `--key=`/`--nonce=` are missing or not valid hex.
pub fn parse_cipher_args(args: &[String]) -> Result<CipherArgs, String> {
    let _ = args;
    todo!("green commit implements the shim")
}

/// Dispatch one shim invocation: `args` is everything after
/// `--botan-shim`. Returns the process exit code; error text goes to
/// stderr via `eprintln!` (ani-cli discards it where a real botan's
/// noise would also be discarded).
pub fn run_shim(args: &[String], stdin: &mut dyn Read, stdout: &mut dyn Write) -> u8 {
    let _ = (args, stdin, stdout);
    todo!("green commit implements the shim")
}

#[cfg(test)]
#[path = "botan_shim_test.rs"]
mod tests;
