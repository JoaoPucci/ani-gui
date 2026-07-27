#!/usr/bin/env bats
#
# Unit tests for ani-cli's `select_quality` (lines 194-214).
#
# Contract (input via globals, output via globals):
#   Inputs:  $links (newline-separated "WIDTH >URL" lines), $player_function.
#   Outputs: $episode (the chosen URL), optionally $subs_flag, $refr_flag,
#            $subtitle, $m3u8_refr (set/unset based on link kind).
#
# Quality argument:
#   - "best"  → first line of $links (already sorted descending in production).
#   - "worst" → last line matching `^[0-9]{3,4}` (numeric quality only).
#   - other   → first line matching the literal string.
#   - not found → falls back to best, prints a warning to stderr.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
    # Reset all globals select_quality interacts with
    unset episode subs_flag refr_flag subtitle m3u8_refr
    player_function='mpv'
}

@test "select_quality: 'best' picks the first link" {
    links=$'1080 >https://a.example/1080.mp4\n720 >https://a.example/720.mp4\n480 >https://a.example/480.mp4'
    select_quality "best" || true
    [ "$episode" = "https://a.example/1080.mp4" ]
}

@test "select_quality: 'worst' picks the last numeric-quality link" {
    links=$'1080 >https://a.example/1080.mp4\n720 >https://a.example/720.mp4\n480 >https://a.example/480.mp4'
    select_quality "worst"
    [ "$episode" = "https://a.example/480.mp4" ]
}

@test "select_quality: explicit '1080' picks the 1080 link" {
    links=$'1080 >https://a.example/1080.mp4\n720 >https://a.example/720.mp4\n480 >https://a.example/480.mp4'
    select_quality "1080"
    [ "$episode" = "https://a.example/1080.mp4" ]
}

@test "select_quality: explicit '720' picks the 720 link" {
    links=$'1080 >https://a.example/1080.mp4\n720 >https://a.example/720.mp4\n480 >https://a.example/480.mp4'
    select_quality "720"
    [ "$episode" = "https://a.example/720.mp4" ]
}

@test "select_quality: not-found falls back to best with stderr warning" {
    links=$'1080 >https://a.example/1080.mp4\n480 >https://a.example/480.mp4'
    # A command substitution would run select_quality in a subshell and
    # lose the $episode global; capture stderr via file instead.
    select_quality "9999" 2>"$BATS_TEST_TMPDIR/warn" || true
    [ "$episode" = "https://a.example/1080.mp4" ]
    grep -q "Specified quality not found" "$BATS_TEST_TMPDIR/warn"
}

@test "select_quality: vlc gets the same unfiltered list as every player (4.15)" {
    # Pre-4.15 stripped cc>/subtitle/refr metadata lines for vlc;
    # 4.15 removed that metadata mechanism entirely, so the first
    # (highest) link wins exactly as for mpv.
    player_function='vlc'
    allanime_refr='https://allmanga.to'
    links=$'1080 >https://a.example/1080.m3u8\n720 >https://a.example/720.mp4'
    select_quality "best" || true
    [ "$episode" = "https://a.example/1080.m3u8" ]
}

@test "select_quality: m3u8 link picks with the allanime_refr fallback (4.15)" {
    # 4.15 dropped the cc>/subtitle metadata: subs_flag is never set
    # here (hardsubs ride the HLS variant), and refr_flag comes from
    # the provider dispatch — default falls back to allanime_refr.
    player_function='mpv'
    allanime_refr='https://allmanga.to'
    unset subs_flag
    links=$'1080 >https://a.example/1080.m3u8'
    select_quality "best" || true
    [ "$episode" = "https://a.example/1080.m3u8" ]
    [ -z "${subs_flag-}" ]
    [ "$refr_flag" = "--referrer=https://allmanga.to" ]
}

@test "select_quality: tools.fast4speed link sets refr_flag to allanime_refr" {
    player_function='mpv'
    allanime_refr='https://allmanga.to'
    links=$'1080 >https://tools.fast4speed.rsvp/path'
    select_quality "best" || true
    [ "$episode" = "https://tools.fast4speed.rsvp/path" ]
    [ "$refr_flag" = "--referrer=https://allmanga.to" ]
}

@test "select_quality: mp4 (no cc>) leaves subs_flag/refr_flag unset" {
    player_function='mpv'
    allanime_refr='https://allmanga.to'
    unset subs_flag
    links=$'1080 >https://a.example/1080.mp4\n720 >https://a.example/720.mp4'
    select_quality "best" || true
    [ "$episode" = "https://a.example/1080.mp4" ]
    [ -z "${subs_flag-}" ]
    # 4.15: refr_flag is always populated (allanime_refr fallback),
    # mp4 or not — the old "unset for mp4" contract is gone.
    [ "$refr_flag" = "--referrer=https://allmanga.to" ]
}
