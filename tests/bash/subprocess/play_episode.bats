#!/usr/bin/env bats
#
# Tests for ani-cli's `play_episode` (5.0).
#
# Contract:
#   - Reads globals: player_function, video_link, links, ep_no,
#     anime_id, anime_title, no_detach, exit_after_play, skip_intro,
#     log_episode, canon_ep_no, mal_id, player_extra_flags,
#     menu_program.
#   - Branches on player_function and invokes the right player.
#   - Saves $video_link into $replay, unsets it, and updates history.
#
# Every external command a branch might call is stubbed; assertions
# check which one ran with which args.

load '../helpers/loader'

setup() {
    # History must be redirected to a tmp dir BEFORE sourcing — the
    # script initializes $histfile during its setup block.
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    source_ani_cli_lib
    # shellcheck source=/dev/null
    load '../helpers/process_stub'
    stub_setup
    stub_command nohup mpv vlc flatpak am catt logger ani-skip syncplay yt-dlp ffmpeg

    ep_no='1'
    canon_ep_no='1'
    anime_id='one-piece-69'
    anime_title='One Piece'
    video_link='https://cdn.example/op/1080/index.m3u8'
    links=$'1080p >https://cdn.example/op/1080/index.m3u8\n720p >https://cdn.example/op/720/index.m3u8'
    skip_intro=0
    log_episode=0
    no_detach=0
    exit_after_play=0
    player_extra_flags=''
    menu_program='fzf'
    _range=''
}

@test "play_episode: mpv default branch invokes nohup mpv with --force-media-title" {
    player_function='mpv'
    play_episode || true
    wait 2>/dev/null || true
    stub_assert_called nohup '.*mpv.*--force-media-title=One Piece Episode 1.*https://cdn.example/op/1080/index.m3u8'
}

@test "play_episode: mpv with no_detach=1 calls mpv synchronously (no nohup)" {
    player_function='mpv'
    no_detach=1
    play_episode || true
    wait 2>/dev/null || true
    stub_assert_called mpv '.*--force-media-title=One Piece Episode 1.*https://cdn.example/op/1080/index.m3u8'
    stub_assert_not_called nohup
}

@test "play_episode: vlc branch passes --play-and-exit and the meta title" {
    player_function='vlc'
    play_episode || true
    wait 2>/dev/null || true
    stub_assert_called nohup '.*vlc.*--play-and-exit.*'
    stub_assert_called nohup '.*--meta-title=One Piece Episode 1.*'
}

@test "play_episode: the flatpak directory branch runs flatpak io.mpv.Mpv" {
    player_function="$HOME/.local/share/flatpak/app/io.mpv/Mpv/"
    play_episode || true
    wait 2>/dev/null || true
    stub_assert_called flatpak '.*run io.mpv.Mpv.*--force-media-title=One Piece Episode 1.*https://cdn.example/op/1080/index.m3u8'
}

@test "play_episode: catt branch invokes catt cast" {
    player_function='catt'
    play_episode || true
    wait 2>/dev/null || true
    stub_assert_called nohup '.*catt cast.*https://cdn.example/op/1080/index.m3u8'
}

@test "play_episode: android_mpv branch invokes am start with MPVActivity" {
    player_function='android_mpv'
    play_episode || true
    wait 2>/dev/null || true
    stub_assert_called nohup '.*am start.*is.xyz.mpv/.MPVActivity.*'
}

@test "play_episode: android_vlc branch invokes am start with VideoPlayerActivity" {
    player_function='android_vlc'
    play_episode || true
    wait 2>/dev/null || true
    stub_assert_called nohup '.*am start.*org.videolan.vlc/org.videolan.vlc.gui.video.VideoPlayerActivity.*'
}

@test "play_episode: debug branch prints links and selected URL, no players" {
    player_function='debug'
    output=$(play_episode 2>/dev/null || true)
    [[ "$output" == *"All links:"* ]]
    [[ "$output" == *"1080p >https://cdn.example/op/1080/index.m3u8"* ]]
    [[ "$output" == *"Selected link:"* ]]
    [[ "$output" == *"https://cdn.example/op/1080/index.m3u8"* ]]
    stub_assert_not_called nohup
    stub_assert_not_called mpv
}

@test "play_episode: updates the history file with ep_no/anime_id/title" {
    player_function='mpv'
    play_episode || true
    wait 2>/dev/null || true
    grep -F "1"$'\t'"one-piece-69"$'\t'"One Piece" "$histfile" >/dev/null
}

@test "play_episode: stores the played link for replay and clears video_link" {
    player_function='mpv'
    play_episode || true
    wait 2>/dev/null || true
    [ "$replay" = "https://cdn.example/op/1080/index.m3u8" ]
    [ -z "${video_link-}" ]
}

@test "play_episode: log_episode=1 invokes logger with the title and ep_no" {
    player_function='mpv'
    log_episode=1
    play_episode || true
    wait 2>/dev/null || true
    stub_assert_called logger '.*-t ani-cli One Piece 1'
}

@test "play_episode: skip_intro=1 consults ani-skip with the MAL id" {
    player_function='mpv'
    skip_intro=1
    mal_id='21'
    play_episode || true
    wait 2>/dev/null || true
    stub_assert_called ani-skip '.*-i 21.*-e 1.*'
}
