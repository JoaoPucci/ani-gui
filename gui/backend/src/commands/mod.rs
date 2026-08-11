//! Plain command bodies the HTTP API in [`crate::api`] mounts as routes.
//!
//! Each function returns `Result<T, AniError>` so the API can map errors
//! to HTTP status codes + a JSON body that carries a stable i18n key
//! (see [`crate::i18n::keys`]). No command ever returns a localized
//! string — the frontend owns user-facing copy.

pub mod account;
pub mod account_edit;
pub mod airing;
pub mod anidb_offset;
pub mod anilist_eps_thumbs;
pub mod aniskip;
pub mod app_info;
pub mod availability;
mod availability_mode;
pub mod availability_refresh;
pub mod cour;
pub mod download;
mod download_range;
pub(crate) mod download_tool;
pub mod external_player;
pub mod history;
pub mod kitsu;
pub mod kitsu_warm;
pub mod play;
pub mod play_args;
pub mod play_cache;
pub mod play_external_command;
pub mod play_handoff;
#[cfg(test)]
#[path = "play_handoff_test.rs"]
mod play_handoff_test;
pub mod play_native;
mod play_native_choice;
pub mod play_native_episode;
pub mod play_native_format;
pub mod play_native_numbering;
pub mod play_native_outcome;
pub mod play_native_resolve;
#[cfg(test)]
pub(crate) mod play_native_test_provider;
pub mod play_native_walk;
pub mod play_native_year;
pub mod play_resolution_cache;
pub mod play_syncplay;
pub mod progress;
pub mod proxy_url;
pub mod session;
pub mod settings;
pub mod syncplay;

pub use app_info::app_info;
pub use external_player::{open_external_player, LaunchArgs};
pub use history::{history_clear, history_list};
pub use proxy_url::proxy_base_url;
pub use session::{
    create_session, create_session_with_kind, CreateSessionArgs, CreateSessionResponse,
};
