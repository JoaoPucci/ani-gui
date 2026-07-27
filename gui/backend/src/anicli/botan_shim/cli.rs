//! The botan-shim argv dispatcher over real streams. Provisioning
//! lives in the sibling `provision` file; both are split from the ops
//! so each stays a small unit under the per-file complexity ratchet.

use super::{gcm_decrypt, gcm_encrypt, hex_decode, parse_cipher_args, sha256_hex_upper, ShimError};
use std::io::{Read, Write};

/// Version string the wrapper reports for `--version`. ani-cli reads
/// only the first character (`3`) to select the Botan-3 syntax.
const SHIM_VERSION: &str = "3.0.0 (ani-gui shim)";

/// Run the `cipher` operation over the streams.
fn run_cipher(
    args: &[String],
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<(), ShimError> {
    let parsed = parse_cipher_args(args)?;
    let mut data = Vec::new();
    stdin.read_to_end(&mut data)?;
    let out = if parsed.decrypt {
        gcm_decrypt(&parsed.key, &parsed.nonce, &data)?
    } else {
        gcm_encrypt(&parsed.key, &parsed.nonce, &data)?
    };
    stdout.write_all(&out)?;
    Ok(())
}

/// Dispatch one shim invocation: `args` is everything after
/// `--botan-shim`. Returns the process exit code; error text goes to
/// stderr via `eprintln!` (ani-cli discards it where a real botan's
/// noise would also be discarded).
pub fn run_shim(args: &[String], stdin: &mut dyn Read, stdout: &mut dyn Write) -> u8 {
    let result: Result<(), ShimError> = match args.first().map(String::as_str) {
        Some("--version") => writeln!(stdout, "{SHIM_VERSION}").map_err(ShimError::from),
        Some("hash") => {
            let mut data = Vec::new();
            match stdin.read_to_end(&mut data) {
                Ok(_) => writeln!(stdout, "{}", sha256_hex_upper(&data)).map_err(ShimError::from),
                Err(e) => Err(ShimError::from(e)),
            }
        }
        Some("hex_dec") => {
            let mut text = String::new();
            match stdin.read_to_string(&mut text) {
                Ok(_) => hex_decode(&text)
                    .and_then(|raw| stdout.write_all(&raw).map_err(ShimError::from)),
                Err(e) => Err(ShimError::from(e)),
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
