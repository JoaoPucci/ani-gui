#!/usr/bin/env bats
#
# Tests for ani-cli's `update_script` (5.0).
#
# Contract:
#   - $curl_exe GETs raw.githubusercontent.com/pystardust/ani-cli/<branch>/ani-cli
#     with --fail-with-body.
#   - On fetch failure: die "[branch:…] Connection error" (exit 1).
#   - A response without a version_number line: die "Invalid response."
#     (a CDN error page must never be diffed into the script).
#   - Diff the response against $0. No diff → "Script is up to date :)".
#   - Otherwise pipe the diff through `patch "$0" -`:
#       - success → "Updated: <old> -> <new>"
#       - failure → die "Failed to update".
#   - Exits 0 on success.
#
# Tests use `run bash -c '…'` so the function's exit doesn't kill the
# bats process. A tmp copy of ani-cli is passed as $0 so `patch "$0"`
# touches the tmp, never the vendored script. $curl_exe is pointed at a
# mock function.

load '../helpers/loader'

@test "update_script: up-to-date prints 'Script is up to date'" {
    tmp_script=$(mktemp)
    cp "$ANI_CLI_PATH" "$tmp_script"
    run bash -c '
        __ANI_CLI_LIB__=1 . "$0" 2>/dev/null
        trap - ERR; set +eE
        remote() { cat "$0"; }
        curl_exe=remote
        update_script
    ' "$tmp_script"
    rm -f "$tmp_script"
    [ "$status" -eq 0 ]
    [[ "$output" == *"up to date"* ]]
}

@test "update_script: with upstream changes calls patch and prints the version move" {
    tmp_script=$(mktemp)
    cp "$ANI_CLI_PATH" "$tmp_script"
    run bash -c '
        __ANI_CLI_LIB__=1 . "$0" 2>/dev/null
        trap - ERR; set +eE
        remote() { cat "$0"; printf "%s\n" "# upstream-added line"; }
        curl_exe=remote
        patch() { cat >/dev/null; return 0; }
        update_script
    ' "$tmp_script"
    rm -f "$tmp_script"
    [ "$status" -eq 0 ]
    [[ "$output" == *"Updated:"* ]]
}

@test "update_script: failed patch dies with 'Failed to update'" {
    tmp_script=$(mktemp)
    cp "$ANI_CLI_PATH" "$tmp_script"
    run bash -c '
        __ANI_CLI_LIB__=1 . "$0" 2>/dev/null
        trap - ERR; set +eE
        remote() { cat "$0"; printf "%s\n" "# extra"; }
        curl_exe=remote
        patch() { cat >/dev/null; return 1; }
        update_script
    ' "$tmp_script"
    rm -f "$tmp_script"
    [ "$status" -eq 1 ]
    [[ "$output" == *"Failed to update"* ]]
}

@test "update_script: fetch failure dies with 'Connection error'" {
    tmp_script=$(mktemp)
    cp "$ANI_CLI_PATH" "$tmp_script"
    run bash -c '
        __ANI_CLI_LIB__=1 . "$0" 2>/dev/null
        trap - ERR; set +eE
        remote() { return 1; }
        curl_exe=remote
        update_script
    ' "$tmp_script"
    rm -f "$tmp_script"
    [ "$status" -eq 1 ]
    [[ "$output" == *"Connection error"* ]]
}

@test "update_script: a response without a version line dies 'Invalid response'" {
    # New in 5.0: an error page served with HTTP 200 must not be
    # diffed into the script.
    tmp_script=$(mktemp)
    cp "$ANI_CLI_PATH" "$tmp_script"
    run bash -c '
        __ANI_CLI_LIB__=1 . "$0" 2>/dev/null
        trap - ERR; set +eE
        remote() { printf "<html>rate limited</html>\n"; }
        curl_exe=remote
        patched=0
        patch() { patched=1; cat >/dev/null; }
        update_script
    ' "$tmp_script"
    rm -f "$tmp_script"
    [ "$status" -eq 1 ]
    [[ "$output" == *"Invalid response"* ]]
}
