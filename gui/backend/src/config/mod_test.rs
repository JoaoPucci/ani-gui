//! Tests for `crate::config`. Extracted via `#[path]` so the inline
//! `#[cfg(test)]` module's complexity doesn't pile onto `mod.rs`'s CCN
//! budget — per `project_crap_inline_test_gotcha`.

use super::*;

#[test]
fn defaults_match_ani_cli_defaults() {
    let c = Config::default();
    assert_eq!(c.mode, "sub");
    assert_eq!(c.quality, "best");
    assert_eq!(c.external_player, "mpv");
}

#[test]
fn auto_play_next_defaults_to_false() {
    // The toggle must be opt-in. Existing users (no field in their
    // config.toml) shouldn't suddenly find episodes auto-advancing
    // after an upgrade.
    assert!(!Config::default().auto_play_next);
}

#[test]
fn auto_skip_defaults_to_false() {
    // Same opt-in rationale: existing users shouldn't suddenly
    // lose the OP/ED on upgrade. Many fans actively want to
    // hear the OP at least the first time.
    let c = Config::default();
    assert!(!c.auto_skip_op);
    assert!(!c.auto_skip_ed);
}

#[test]
fn use_custom_player_controls_defaults_to_true() {
    // The native Chromium controls bar is functional but plain;
    // the custom overlay carries the per-show accent color, keeps
    // the Skip OP/Outro overlay visible during fullscreen, and is
    // the chrome the M3 design direction (§7) actually targets.
    // Native is the strictly inferior option for ani-gui's
    // top-priority UI surface, so a fresh install should pick the
    // designed experience by default. Existing users who already
    // wrote `use_custom_player_controls = false` keep that —
    // serde's deserializer respects the file, this default only
    // covers the absent-field case (fresh install / upgraded user
    // whose config predates the field landing).
    assert!(Config::default().use_custom_player_controls);
}

#[test]
fn disable_auto_pip_on_leave_defaults_to_true() {
    // Auto-PiP-on-navigate is surprising default behaviour: the
    // user clicks back, the video doesn't stop, and a small floating
    // window follows them around the OS. The discoverability of
    // PiP isn't worth the surprise — most users hit Back expecting
    // playback to halt. Flip the default ON so a fresh install
    // pauses on navigate; users who actively want PiP can toggle
    // it back off in settings. Existing users who already wrote
    // `disable_auto_pip_on_leave = false` keep that — serde's
    // deserializer respects the file, this default only covers the
    // absent-field case.
    assert!(Config::default().disable_auto_pip_on_leave);
}

#[test]
fn auto_play_next_round_trips_through_toml() {
    let c = Config {
        auto_play_next: true,
        ..Config::default()
    };
    let s = toml::to_string(&c).unwrap();
    let parsed: Config = toml::from_str(&s).unwrap();
    assert!(parsed.auto_play_next);
}

#[test]
fn auto_play_next_absent_in_old_config_decodes_as_false() {
    // Pre-existing config.toml files don't have this field. Thanks
    // to #[serde(default)] on the struct they should still parse,
    // with the missing field defaulting to false.
    let body = "mode = \"sub\"\nquality = \"best\"\n";
    let cfg: Config = toml::from_str(body).unwrap();
    assert!(!cfg.auto_play_next);
}

#[test]
fn round_trips_through_toml() {
    let c = Config {
        locale: "pt-BR".into(),
        ..Config::default()
    };
    let s = toml::to_string(&c).unwrap();
    let parsed: Config = toml::from_str(&s).unwrap();
    assert_eq!(c, parsed);
}

#[test]
fn syncplay_binary_round_trips_through_toml() {
    // User can override the binary path in settings (Windows
    // users often install Syncplay outside PATH). The override
    // must survive a TOML round-trip or the user's choice resets
    // every launch.
    let c = Config {
        syncplay_binary: "/opt/syncplay/syncplay".into(),
        ..Config::default()
    };
    let s = toml::to_string(&c).unwrap();
    let parsed: Config = toml::from_str(&s).unwrap();
    assert_eq!(parsed.syncplay_binary, "/opt/syncplay/syncplay");
}

#[test]
fn syncplay_binary_absent_in_old_config_decodes_with_default() {
    // Pre-Syncplay-feature config.toml files don't have this
    // field — they must decode with the per-OS default so
    // existing users don't get a sudden "missing setting" error.
    let body = "external_player = \"mpv\"\n";
    let cfg: Config = toml::from_str(body).unwrap();
    assert_eq!(cfg.syncplay_binary, default_syncplay_binary());
}

#[test]
fn primary_account_defaults_to_empty() {
    // No explicit choice on a fresh install; the UI falls back to
    // its AniList-first precedence rather than forcing a provider.
    assert_eq!(Config::default().primary_account, "");
}

#[test]
fn primary_account_round_trips_through_toml() {
    let c = Config {
        primary_account: "mal".into(),
        ..Config::default()
    };
    let s = toml::to_string(&c).unwrap();
    let parsed: Config = toml::from_str(&s).unwrap();
    assert_eq!(parsed.primary_account, "mal");
}

#[test]
fn primary_account_absent_in_old_config_decodes_as_empty() {
    // Pre-picker config.toml files don't have this field; serde's
    // struct-level default must decode them with an empty choice.
    let body = "mode = \"sub\"\nquality = \"best\"\n";
    let cfg: Config = toml::from_str(body).unwrap();
    assert_eq!(cfg.primary_account, "");
}

#[test]
fn external_player_kind_round_trips_through_toml() {
    // The kind picker in settings persists this value to disk; if it
    // doesn't survive a TOML round-trip, the user's choice resets
    // every launch.
    use crate::commands::external_player::ExternalPlayerKind;
    let c = Config {
        external_player_kind: ExternalPlayerKind::Vlc,
        ..Config::default()
    };
    let s = toml::to_string(&c).unwrap();
    let parsed: Config = toml::from_str(&s).unwrap();
    assert_eq!(parsed.external_player_kind, ExternalPlayerKind::Vlc);
}

#[test]
fn external_player_kind_absent_in_old_config_decodes_as_mpv() {
    // Pre-existing config.toml files don't have this field — they
    // must decode with the default Mpv kind so existing users don't
    // get a sudden behaviour change on upgrade.
    use crate::commands::external_player::ExternalPlayerKind;
    let body = "external_player = \"mpv\"\n";
    let cfg: Config = toml::from_str(body).unwrap();
    assert_eq!(cfg.external_player_kind, ExternalPlayerKind::Mpv);
}

#[test]
fn external_player_custom_args_round_trips_through_toml() {
    // The Custom kind needs the args template to survive disk
    // round-trips — otherwise picking Custom and writing a template
    // resets every launch.
    let c = Config {
        external_player_custom_args: "--ref={referer} --title={title} {url}".into(),
        ..Config::default()
    };
    let s = toml::to_string(&c).unwrap();
    let parsed: Config = toml::from_str(&s).unwrap();
    assert_eq!(
        parsed.external_player_custom_args,
        "--ref={referer} --title={title} {url}"
    );
}

#[test]
fn external_player_custom_args_defaults_to_empty_string() {
    // A fresh install has nothing to spawn with under Custom — the
    // empty default is the trigger for build_argv_custom's bare-URL
    // fallback.
    let c = Config::default();
    assert!(c.external_player_custom_args.is_empty());
}

#[test]
fn read_config_returns_defaults_when_file_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nope.toml");
    let c = read_config(&path).expect("ok");
    assert_eq!(c, Config::default());
}

#[test]
fn write_then_read_round_trips_through_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("config.toml");
    let c = Config {
        mode: "dub".into(),
        quality: "1080".into(),
        external_player: "vlc".into(),
        ..Config::default()
    };
    write_config(&path, &c).expect("write");
    let back = read_config(&path).expect("read");
    assert_eq!(back, c);
}

#[test]
fn write_config_is_atomic_via_temp_rename() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    write_config(&path, &Config::default()).unwrap();
    // The .new sidecar should not survive the write.
    let sidecar = path.with_extension("toml.new");
    assert!(!sidecar.exists(), "atomic-rename leaves no .new behind");
}

#[test]
fn read_config_rejects_non_toml_body() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "this is not toml: [[[").unwrap();
    let r = read_config(&path);
    assert!(matches!(r, Err(AniError::Config)));
}
