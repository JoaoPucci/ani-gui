//! The botan-shim process surface: argv dispatch over real streams,
//! plus wrapper-script provisioning. Split from the sibling ops file
//! so each stays a small unit under the per-file complexity ratchet.

use super::{gcm_decrypt, gcm_encrypt, hex_decode, parse_cipher_args, sha256_hex_upper};
use std::io::{Read, Write};

/// Version string the wrapper reports for `--version`. ani-cli reads
/// only the first character (`3`) to select the Botan-3 syntax.
const SHIM_VERSION: &str = "3.0.0 (ani-gui shim)";

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

/// Provision the botan wrapper for this process's own binary into
/// `<cache_root>/botan-shim`, returning the directory for PATH
/// appending. Best effort: any failure only means the spawn PATH
/// carries no shim and a machine without a system Botan fails exactly
/// as it did before the shim existed. Lives here (not app.rs) so
/// its branches and tests stay under the shim module's test file.
#[must_use]
pub fn provision_own_botan_shim(cache_root: &std::path::Path) -> Option<std::path::PathBuf> {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            tracing::warn!(target: "anicli::boot", error = %e, "current_exe failed; botan shim not provisioned");
            return None;
        }
    };
    match provision_botan_wrapper(&cache_root.join("botan-shim"), &exe) {
        Ok(dir) => Some(dir),
        Err(e) => {
            tracing::warn!(target: "anicli::boot", error = %e, "botan shim provisioning failed; relying on a system botan");
            None
        }
    }
}
