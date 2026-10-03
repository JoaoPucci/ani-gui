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
mod availability_reschedule;
pub(crate) mod availability_ttl;
pub mod cour;
mod cour_keyword_form;
mod cour_ordinal_form;
pub mod download;
mod download_names;

#[cfg(test)]
#[path = "download_lock_prop_test.rs"]
mod download_lock_prop_tests;
mod download_pacing;
mod download_progress;
mod download_range;
pub(crate) mod download_tool;
mod download_tool_output;
mod download_transfer;
pub mod external_player;
pub mod history;
pub(crate) mod history_claim;
pub(crate) mod history_forget;
pub(crate) mod history_forget_resolutions;
pub(crate) mod history_forget_skips;
pub(crate) mod history_forget_titles;
mod history_remove;
mod history_resume;
pub(crate) mod history_title_tail;
pub mod kitsu;
pub(crate) mod kitsu_gone;
pub(crate) mod kitsu_played;
pub(crate) mod kitsu_title_words;
pub mod kitsu_warm;
pub mod play;
pub mod play_args;
pub mod play_cache;
pub(crate) mod play_cache_tracks;
pub mod play_external_command;
pub mod play_handoff;
#[cfg(test)]
#[path = "play_handoff_prop_test.rs"]
mod play_handoff_prop_test;
#[cfg(test)]
#[path = "play_handoff_test.rs"]
mod play_handoff_test;
pub mod play_native;
mod play_native_choice;
pub mod play_native_episode;
pub mod play_native_format;
mod play_native_merge;
pub mod play_native_numbering;
pub mod play_native_outcome;
mod play_native_part_title;
pub(crate) mod play_native_record;
pub mod play_native_resolve;
mod play_native_split;
#[cfg(test)]
pub(crate) mod play_native_test_provider;
pub mod play_native_walk;
pub mod play_native_year;
pub mod play_resolution_cache;
pub mod play_syncplay;
pub mod progress;
pub mod providers;
pub mod proxy_url;
pub mod session;
pub mod settings;
pub mod syncplay;
pub(crate) mod title_match_store;

pub use app_info::app_info;
pub use external_player::{open_external_player, LaunchArgs};
pub use history::{history_clear, history_list};
pub use proxy_url::proxy_base_url;
pub use session::{
    create_session, create_session_with_kind, CreateSessionArgs, CreateSessionResponse,
};

#[cfg(test)]
#[path = "kitsu_id_compare_test.rs"]
mod kitsu_id_compare_test;
