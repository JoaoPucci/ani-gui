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
//! After printing, this binary serves until it is stopped. Electron
//! sends SIGTERM to the process group on app quit; the OS reaps us
//! cleanly because all threads are tokio's, none owning external
//! resources beyond the SQLite pool (closed on drop) and reqwest
//! sockets (closed on drop).
//!
//! A quit is not the only way Electron ends, so the backend also
//! watches for its parent being gone (see `ani_gui::parent_watch`):
//! when Electron sets `ANI_GUI_PARENT_STDIN=1`, end of file on stdin
//! shuts the server down gracefully, bounded by `PARENT_GONE_GRACE`,
//! and the runtime's teardown then stops any download trees.

#![forbid(unsafe_code)]

use std::sync::Arc;

use ani_gui::{api, app, parent_watch, proxy, AniError};

/// How long in-flight requests get to finish once the parent is gone
/// before the server stops waiting for them. Their client — the
/// renderer — died with the parent, so they end as soon as they next
/// write; the bound is for one stuck on an upstream read.
const PARENT_GONE_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

/// Resolves once the parent is gone, or never when the parent did not
/// ask to be watched (a backend run by hand).
async fn parent_gone() {
    if std::env::var(parent_watch::PARENT_STDIN_ENV).as_deref() != Ok("1") {
        return std::future::pending().await;
    }
    let mut rx = parent_watch::watch_for_eof(std::io::stdin());
    if rx.wait_for(|gone| *gone).await.is_err() {
        // The watch thread could not report; treat it as gone rather
        // than leave a backend nobody can reach.
        tracing::warn!("parent watch ended without a report");
    }
}

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
        Ok(r) => Arc::new(r),
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

        // The handshake: print the URL the Electron main process is
        // waiting for, then run forever. Flushing stdout matters —
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

        let (gone_tx, gone_rx) = tokio::sync::watch::channel(false);
        tokio::spawn(async move {
            parent_gone().await;
            tracing::info!("parent process gone; shutting down");
            let _ = gone_tx.send(true);
        });
        let mut shutdown = gone_rx.clone();
        let mut grace = gone_rx;
        let serve = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = shutdown.wait_for(|gone| *gone).await;
        });
        tokio::select! {
            served = serve => served.map_err(|_| AniError::Network)?,
            () = async move {
                let _ = grace.wait_for(|gone| *gone).await;
                tokio::time::sleep(PARENT_GONE_GRACE).await;
            } => tracing::warn!("in-flight requests outlived the shutdown grace"),
        }
        Ok::<_, AniError>(())
    });

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = ?e, "ani-gui-backend exited with error");
            std::process::ExitCode::FAILURE
        }
    }
}
