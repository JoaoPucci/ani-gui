#!/usr/bin/env bats
#
# Unit tests covering the new code paths upstream landed in pystardust/ani-cli
# commit b8032b7 (mp4upload provider, refactor):
#
#   - select_quality's new per-provider refr_flag dispatch
#     (mp4upload sets a literal mp4upload.com referer; sharepoint unsets
#     refr_flag; everything else falls back to allanime_refr).
#   - cleanup() — new top-level helper invoked on SIGINT and normal exit.
#   - process_response() — replaces the old decode_tobeparsed entrypoint;
#     non-encrypted responses short-circuit through a pass-through path.
#   - provider_init's new "raw provider_id" branch — when the matched line
#     does NOT start with "--", the hex-decode pipeline is skipped and the
#     raw value is used directly.
#
# These tests pin the new behavior so future upstream syncs that touch the
# same surface flag intentional changes.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    unset episode subs_flag refr_flag subtitle m3u8_refr
    player_function='mpv'
}

@test "select_quality: mp4upload result sets the literal mp4upload.com referer" {
    allanime_refr='https://allmanga.to'
    links=$'1080 >https://example.mp4upload.com/v/abc'
    select_quality "best"
    [ "$episode" = "https://example.mp4upload.com/v/abc" ]
    [ "$refr_flag" = "--referrer=https://www.mp4upload.com" ]
}

@test "select_quality: sharepoint result unsets refr_flag entirely" {
    allanime_refr='https://allmanga.to'
    refr_flag='--referrer=stale-from-prior-call'
    links=$'1080 >https://my.sharepoint.com/x/y/z.mp4'
    select_quality "best"
    [ "$episode" = "https://my.sharepoint.com/x/y/z.mp4" ]
    [ -z "${refr_flag-}" ]
}

@test "select_quality: default provider falls back to allanime_refr" {
    allanime_refr='https://allmanga.to'
    links=$'1080 >https://wixmp-cdn.example/v.mp4'
    select_quality "best"
    [ "$episode" = "https://wixmp-cdn.example/v.mp4" ]
    [ "$refr_flag" = "--referrer=https://allmanga.to" ]
}

@test "cleanup: removes histfile.new and emits the SGR reset sequence" {
    tmp_hist="$(mktemp)"
    histfile="$tmp_hist"
    : >"${histfile}.new"
    [ -f "${histfile}.new" ]
    output=$(cleanup)
    [ ! -f "${histfile}.new" ]
    # The function prints exactly one ANSI reset sequence (`\033[0m`) to
    # stdout — no trailing newline.
    printf '%s' "$output" | grep -q $'\033\[0m'
    rm -f "$tmp_hist"
}

@test "cleanup: android_mpv player path truncates the mpv config bridge file" {
    # The android_mpv branch writes to a fixed Termux path
    # (/storage/emulated/0/mpv/mpv.config.mp4) that does not exist on a CI
    # runner. We only assert that cleanup() does not error when the path is
    # unwritable — the `:` builtin's redirect failure must be tolerated by
    # the surrounding `&&` chain so the function still proceeds to the
    # color reset and histfile cleanup.
    tmp_hist="$(mktemp)"
    histfile="$tmp_hist"
    player_function='android_mpv'
    : >"${histfile}.new"
    run cleanup
    [ "$status" -eq 0 ]
    [ ! -f "${histfile}.new" ]
    rm -f "$tmp_hist"
}

@test "process_response: non-tobeparsed input passes through unchanged" {
    payload='{"data":{"episode":{"sourceUrls":[{"sourceUrl":"--abc","sourceName":"wixmp"}]}}}'
    output=$(process_response "$payload")
    [ "$output" = "$payload" ]
}

@test "process_response: empty input returns empty (non-tobeparsed path)" {
    output=$(process_response "")
    [ -z "$output" ]
}

@test "provider_init: takes provider_id raw when the matched value lacks the -- prefix" {
    # Old behavior: every matched line was assumed to be `--<hex-encoded>`
    # and run through the hex-to-ascii decode table. The new behavior in
    # upstream b8032b7 only triggers that decode when the value starts
    # with `--`; otherwise the value is taken as-is, which is what the
    # mp4upload and youtube providers now deliver.
    resp=$'/Mp4 :https://example.mp4upload.com/v/raw123\n/Other :rest'
    provider_init "mp4upload" "/Mp4 :/p"
    [ "$provider_name" = "mp4upload" ]
    [ "$provider_id" = "https://example.mp4upload.com/v/raw123" ]
}
