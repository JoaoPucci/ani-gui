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
use std::io::{Read, Write};

/// Version string the wrapper reports for `--version`. ani-cli reads
/// only the first character (`3`) to select the Botan-3 syntax.
const SHIM_VERSION: &str = "3.0.0 (ani-gui shim)";

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
/// A description when a non-hex digit remains or the digit count is odd.
pub fn hex_decode(text: &str) -> Result<Vec<u8>, String> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .map(|b| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            b'A'..=b'F' => Ok(b - b'A' + 10),
            other => Err(format!("not a hex digit: {:?}", char::from(other))),
        })
        .collect::<Result<_, _>>()?;
    if digits.len() % 2 != 0 {
        return Err("odd number of hex digits".into());
    }
    Ok(digits
        .chunks(2)
        .map(|pair| (pair[0] << 4) | pair[1])
        .collect())
}

/// Build the cipher, validating key and nonce lengths.
fn gcm(key: &[u8], nonce: &[u8]) -> Result<Aes256Gcm, String> {
    if key.len() != 32 {
        return Err(format!(
            "AES-256/GCM needs a 32-byte key, got {}",
            key.len()
        ));
    }
    if nonce.len() != 12 {
        return Err(format!(
            "AES-256/GCM needs a 12-byte nonce, got {}",
            nonce.len()
        ));
    }
    Ok(Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key)))
}

/// AES-256-GCM encrypt: returns ciphertext with the 16-byte tag
/// appended, exactly how botan's `cipher` writes it.
///
/// # Errors
/// A description when the key is not 32 bytes or the nonce not 12.
pub fn gcm_encrypt(key: &[u8], nonce: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = gcm(key, nonce)?;
    cipher
        .encrypt(Nonce::from_slice(nonce), Payload::from(plaintext))
        .map_err(|_| "encryption failed".into())
}

/// AES-256-GCM decrypt of a `ciphertext||tag(16)` blob.
///
/// # Errors
/// A description on bad key/nonce lengths, a too-short blob, or GCM
/// authentication failure (tampered ciphertext or wrong key).
pub fn gcm_decrypt(key: &[u8], nonce: &[u8], ct_and_tag: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = gcm(key, nonce)?;
    if ct_and_tag.len() < 16 {
        return Err("input shorter than the GCM tag".into());
    }
    cipher
        .decrypt(Nonce::from_slice(nonce), Payload::from(ct_and_tag))
        .map_err(|_| "GCM authentication failed".into())
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
/// A description when `--key=`/`--nonce=` are missing or not valid hex.
pub fn parse_cipher_args(args: &[String]) -> Result<CipherArgs, String> {
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
        key: key.ok_or("cipher needs --key=<hex>")?,
        nonce: nonce.ok_or("cipher needs --nonce=<hex>")?,
    })
}

/// Run the `cipher` operation over the streams.
fn run_cipher(args: &[String], stdin: &mut dyn Read, stdout: &mut dyn Write) -> Result<(), String> {
    let parsed = parse_cipher_args(args)?;
    let mut data = Vec::new();
    stdin
        .read_to_end(&mut data)
        .map_err(|e| format!("reading stdin: {e}"))?;
    let out = if parsed.decrypt {
        gcm_decrypt(&parsed.key, &parsed.nonce, &data)?
    } else {
        gcm_encrypt(&parsed.key, &parsed.nonce, &data)?
    };
    stdout
        .write_all(&out)
        .map_err(|e| format!("writing stdout: {e}"))
}

/// Dispatch one shim invocation: `args` is everything after
/// `--botan-shim`. Returns the process exit code; error text goes to
/// stderr via `eprintln!` (ani-cli discards it where a real botan's
/// noise would also be discarded).
pub fn run_shim(args: &[String], stdin: &mut dyn Read, stdout: &mut dyn Write) -> u8 {
    let result: Result<(), String> = match args.first().map(String::as_str) {
        Some("--version") => writeln!(stdout, "{SHIM_VERSION}").map_err(|e| e.to_string()),
        Some("hash") => {
            let mut data = Vec::new();
            match stdin.read_to_end(&mut data) {
                Ok(_) => writeln!(stdout, "{}", sha256_hex_upper(&data)).map_err(|e| e.to_string()),
                Err(e) => Err(format!("reading stdin: {e}")),
            }
        }
        Some("hex_dec") => {
            let mut text = String::new();
            match stdin.read_to_string(&mut text) {
                Ok(_) => hex_decode(&text)
                    .and_then(|raw| stdout.write_all(&raw).map_err(|e| e.to_string())),
                Err(e) => Err(format!("reading stdin: {e}")),
            }
        }
        Some("cipher") => run_cipher(&args[1..], stdin, stdout),
        _ => {
            eprintln!("botan-shim: unsupported invocation: {args:?}");
            return 2;
        }
    };
    match result {
        Ok(()) => 0,
        Err(msg) => {
            eprintln!("botan-shim: {msg}");
            1
        }
    }
}

/// Write the `botan` wrapper script into `dir` and return `dir` for
/// PATH appending. The wrapper execs `backend_exe --botan-shim "$@"`,
/// so ani-cli's `dep_ch_failover "botan3,botan,botan-cli"` resolves it
/// like a real botan. Regenerated on every boot — a moved or upgraded
/// backend binary self-heals.
///
/// # Errors
/// Propagates filesystem errors (create dir, write, chmod).
pub fn provision_botan_wrapper(
    dir: &std::path::Path,
    backend_exe: &std::path::Path,
) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dir)?;
    let wrapper = dir.join("botan");
    let body = format!(
        "#!/bin/sh\n\
         # Provisioned by ani-gui-backend on boot; execs the backend's\n\
         # in-process botan shim for ani-cli's encrypted allanime\n\
         # transport. Appended to the spawn PATH, so a real Botan\n\
         # installation always wins over this wrapper.\n\
         exec \"{}\" --botan-shim \"$@\"\n",
        backend_exe.display()
    );
    std::fs::write(&wrapper, body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(dir.to_path_buf())
}

#[cfg(test)]
#[path = "botan_shim_test.rs"]
mod tests;
