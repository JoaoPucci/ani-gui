//! Transport for the anidb client — split from the module head so
//! each file stays inside the complexity ratchet's per-file bar.

use std::path::{Path, PathBuf};

use crate::error::{AniError, Result};

use super::{CURL_FAILOVER, IMPERSONATE_AGENT};

/// A fetched response: enough for the client to tell content from a
/// challenge page without transport details leaking upward.
#[derive(Debug, Clone)]
pub struct FetchResponse {
    /// HTTP status of the final response after redirects.
    pub status: u16,
    /// Response body, lossily decoded.
    pub body: String,
}

/// Transport seam. Implemented by the curl-impersonate subprocess in
/// production and by fixture-backed fakes in tests.
#[async_trait::async_trait]
pub trait AnidbFetch: Send + Sync {
    /// GET `url` and return status + body.
    ///
    /// # Errors
    /// [`AniError::Network`] on spawn/transport failure,
    /// [`AniError::Timeout`] when the request exceeds its deadline.
    async fn get(&self, url: &str) -> Result<FetchResponse>;
}

/// Production transport: a curl-impersonate binary spawned per
/// request with the script's own flags.
#[derive(Debug, Clone)]
pub struct CurlImpersonateFetch {
    exe: PathBuf,
    deadline: std::time::Duration,
}

/// Executable-name suffixes the platform's binaries carry. Windows
/// ships `curl.exe` and `curl_chrome136.exe`; the bare name stays
/// first everywhere so a suffixless shim wins where both exist.
/// Deliberately narrower than `PATHEXT`: the resolver hunts real curl
/// executables, and a `.bat`/`.cmd` entry resolved through the wider
/// table would name something the spawn cannot treat as curl.
#[cfg(windows)]
const EXE_SUFFIXES: &[&str] = &["", ".exe"];
#[cfg(not(windows))]
const EXE_SUFFIXES: &[&str] = &[""];

/// Every filename `name` may carry given the platform's suffix table,
/// bare name first.
pub(crate) fn candidate_names(name: &str, suffixes: &[&str]) -> Vec<String> {
    suffixes.iter().map(|s| format!("{name}{s}")).collect()
}

impl CurlImpersonateFetch {
    /// Walk [`CURL_FAILOVER`] across `extra_dir` (the bundled-binary
    /// directory, when packaging ships one) and then the given PATH
    /// string, returning the first executable found — the same
    /// preference order as the script's `dep_ch_failover`, widened by
    /// the platform's executable suffixes.
    pub fn resolve(extra_dir: Option<&Path>, path_env: &str) -> Option<Self> {
        Self::resolve_with_suffixes(extra_dir, path_env, EXE_SUFFIXES)
    }

    /// [`Self::resolve`] with the suffix table explicit, so the
    /// Windows arm is exercisable from any platform's tests.
    pub(crate) fn resolve_with_suffixes(
        extra_dir: Option<&Path>,
        path_env: &str,
        suffixes: &[&str],
    ) -> Option<Self> {
        for name in CURL_FAILOVER {
            for file in candidate_names(name, suffixes) {
                if let Some(dir) = extra_dir {
                    let candidate = dir.join(&file);
                    if is_executable(&candidate) {
                        return Some(Self {
                            exe: candidate,
                            deadline: FETCH_TIMEOUT,
                        });
                    }
                }
                for dir in std::env::split_paths(path_env) {
                    let candidate = dir.join(&file);
                    if is_executable(&candidate) {
                        return Some(Self {
                            exe: candidate,
                            deadline: FETCH_TIMEOUT,
                        });
                    }
                }
            }
        }
        None
    }

    /// The resolved executable, for logging and diagnostics.
    pub fn exe(&self) -> &Path {
        &self.exe
    }

    /// Replace the outer deadline — the seam the hang test drives;
    /// production keeps [`FETCH_TIMEOUT`] from resolution. Gated to
    /// the platform that drives it: the hang test stages a shell
    /// stub, so on Windows the seam would be dead code under
    /// -D warnings.
    #[cfg(all(test, unix))]
    pub(crate) fn with_deadline(mut self, deadline: std::time::Duration) -> Self {
        self.deadline = deadline;
        self
    }
}

/// Whether `path` names an executable regular file.
fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Per-request deadline for the subprocess; slightly above the
/// script's own `--max-time 10` so curl reports its timeout first.
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// The script's Darwin cipher pinning (its `ciphers`/`tls13_ciphers`
/// globals, applied by the Darwin arm of its platform case): macOS
/// curl builds negotiate default suites the provider's TLS
/// fingerprinting rejects. Everywhere else the impersonate build's
/// own defaults ARE the fingerprint, so no flags are added.
#[cfg(target_os = "macos")]
const CIPHER_ARGS: &[&str] = &[
    "--ciphers",
    "ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305",
    "--tls13-ciphers",
    "TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256",
];
#[cfg(not(target_os = "macos"))]
const CIPHER_ARGS: &[&str] = &[];

#[async_trait::async_trait]
impl AnidbFetch for CurlImpersonateFetch {
    async fn get(&self, url: &str) -> Result<FetchResponse> {
        // `-w` appends the status after the body; the last line is
        // split back off. Mirrors the script's anidb_curl flags.
        let mut cmd = tokio::process::Command::new(&self.exe);
        // §5's subprocess environment: nothing the child prints may
        // depend on the terminal that launched the backend.
        cmd.env("TERM", "dumb")
            .env("NO_COLOR", "1")
            .arg("-sL")
            .arg("-A")
            .arg(IMPERSONATE_AGENT)
            .arg("--max-time")
            .arg("10")
            .args(CIPHER_ARGS)
            .arg("-w")
            .arg("\n%{http_code}")
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            // §5's subprocess rule: when the deadline drops the
            // output() future, the child goes with it instead of
            // surviving as an orphan for its full hang.
            .kill_on_drop(true);
        let output = tokio::time::timeout(self.deadline, cmd.output())
            .await
            .map_err(|_| AniError::Timeout)?
            .map_err(|_| AniError::Network)?;
        // The -w trailer reports the last HTTP status even when the
        // transfer then failed, so a nonzero exit means the body is
        // not to be trusted whatever the trailer parses to.
        if !output.status.success() {
            return Err(AniError::Network);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let (body, status_line) = text.rsplit_once('\n').unwrap_or(("", &text));
        let status: u16 = status_line.trim().parse().map_err(|_| AniError::Network)?;
        if status == 0 {
            // curl writes 000 when the transfer itself failed.
            return Err(AniError::Network);
        }
        Ok(FetchResponse {
            status,
            body: body.to_string(),
        })
    }
}

#[cfg(test)]
#[path = "fetch_prop_test.rs"]
mod fetch_prop_tests;
