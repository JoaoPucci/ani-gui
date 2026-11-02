#!/bin/sh
# Run a command inside a private, headless GNOME Shell Wayland session.
#
#   tools/headless-wayland.sh <command> [args...]
#
# The e2e suite launches the packaged app, and the app opens real
# windows. Run from a desktop session, a test launch reaches that
# desktop: under xvfb-run the packaged launcher still picked the
# session's Wayland compositor and opened its windows on the screen of
# whoever ran the suite. This starts a compositor nobody can see and
# hands the command a session that leads to it alone:
#
#   - its own runtime directory, made here and removed afterwards, which
#     holds the compositor's socket;
#   - its own D-Bus session (dbus-run-session), so neither the compositor
#     nor anything it activates talks to the desktop's session bus;
#   - its own HOME and XDG base directories for the compositor and every
#     service the bus activates, so their settings and state are not the
#     desktop's; the compositor reads its settings from a key file there
#     (GSETTINGS_BACKEND=keyfile), so a setting changed on the desktop
#     does not reach it;
#   - no DISPLAY, so nothing falls back to an X server on the desktop.
#
# The command keeps the caller's HOME and XDG directories (pnpm and
# Playwright keep their caches there), and receives WAYLAND_DISPLAY,
# XDG_RUNTIME_DIR, XDG_SESSION_TYPE=wayland and ANI_GUI_HEADLESS_WAYLAND,
# the compositor socket's full path. electron/e2e/launch.ts refuses to
# launch the app outside CI unless that variable names the display the
# app is about to use.
#
# The compositor is ready when its socket exists and it reports that the
# shell has started. If it exits before that, or while the command runs,
# its log is printed and the run fails. However the run ends — success,
# failure, or a signal to this tool (Ctrl-C included) — the command is
# stopped, then the compositor, then the bus, and the runtime directory
# is removed. The command runs in a process group of its own, which is
# signalled whole, including what it leaves behind there. Anything else
# still running whose environment names the runtime directory is
# stopped as well. That sweep cannot see a process that rewrote its
# environment block, as Chromium's child processes do; those are
# reached through the command's process group, unless they left it.
# The command has no terminal and reads nothing. A SIGKILL to this tool
# leaves the whole session running; it has to be stopped by hand.
#
# What the session still shares with the desktop is the system bus.
# gnome-shell needs it: without one its startup throws and it aborts on
# exit, which Ubuntu's crash reporter announces on the desktop. Through
# it the shell finds the desktop's logind session and follows its lock
# state, and a shell that locked would set that session's LockedHint.
# Its settings forbid locking (screen lock disabled and locked down, no
# idle delay), so it never locks and takes no sleep inhibitor. Core
# dumps are switched off for the session.

# ours and await_exit are predicates, called in conditions on purpose;
# the functions the traps call are invoked indirectly.
# shellcheck disable=SC2310,SC2329
set -eu

# The compositor, found on PATH; this tool's own tests
# (tests/bash/arch/headless_wayland.bats) put a stand-in there.
GNOME_SHELL=gnome-shell
SOCKET_NAME=ani-gui-e2e
READY_TIMEOUT_TENTHS=600
STOP_TIMEOUT_TENTHS=150

say() {
    printf 'headless-wayland: %s\n' "$*" >&2
}

show_log() {
    if [ -s "$1" ]; then
        say "compositor log (last 40 lines of $1):"
        tail -n 40 "$1" >&2
    fi
}

# Whether process $1 is still running in this session: its environment
# names the runtime directory $2. A pid read from a file may have been
# reused by then.
ours() {
    { tr '\0' '\n' <"/proc/$1/environ"; } 2>/dev/null | grep -qxF "XDG_RUNTIME_DIR=$2"
}

# Wait until process $1 is gone, $2 tenths of a second at most
# ($STOP_TIMEOUT_TENTHS by default); fail if it is still there.
await_exit() {
    n=0
    while kill -0 "$1" 2>/dev/null && [ "$n" -lt "${2:-$STOP_TIMEOUT_TENTHS}" ]; do
        sleep 0.1
        n=$((n + 1))
    done
    ! kill -0 "$1" 2>/dev/null
}

# --- inside the private D-Bus session ---------------------------------

if [ "${1-}" = '__inner' ]; then
    rt=$2
    shift 2
    log="$rt/gnome-shell.log"
    socket="$rt/$SOCKET_NAME"
    # dash and bash both take -c; POSIX leaves it undefined.
    # shellcheck disable=SC3045
    ulimit -c 0 2>/dev/null || true

    # The compositor, and every service this bus starts for it, get the
    # session's own HOME and XDG directories; no FUSE mounts in it.
    private="HOME=$rt/home XDG_CONFIG_HOME=$rt/config XDG_DATA_HOME=$rt/data"
    private="$private XDG_CACHE_HOME=$rt/cache XDG_STATE_HOME=$rt/state"
    private="$private GVFS_DISABLE_FUSE=1"
    # shellcheck disable=SC2086 # one word per assignment, no spaces in $rt
    dbus-update-activation-environment $private

    # The compositor's settings: a key file only it reads. It must never
    # lock — a lock would reach the desktop's logind session.
    mkdir -p "$rt/config/glib-2.0/settings"
    cat >"$rt/config/glib-2.0/settings/keyfile" <<'EOF'
[org/gnome/desktop/screensaver]
lock-enabled=false

[org/gnome/desktop/lockdown]
disable-lock-screen=true

[org/gnome/desktop/session]
idle-delay=uint32 0
EOF

    # shellcheck disable=SC2086
    env $private GSETTINGS_BACKEND=keyfile \
        "$GNOME_SHELL" --headless --wayland --no-x11 \
        --wayland-display="$SOCKET_NAME" \
        --virtual-monitor 1920x1080 >"$log" 2>&1 &
    shell=$!
    group=''
    logged=''

    teardown() {
        # The command's whole process group, whether its leader is still
        # there or left something behind.
        if [ -n "$group" ] && kill -s TERM -- "-$group" 2>/dev/null; then
            n=0
            while kill -s 0 -- "-$group" 2>/dev/null && [ "$n" -lt "$STOP_TIMEOUT_TENTHS" ]; do
                sleep 0.1
                n=$((n + 1))
            done
            if kill -s 0 -- "-$group" 2>/dev/null; then kill -s KILL -- "-$group" 2>/dev/null || true; fi
        fi
        if kill -0 "$shell" 2>/dev/null; then
            kill -TERM "$shell" 2>/dev/null || true
            if ! await_exit "$shell"; then
                say "the compositor did not stop on TERM; killing it"
                kill -KILL "$shell" 2>/dev/null || true
            fi
        fi
        code=0
        wait "$shell" 2>/dev/null || code=$?
        if [ "$code" -ne 0 ] && [ "$code" -ne 127 ] && [ -z "$logged" ]; then
            say "the compositor ended with status $code"
            show_log "$log"
        fi
    }
    trap teardown EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM

    n=0
    until [ -S "$socket" ] && grep -q 'GNOME Shell started' "$log"; do
        if ! kill -0 "$shell" 2>/dev/null; then
            say "the compositor exited before it was ready"
            show_log "$log"
            logged=1
            exit 69
        fi
        if [ "$n" -ge "$READY_TIMEOUT_TENTHS" ]; then
            say "the compositor was not ready after 60 seconds"
            show_log "$log"
            logged=1
            exit 69
        fi
        sleep 0.1
        n=$((n + 1))
    done

    export WAYLAND_DISPLAY="$SOCKET_NAME"
    export XDG_SESSION_TYPE=wayland
    export ANI_GUI_HEADLESS_WAYLAND="$socket"

    # In the background, so a signal to this shell is handled at once
    # instead of after the command; in a session and process group of
    # its own, so the whole of it can be stopped. It has no terminal and
    # reads nothing: an interactive command does not work here.
    setsid "$@" &
    group=$!
    status=0
    wait "$group" || status=$?

    if ! kill -0 "$shell" 2>/dev/null; then
        say "the compositor exited while the command ran"
        show_log "$log"
        logged=1
        [ "$status" -ne 0 ] || status=70
    fi
    exit "$status"
fi

# --- outside: make the session, run it, clean up ----------------------

if [ "$#" -eq 0 ]; then
    say "usage: tools/headless-wayland.sh <command> [args...]"
    exit 64
fi
for tool in "$GNOME_SHELL" dbus-run-session dbus-update-activation-environment setsid pgrep; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        say "$tool is not installed"
        exit 69
    fi
done

dir=$(dirname "$0")
dir=$(cd "$dir" && pwd)
self="$dir/${0##*/}"

# Under /tmp, not $TMPDIR: a Unix socket's path is capped at 108 bytes,
# and the compositor aborts when its runtime directory's path is longer
# than that leaves room for.
rt=$(mktemp -d /tmp/ani-gui-wl.XXXXXX)
chmod 700 "$rt"
session=''

# Stop what is still running in this session. The inner shell first: it
# stops the command, then the compositor while its bus is still there;
# then anything else whose environment names the runtime directory.
reap() {
    if [ -n "$session" ] && kill -0 "$session" 2>/dev/null; then
        for pid in $(pgrep -s "$session" 2>/dev/null); do
            case "$(tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null)" in
                "sh $self __inner "*) kill -TERM "$pid" 2>/dev/null || true ;;
                *) ;;
            esac
        done
        # The inner shell may take a stop timeout for the command and
        # another for the compositor.
        await_exit "$session" $((STOP_TIMEOUT_TENTHS * 2 + 50)) ||
            kill -s KILL -- "-$session" 2>/dev/null || true
    fi
    for round in TERM KILL; do
        for env in /proc/[0-9]*/environ; do
            pid=${env#/proc/}
            pid=${pid%/environ}
            [ "$pid" != "$$" ] || continue
            if ours "$pid" "$rt"; then
                kill "-$round" "$pid" 2>/dev/null || true
            fi
        done
        [ "$round" = KILL ] || sleep 1
    done
}

cleanup() {
    status=$?
    trap - EXIT HUP INT TERM
    reap
    # A FUSE daemon killed before it unmounted leaves its mount behind.
    grep -F " $rt/" /proc/self/mounts 2>/dev/null | while read -r _ mnt _; do
        fusermount3 -uz "$mnt" 2>/dev/null || fusermount -uz "$mnt" 2>/dev/null || true
    done
    rm -rf "${rt:?}" 2>/dev/null || say "could not remove $rt"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

mkdir -p "$rt/home" "$rt/config" "$rt/data" "$rt/cache" "$rt/state"

# A session of its own, so a Ctrl-C at the terminal reaches this shell
# alone, and the teardown above stops the rest in order.
env -u DISPLAY -u WAYLAND_DISPLAY -u WAYLAND_SOCKET -u XDG_SESSION_ID \
    -u DBUS_SESSION_BUS_ADDRESS -u GSETTINGS_BACKEND -u XAUTHORITY \
    XDG_RUNTIME_DIR="$rt" \
    XDG_SESSION_TYPE=wayland \
    setsid dbus-run-session -- sh "$self" __inner "$rt" "$@" &
session=$!
status=0
wait "$session" || status=$?
exit "$status"
