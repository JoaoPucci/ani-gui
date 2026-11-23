#!/usr/bin/env bats
#
# What tools/appimage-catalog-check.sh decides about an AppImage, by
# running it.
#
# The Packages workflow runs the script on every AppImage it builds,
# on an ubuntu-22.04 runner with a real X server and firejail. Those
# are not here: CI's bats runner has neither, and a real launch is the
# workflow's job. What these cases hold is the verdict: for each way an
# app can come up or fail to, the script has to pass or fail, and name
# the criterion that failed.
#
# Every program the script starts is a stand-in put on PATH: Xvfb
# reports a display and waits, icewm waits, xdpyinfo answers, import
# writes a file, and xdotool finds a window of the app's class while a
# marker file exists. firejail answers the interface listing with what
# STUB_NET says (loopback by default) and runs the "AppImage", a script
# that acts out STUB_APP: what it logs, when its window appears and
# goes, whether it exits. `sleep` is a stand-in too, sleeping a
# twentieth of what it is asked, so the script's 30-second window wait
# and 20-second hold take a second or two; the script and the app
# share it, so their timings keep their proportions.

load '../helpers/loader'

setup() {
    CHECK="$REPO_ROOT/tools/appimage-catalog-check.sh"
    STUB_DIR=$(mktemp -d "$BATS_TEST_TMPDIR/stub-XXXXXX")
    OUT="$BATS_TEST_TMPDIR/out"
    export STUB_DIR
    mkdir -p "$STUB_DIR/bin"
    real_sleep=$(command -v sleep)

    # A twentieth of the time asked for.
    cat >"$STUB_DIR/bin/sleep" <<EOF
#!/bin/sh
exec $real_sleep "\$(awk -v s="\$1" 'BEGIN { print s / 20 }')"
EOF
    cat >"$STUB_DIR/bin/Xvfb" <<'EOF'
#!/bin/sh
printf '42\n' >&3
exec sleep 600
EOF
    cat >"$STUB_DIR/bin/icewm" <<'EOF'
#!/bin/sh
exec sleep 600
EOF
    printf '#!/bin/sh\nexit 0\n' >"$STUB_DIR/bin/xdpyinfo"
    cat >"$STUB_DIR/bin/import" <<'EOF'
#!/bin/sh
for last; do :; done
: >"$last"
EOF
    cat >"$STUB_DIR/bin/xdotool" <<'EOF'
#!/bin/sh
[ -e "$STUB_DIR/window" ] || exit 1
case " $* " in
    *" getwindowname "*) echo ani-gui ;;
esac
exit 0
EOF
    cat >"$STUB_DIR/bin/firejail" <<'EOF'
#!/bin/sh
case " $* " in
    *" --version "*) echo 'firejail version stub' ;;
    *" ls /sys/class/net "*) printf '%s\n' "${STUB_NET:-lo}" ;;
    *" --appimage "*)
        for last; do :; done
        exec "$last"
        ;;
esac
EOF
    chmod +x "$STUB_DIR"/bin/*
    PATH="$STUB_DIR/bin:$PATH"
    export PATH

    # The AppImage: a script acting out STUB_APP.
    APPIMAGE="$BATS_TEST_TMPDIR/app.AppImage"
    cat >"$APPIMAGE" <<'EOF'
#!/bin/sh
w="$STUB_DIR/window"
case "${STUB_APP:?}" in
    good)
        echo '[backend] INFO ani_gui_backend: ani-gui-backend ready lanes=[]'
        : >"$w"
        echo "[main] app's first page rendered"
        ;;
    no-window)
        echo '[backend] INFO ani_gui_backend: ani-gui-backend ready lanes=[]'
        ;;
    window-goes)
        echo '[backend] INFO ani_gui_backend: ani-gui-backend ready lanes=[]'
        : >"$w"
        echo "[main] app's first page rendered"
        sleep 4
        rm -f "$w"
        ;;
    no-backend)
        : >"$w"
        ;;
    splash-only)
        # The backend is up and the splash shows, but the main window
        # never renders its first page: the splash stays, offering
        # Restart and Quit, and nothing logs a startup failure.
        echo '[backend] INFO ani_gui_backend: ani-gui-backend ready lanes=[]'
        : >"$w"
        ;;
    startup-failed)
        : >"$w"
        echo '[main] startup failed: Error: backend exited before handshake (code=1, signal=null)'
        ;;
    exits)
        echo '[main] startup failed: Error: backend exited before handshake (code=1, signal=null)'
        exit 0
        ;;
esac
exec sleep 600
EOF
    chmod +x "$APPIMAGE"
}

@test "an app that comes up, renders and stays up passes" {
    STUB_APP=good run sh "$CHECK" "$APPIMAGE" "$OUT"
    [ "$status" -eq 0 ]
    [[ "$output" == *PASS* ]]
    [[ "$output" != *FAIL* ]]
    [ -f "$OUT/app.log" ]
    [ -f "$OUT/screen.png" ]
}

@test "no window of the app's class within the wait fails" {
    STUB_APP=no-window run sh "$CHECK" "$APPIMAGE" "$OUT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"FAIL: no window of class ani-gui within 30 s"* ]]
    [ -f "$OUT/screen.png" ]
}

@test "a window gone before the hold ends fails" {
    STUB_APP=window-goes run sh "$CHECK" "$APPIMAGE" "$OUT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"FAIL: no window of class ani-gui 20 s after the first appeared"* ]]
}

@test "a window with no backend ready in the log fails" {
    STUB_APP=no-backend run sh "$CHECK" "$APPIMAGE" "$OUT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"FAIL: the backend never reported ready"* ]]
}

@test "a splash that never gives way to the first page fails" {
    STUB_APP=splash-only run sh "$CHECK" "$APPIMAGE" "$OUT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"FAIL: the app's first page never rendered"* ]]
}

@test "the failed-startup dialog is a window, and fails on the logged failure" {
    STUB_APP=startup-failed run sh "$CHECK" "$APPIMAGE" "$OUT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"FAIL: the app logged a startup failure"* ]]
    [[ "$output" == *"backend exited before handshake"* ]]
}

@test "an app that exits without a window fails, saying it exited" {
    # v0.14.1's AppImage on the catalog's glibc: the backend could not
    # load, the app logged it and quit before any window.
    STUB_APP=exits run sh "$CHECK" "$APPIMAGE" "$OUT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"FAIL: the app exited (status 0)"* ]]
    [[ "$output" == *"startup failed: Error: backend exited before handshake"* ]]
}

@test "a sandbox that leaves the network up stops the run before the launch" {
    STUB_NET=$'eth0\nlo' STUB_APP=good run sh "$CHECK" "$APPIMAGE" "$OUT"
    [ "$status" -eq 2 ]
    [[ "$output" == *"left more than loopback"* ]]
    [ ! -e "$OUT/app.log" ]
}

@test "a missing AppImage or a wrong argument count is an environment error" {
    run sh "$CHECK" "$BATS_TEST_TMPDIR/absent.AppImage" "$OUT"
    [ "$status" -eq 2 ]
    run sh "$CHECK" "$APPIMAGE"
    [ "$status" -eq 2 ]
    [[ "$output" == *usage* ]]
}
