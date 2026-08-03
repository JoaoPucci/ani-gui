#!/usr/bin/env bats
#
# Acceptance test: end-to-end "search → episode → play" pipeline.
#
# Runs the real ./ani-cli with ANI_CLI_PLAYER=debug and curl shimmed on
# PATH; the shim serves tests/fixtures/anidb/ by URL pattern. The
# script should walk browse → episodes → languages → embed → master
# playlist and print the resolved stream URL in the debug branch.
#
# The shim is installed under both `curl` and `curl_firefox135`:
# 5.0 prefers curl-impersonate binaries when present, and the
# impersonate name comes first in its failover list, so a developer
# machine with the real thing would otherwise route around the shim.

load '../helpers/loader'

setup() {
    export ANI_CLI_HIST_DIR="$BATS_TEST_TMPDIR/hist"
    mkdir -p "$ANI_CLI_HIST_DIR"
    export ANI_CLI_PLAYER='debug'
    export CURL_FIXTURE_DIR="$FIXTURES_DIR/anidb"

    export PATH_SHIM="$BATS_TEST_TMPDIR/bin"
    mkdir -p "$PATH_SHIM"
    cp "$REPO_ROOT/tests/bash/helpers/curl_shim.sh" "$PATH_SHIM/curl"
    cp "$REPO_ROOT/tests/bash/helpers/curl_shim.sh" "$PATH_SHIM/curl_firefox135"
    chmod +x "$PATH_SHIM/curl" "$PATH_SHIM/curl_firefox135"
    export PATH="$PATH_SHIM:$PATH"
}

@test "search-and-play (debug player): prints 'All links:' and 'Selected link:' with the 1080p URL" {
    run "$ANI_CLI_PATH" -S 1 -e 1 -q best "one piece"
    [ "$status" -eq 0 ]
    [[ "$output" == *"All links:"* ]]
    [[ "$output" == *"Selected link:"* ]]
    [[ "$output" == *"1080p >https://cdn.example/op/1080/index.m3u8"* ]]
    [[ "$output" == *"https://cdn.example/op/1080/index.m3u8"* ]]
}

@test "search-and-play: writes the played episode to the history file under the anidb slug" {
    run "$ANI_CLI_PATH" -S 1 -e 1 -q best "one piece"
    [ "$status" -eq 0 ]
    histfile="$ANI_CLI_HIST_DIR/ani-hsts"
    [ -s "$histfile" ]
    grep -F "1"$'\t'"one-piece-69"$'\t'"One Piece" "$histfile" >/dev/null
}

@test "continue-from-history (-c): replays the next episode for an in-progress entry" {
    histfile="$ANI_CLI_HIST_DIR/ani-hsts"
    printf '1\tone-piece-69\tOne Piece\n' >"$histfile"
    run "$ANI_CLI_PATH" -c -S 1 -q best
    [ "$status" -eq 0 ]
    [[ "$output" == *"Selected link:"* ]]
    grep -F "2"$'\t'"one-piece-69" "$histfile" >/dev/null
}

@test "continue-from-history (-c): backs pre-5.0 rows up to ani-hsts.v4 first" {
    # 5.0 keys history on anidb slugs; provider-native ids from 4.x
    # (no hyphen) move aside once so they are never misread as slugs.
    histfile="$ANI_CLI_HIST_DIR/ani-hsts"
    {
        printf '3\tReooPAxPMsHM4KPMY\tOne Piece (1122 episodes)\n'
        printf '1\tone-piece-69\tOne Piece\n'
    } >"$histfile"
    run "$ANI_CLI_PATH" -c -S 1 -q best
    [ "$status" -eq 0 ]
    grep -F "ReooPAxPMsHM4KPMY" "${histfile}.v4" >/dev/null
    run ! grep -F "ReooPAxPMsHM4KPMY" "$histfile"
    grep -F "one-piece-69" "$histfile" >/dev/null
}

@test "search-and-play with --dub: resolves through the eng embed without aborting" {
    run "$ANI_CLI_PATH" --dub -S 1 -e 1 -q best "one piece"
    [ "$status" -eq 0 ]
    [[ "$output" == *"Selected link:"* ]]
}

@test "no results: an empty browse page dies 'No results found!'" {
    run "$ANI_CLI_PATH" -S 1 -e 1 "nohit"
    [ "$status" -eq 1 ]
    [[ "$output" == *"No results found"* ]]
}

@test "cloudflare interstitial dies 'Blocked by cloudflare'" {
    run "$ANI_CLI_PATH" -S 1 -e 1 "cloudflare"
    [ "$status" -eq 1 ]
    [[ "$output" == *"Blocked by cloudflare"* ]]
}

@test "download-flag: -d resolves the stream and hands it to yt-dlp" {
    cat >"$PATH_SHIM/yt-dlp" <<EOF
#!/bin/sh
printf 'yt-dlp %s\n' "\$*" >"$BATS_TEST_TMPDIR/yt-dlp.log"
exit 0
EOF
    chmod +x "$PATH_SHIM/yt-dlp"
    run "$ANI_CLI_PATH" -d -S 1 -e 1 -q best "one piece"
    [ "$status" -eq 0 ]
    [ -f "$BATS_TEST_TMPDIR/yt-dlp.log" ]
    grep -q 'https://cdn.example/op/1080/index.m3u8' "$BATS_TEST_TMPDIR/yt-dlp.log"
}
