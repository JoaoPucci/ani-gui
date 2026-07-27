//! The botan-shim process surface: argv dispatch over real streams,
//! plus wrapper-script provisioning. Split from the sibling ops file
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

/// The wrapper directory for one backend process:
/// `<cache_root>/botan-shim/<pid>`. Scoped per process so concurrent
/// instances (each AppImage mounts at its own transient path) never
/// overwrite each other's wrapper.
pub(crate) fn per_process_shim_dir(cache_root: &std::path::Path, pid: u32) -> std::path::PathBuf {
    cache_root.join("botan-shim").join(pid.to_string())
}

/// Best-effort removal of sibling wrapper dirs whose owning process is
/// gone. `keep` is this process's dir; entries that don't parse as a
/// pid are left alone.
pub(crate) fn prune_stale_shim_dirs(
    root: &std::path::Path,
    keep: &std::path::Path,
    is_alive: impl Fn(u32) -> bool,
) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == keep {
            continue;
        }
        let Some(pid) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        if !is_alive(pid) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// Whether a process id belongs to a live process. Linux-only signal
/// (procfs); elsewhere assume alive so pruning never removes a
/// wrapper another instance still needs.
fn pid_is_alive(pid: u32) -> bool {
    if cfg!(target_os = "linux") {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    } else {
        true
    }
}

/// Serialize a string as a single shell-safe word: single-quoted, with
/// embedded single quotes rendered as `'\''`. Double-quoted
/// interpolation would let /bin/sh expand `$…` or backticks inside the
/// install path.
fn sh_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
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
         exec {} --botan-shim \"$@\"\n",
        sh_single_quote(&backend_exe.display().to_string())
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
    let dir = per_process_shim_dir(cache_root, std::process::id());
    match provision_botan_wrapper(&dir, &exe) {
        Ok(dir) => {
            if let Some(root) = dir.parent() {
                prune_stale_shim_dirs(root, &dir, pid_is_alive);
            }
            Some(dir)
        }
        Err(e) => {
            tracing::warn!(target: "anicli::boot", error = %e, "botan shim provisioning failed; relying on a system botan");
            None
        }
    }
}
