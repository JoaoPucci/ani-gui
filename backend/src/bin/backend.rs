//! Standalone backend binary — same Rust logic as the Tauri-bundled
//! app, minus Tauri itself. Used by Electron's main process, which
//! spawns this as a sidecar and reads its stdout to learn the bound
//! port.
//!
//! Stdout protocol: a single line of the form
//!     ANI_GUI_LISTENING http://127.0.0.1:<port>
//! is printed once the axum server is accepting connections. The
//! Electron main process matches that prefix, parses the URL, and
//! injects it into the renderer via the preload script's
//! `window.aniGui.apiBase`.
//!
//! After printing, this binary serves until it is asked to stop (see
//! `ani_gui::shutdown`). Two things ask. Electron's quit sends SIGTERM
//! to the backend's process group on Linux and macOS. And because a
//! quit is not the only way Electron ends, the backend watches for its
//! parent being gone (see `ani_gui::parent_watch`): when Electron sets
//! `ANI_GUI_PARENT_STDIN=1`, end of file on stdin asks the same.
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

fn main() -> std::process::ExitCode {
    // Logging — RUST_LOG honoured, default keeps the noise down.
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "ani_gui=info".into());
    tracing_subscriber::fmt()
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

        // Listening for a stop starts before the handshake announces
        // the backend: from the moment Electron knows it is up, a quit
        // is a request rather than a kill.
        let stop = shutdown::requested();

        // The handshake: print the URL the Electron main process is
        // waiting for, then serve until stopped. Flushing stdout matters —
        // Electron may buffer line-by-line, so any partial line could
        // hang the spawn.
        println!("ANI_GUI_LISTENING {}", origin.base);
        // Codex P2 #3370011855: emit the renderer-only secret on the
        // same handshake channel so Electron's main process can parse
        // it and thread it through preload into `window.aniGui.
        // internalSecret`. The renderer attaches it as the
        // `x-ani-gui-internal-secret` header on the few backend paths
        // that need a cross-origin defense beyond the bearer.
        println!("ANI_GUI_INTERNAL_SECRET {}", state.internal_secret.as_hex());
        use std::io::Write;
        let _ = std::io::stdout().flush();
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
