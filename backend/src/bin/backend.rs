//! Standalone backend binary — same Rust logic as the Tauri-bundled
//! app, minus Tauri itself. Used by Electron's main process, which
//! spawns this as a sidecar and reads its stdout to learn the bound
//! port.
//!
//! Stdout protocol: two lines,
//!     ANI_GUI_LISTENING http://127.0.0.1:<port>
//!     ANI_GUI_INTERNAL_SECRET <hex>
//! printed once the listener is bound and the state is built. The
//! Electron main process matches the prefixes and hands both to the
//! renderer through the preload script: `window.aniGui.apiBase` and
//! the renderer-only secret.
//!
//! After printing, this binary serves until it is asked to stop (see
//! `ani_gui::shutdown`). Electron asks in two ways. Its quit sends
//! SIGTERM to the backend's process group on Linux and macOS. And
//! because a quit is not the only way Electron ends, the backend
//! watches for its parent being gone (see `ani_gui::parent_watch`):
//! when Electron sets `ANI_GUI_PARENT_STDIN=1`, end of file on stdin
//! asks the same. A backend run by hand is asked by Ctrl+C, or by its
//! terminal closing.
//!
//! Either way the server winds down and `main` tears the runtime
//! down, dropping every task still running; both steps are bounded.
//! That teardown is what stops a running download's yt-dlp or ffmpeg:
//! they run in process groups of their own, which the signal sent to
//! the backend's group never reaches, and only the guard that owns
//! each one kills it. Dying on the signal instead would leave them
//! running.
//!
//! On Windows a quit runs `taskkill /F /T`, which ends the backend and
//! everything below it by parent pid; nothing is asked and nothing
//! needs to wind down.

#![forbid(unsafe_code)]

use std::sync::Arc;

use ani_gui::{api, app, proxy, shutdown, AniError};

/// Print the handshake and flush it — Electron reads it line by line,
/// so a line left in a buffer would hang the spawn.
///
/// Written with `writeln!`, not `println!`: a parent gone before the
/// handshake leaves stdout closed, and `println!` answers that with a
/// panic. Nobody is left to serve, so the caller fails the start
/// instead.
fn announce(api_base: &str, internal_secret: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    writeln!(out, "ANI_GUI_LISTENING {api_base}")?;
    writeln!(out, "ANI_GUI_INTERNAL_SECRET {internal_secret}")?;
    out.flush()
}

fn main() -> std::process::ExitCode {
    // Logging — RUST_LOG honoured, default keeps the noise down.
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "ani_gui=info".into());
    tracing_subscriber::fmt()
        // A log line that cannot be written is a line lost, nothing
        // more. Left on, the logger reports a failed write with
        // `eprintln!`, which panics when stderr fails as well — and
        // both fail together, being pipes to a parent, the moment
        // that parent dies. This build aborts on a panic, so the
        // first line logged after the parent's death would kill the
        // backend before it had stopped anything it started.
        .log_internal_errors(false)
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_target(true)
        .compact()
        .init();

    tracing::info!(version = ani_gui::VERSION, "starting ani-gui-backend");

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "tokio runtime build failed");
            return std::process::ExitCode::FAILURE;
        }
    };

    // Resource dir = the directory the binary lives in. Lets us find
    // the `bin/` directory the packages stage alongside the backend,
    // which carries the impersonating transport and the download
    // tools. Falls back to None when current_exe() can't resolve
    // (extremely rare); AppState::build then only checks PATH, which
    // is fine for `cargo run` development.
    let resource_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf));

    // Bind, build state, spawn server, hold the runtime.
    let result = runtime.block_on(async {
        let proxy_http = proxy::upstream::build_client()?;
        let (addr, listener) = proxy::bind_loopback(0).await?;
        let origin = proxy::ProxyOrigin::new(&addr.ip().to_string(), addr.port());
        let state = Arc::new(app::AppState::build(
            proxy_http,
            origin.clone(),
            resource_dir,
        )?);

        let proxy_router = proxy::build_router(state.proxy_state());
        let api_router = api::build_api_router(state.clone());
        let router = proxy_router.merge(api_router);

        // The signal handlers go in before the handshake announces the
        // backend: from the moment Electron knows it is up, a quit's
        // SIGTERM is a request rather than a kill.
        let stop = shutdown::requested();

        // The handshake: the URL the Electron main process is waiting
        // for, then serve until stopped.
        //
        // Codex P2 #3370011855: the renderer-only secret goes out on
        // the same channel so Electron's main process can parse it and
        // thread it through preload into `window.aniGui.
        // internalSecret`. The renderer attaches it as the
        // `x-ani-gui-internal-secret` header on the few backend paths
        // that need a cross-origin defense beyond the bearer.
        announce(&origin.base, state.internal_secret.as_hex()).map_err(|_| AniError::Io)?;
        tracing::info!(addr = %addr, "ani-gui-backend ready");

        shutdown::serve_until(
            listener,
            router,
            async move {
                let reason = stop.await;
                tracing::info!(?reason, "stopping");
            },
            shutdown::REQUEST_GRACE,
        )
        .await
        .map_err(|_| AniError::Network)?;
        Ok::<_, AniError>(())
    });

    // The server has stopped. Whatever is still running — a download,
    // above all — is dropped here, which is what stops its tools.
    shutdown::teardown(runtime, shutdown::TEARDOWN_LIMIT);

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = ?e, "ani-gui-backend exited with error");
            std::process::ExitCode::FAILURE
        }
    }
}
