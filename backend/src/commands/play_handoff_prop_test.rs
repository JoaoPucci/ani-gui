//! Property coverage for the pure mapping a handoff's launch is
//! built from.

use super::play::PlayArgs;

proptest::proptest! {
    /// The launch carries the resolve and the settings field for
    /// field: the stream and the referer as the resolve gave them
    /// (none staying none), the title composed from the request, and
    /// the player command, kind and custom-argument template as
    /// configured.
    #[test]
    fn the_launch_carries_the_resolve_and_the_settings_field_for_field(
        master_url in "https://[a-z]{1,10}\\.[a-z]{2,4}/[a-z0-9/._-]{0,30}",
        referer in proptest::option::of("https://[a-z]{1,10}\\.[a-z]{2,4}/"),
        show_title in "[A-Za-z0-9 ,:'!-]{1,30}",
        episode in "[0-9]{1,4}",
        player in "[a-z]{1,10}",
        kind in proptest::sample::select(vec![
            crate::commands::external_player::ExternalPlayerKind::Mpv,
            crate::commands::external_player::ExternalPlayerKind::Vlc,
            crate::commands::external_player::ExternalPlayerKind::Iina,
            crate::commands::external_player::ExternalPlayerKind::Custom,
        ]),
        custom in "[^\\x00]{0,30}",
    ) {
        let resolved = crate::commands::play_native_resolve::NativeResolved {
            slug: "the-show-77".into(),
            title: "The Show".into(),
            master_url: master_url.clone(),
            episode_cap: None,
            numbering_offset: 0,
            extra_tags: Vec::new(),
            resolved_slot: 1,
            resolved_tag: None,
            referer: referer.clone(),
        };
        let args: PlayArgs = serde_json::from_value(serde_json::json!({
            "title": show_title,
            "episode": episode,
            "mode": "sub",
        }))
        .expect("args");
        let cfg = crate::config::Config {
            external_player: player.clone(),
            external_player_kind: kind,
            external_player_custom_args: custom.clone(),
            ..crate::config::Config::default()
        };
        let launch = super::play_handoff::launch_args_for(resolved, &args, &cfg);
        proptest::prop_assert_eq!(launch.stream_url, master_url);
        proptest::prop_assert_eq!(launch.referer, referer);
        proptest::prop_assert_eq!(launch.title, Some(format!("{show_title} · ep {episode}")));
        proptest::prop_assert_eq!(launch.player_command, player);
        proptest::prop_assert_eq!(launch.player_kind, kind);
        proptest::prop_assert_eq!(launch.custom_args_template, Some(custom));
    }
}
