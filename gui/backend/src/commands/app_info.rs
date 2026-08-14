//! `app_info` command — meta about the running backend.

use serde::Serialize;

use crate::error::Result;

/// Snapshot of build/runtime info the frontend may want at startup.
#[derive(Debug, Clone, Serialize)]
pub struct AppInfo {
    /// Crate version from Cargo.toml, with a `-dev` marker under the dev profile.
    pub version: String,
    /// Where this build of ani-gui keeps its own watch history.
    pub history_path: String,
    /// `http://127.0.0.1:<port>` for the streaming proxy.
    pub proxy_base_url: String,
}

/// Body of the command. Pure projection of `AppState` fields.
///
/// # Errors
/// Currently never returns an error; signature uses `Result` to keep the
/// future-compatible shape Tauri commands expect.
pub fn app_info(state: &crate::app::AppState) -> Result<AppInfo> {
    Ok(AppInfo {
        version: crate::display_version(),
        history_path: state.history_path.display().to_string(),
        proxy_base_url: state.proxy_origin.base.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn app_info_advertises_no_bundled_script() {
        // The app resolves natively; there is no script to point at,
        // and a path here would send a reader looking for a component
        // that no longer ships.
        let json = serde_json::to_value(app_info(&fake_state()).expect("info")).expect("json");
        let obj = json.as_object().expect("object");
        assert!(
            !obj.contains_key("ani_cli_path"),
            "app-info still advertises a bundled script: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
    }

    fn fake_state() -> AppState {
        AppState {
            anidb_base: None,
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            meta_http: reqwest::Client::new(),
            proxy_origin: ProxyOrigin::new("127.0.0.1", 42_337),
            bundled_bin: None,
            legacy_sweep: crate::legacy_script::SweepReport::default(),
            history_path: PathBuf::from("/home/u/.local/state/ani-gui/history"),
            anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
            image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
            config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
            state_dir: PathBuf::from("/tmp/ani-gui-state"),
            internal_secret: crate::account::InternalSecret::random(),
            mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
            account_write_locks: crate::commands::account::AccountWriteLocks::new(),
            availability_refreshes:
                crate::commands::availability_refresh::AvailabilityRefreshes::new(),
        }
    }

    #[test]
    fn app_info_reports_what_the_boot_sweep_removed() {
        // The sweep runs once, during backend boot, on installs that
        // carried the copy an earlier version maintained. Reporting it
        // here is what makes the removal visible rather than silent:
        // the diagnostics page reads app-info and has no other channel
        // to learn a file was deleted out from under the user.
        let mut state = fake_state();
        state.legacy_sweep = crate::legacy_script::SweepReport {
            removed: vec![PathBuf::from("/home/u/.cache/ani-gui/ani-cli")],
        };
        let info = app_info(&state).expect("info");
        assert_eq!(
            info.removed_legacy_paths,
            vec!["/home/u/.cache/ani-gui/ani-cli".to_string()]
        );
    }

    #[test]
    fn an_empty_sweep_reports_an_empty_list_not_an_absent_one() {
        // Every launch after the first, and every install that never
        // ran one of those versions. The field is present and empty so
        // the page renders nothing, rather than absent so it breaks.
        let info = app_info(&fake_state()).expect("info");
        assert!(info.removed_legacy_paths.is_empty());
    }

    #[test]
    fn app_info_projects_state_fields() {
        let s = fake_state();
        let info = app_info(&s).unwrap();
        // Robust to the dev profile's `-dev` suffix (and any parallel test
        // toggling ANI_GUI_DEV): the version always starts with the crate
        // version.
        assert!(info.version.starts_with(crate::VERSION));
        assert_eq!(info.history_path, "/home/u/.local/state/ani-gui/history");
        assert_eq!(info.proxy_base_url, "http://127.0.0.1:42337");
    }

    #[test]
    fn app_info_serializes_with_snake_case_unchanged() {
        let s = fake_state();
        let info = app_info(&s).unwrap();
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"version\""));
        assert!(json.contains("\"history_path\""));
        assert!(json.contains("\"proxy_base_url\""));
    }
}
