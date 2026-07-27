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

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use sha2::{Digest, Sha256};
mod cli;
mod provision;
pub use cli::run_shim;
#[cfg(test)]
pub(crate) use provision::{per_process_shim_dir, prune_stale_shim_dirs};
pub use provision::{provision_botan_wrapper, provision_own_botan_shim};

/// Typed failures for the shim's operations, per the library-boundary
/// error convention. [`run_shim`] renders them to stderr; the process
/// exit codes are the CLI contract (1 operation failure, 2 usage).
#[derive(Debug, thiserror::Error)]
pub enum ShimError {
    /// A non-hex character reached the decoder.
    #[error("not a hex digit: {0:?}")]
    NotHex(char),
    /// Hex text with an odd digit count.
    #[error("odd number of hex digits")]
    OddHexLength,
    /// Key of the wrong size for AES-256.
    #[error("AES-256/GCM needs a 32-byte key, got {0}")]
    KeyLength(usize),
    /// Nonce of the wrong size for GCM.
    #[error("AES-256/GCM needs a 12-byte nonce, got {0}")]
    NonceLength(usize),
    /// Decrypt input shorter than the 16-byte tag.
    #[error("input shorter than the GCM tag")]
    InputTooShort,
    /// Tag verification failed — tampered input or wrong key.
    #[error("GCM authentication failed")]
    AuthFailed,
    /// Encryption failed inside the cipher (practically unreachable
    /// once key and nonce lengths are validated).
    #[error("encryption failed")]
    EncryptFailed,
    /// `cipher` invoked without a required argument.
    #[error("cipher needs {0}")]
    MissingCipherArg(&'static str),
    /// Reading stdin or writing stdout failed.
    #[error("stdio: {0}")]
    Io(#[from] std::io::Error),
}

/// SHA-256 of `data` as uppercase hex — botan's `hash` output format
/// (the trailing newline is added by the dispatcher).
#[must_use]
pub fn sha256_hex_upper(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02X}"));
    }
    out
}

/// Decode hex text into raw bytes, ignoring ASCII whitespace (botan's
/// `hex_dec` reads the `hash` output including its newline).
///
/// # Errors
/// [`ShimError`] when a non-hex digit remains or the digit count is odd.
pub fn hex_decode(text: &str) -> Result<Vec<u8>, ShimError> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .map(|b| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            b'A'..=b'F' => Ok(b - b'A' + 10),
            other => Err(ShimError::NotHex(char::from(other))),
        })
        .collect::<Result<_, _>>()?;
    if digits.len() % 2 != 0 {
        return Err(ShimError::OddHexLength);
    }
    Ok(digits
        .chunks(2)
        .map(|pair| (pair[0] << 4) | pair[1])
        .collect())
}

/// Build the cipher, validating key and nonce lengths.
fn gcm(key: &[u8], nonce: &[u8]) -> Result<Aes256Gcm, ShimError> {
    if key.len() != 32 {
        return Err(ShimError::KeyLength(key.len()));
    }
    if nonce.len() != 12 {
        return Err(ShimError::NonceLength(nonce.len()));
    }
    Ok(Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key)))
}

/// AES-256-GCM encrypt: returns ciphertext with the 16-byte tag
/// appended, exactly how botan's `cipher` writes it.
///
/// # Errors
/// [`ShimError`] when the key is not 32 bytes or the nonce not 12.
pub fn gcm_encrypt(key: &[u8], nonce: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, ShimError> {
    let cipher = gcm(key, nonce)?;
    cipher
        .encrypt(Nonce::from_slice(nonce), Payload::from(plaintext))
        .map_err(|_| ShimError::EncryptFailed)
}

/// AES-256-GCM decrypt of a `ciphertext||tag(16)` blob.
///
/// # Errors
/// [`ShimError`] on bad key/nonce lengths, a too-short blob, or GCM
/// authentication failure (tampered ciphertext or wrong key).
pub fn gcm_decrypt(key: &[u8], nonce: &[u8], ct_and_tag: &[u8]) -> Result<Vec<u8>, ShimError> {
    let cipher = gcm(key, nonce)?;
    if ct_and_tag.len() < 16 {
        return Err(ShimError::InputTooShort);
    }
    cipher
        .decrypt(Nonce::from_slice(nonce), Payload::from(ct_and_tag))
        .map_err(|_| ShimError::AuthFailed)
}

/// Parsed `cipher` arguments.
#[derive(Debug, PartialEq, Eq)]
pub struct CipherArgs {
    /// `--decrypt` present — decrypt instead of encrypt.
    pub decrypt: bool,
    /// Decoded `--key=<hex>` bytes.
    pub key: Vec<u8>,
    /// Decoded `--nonce=<hex>` bytes.
    pub nonce: Vec<u8>,
}

/// Parse the argument list following `cipher`.
///
/// # Errors
/// [`ShimError`] when `--key=`/`--nonce=` are missing or not valid hex.
pub fn parse_cipher_args(args: &[String]) -> Result<CipherArgs, ShimError> {
    let mut decrypt = false;
    let mut key = None;
    let mut nonce = None;
    for arg in args {
        if arg == "--decrypt" {
            decrypt = true;
        } else if let Some(v) = arg.strip_prefix("--key=") {
            key = Some(hex_decode(v)?);
        } else if let Some(v) = arg.strip_prefix("--nonce=") {
            nonce = Some(hex_decode(v)?);
        }
        // --cipher=… and the trailing "-" are accepted and ignored:
        // the shim only implements AES-256/GCM over stdin/stdout.
    }
    Ok(CipherArgs {
        decrypt,
        key: key.ok_or(ShimError::MissingCipherArg("--key=<hex>"))?,
        nonce: nonce.ok_or(ShimError::MissingCipherArg("--nonce=<hex>"))?,
    })
}

#[cfg(test)]
#[path = "botan_shim_test.rs"]
mod tests;
