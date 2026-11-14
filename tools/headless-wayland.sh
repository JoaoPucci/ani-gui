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
# signalled whole, including what it leaves behind there. Everything
# the session started runs in a transient systemd user scope of its own
# (systemd-run --user --scope), a cgroup nothing started inside it can
# leave: whatever is still running there at the end — a service that
# daemonized, a process that made a session of its own — is stopped by
# stopping that one scope. Nothing is picked out of the process table:
# processes are stopped by the pid this tool recorded or that the
# session's own shell reported, or by a group this tool created, each
# only once /proc/<pid>/stat shows the pid is still that process (its
# start time) or a member of the session this tool made; or by the
# scope. It needs systemd 254 or later and a user
# service manager. The command has no terminal and reads nothing. A
# SIGKILL to this tool leaves the whole session running; it has to be
# stopped by hand (`systemctl --user stop` on the scope it names as the
# session starts).
#
# What the session still shares with the desktop is the system bus.
# gnome-shell needs it: without one its startup throws and it aborts on
# exit, which Ubuntu's crash reporter announces on the desktop. Through
# it the shell finds the desktop's logind session and follows its lock
# state, and a shell that locked would set that session's LockedHint.
# Its settings forbid locking (screen lock disabled and locked down, no
# idle delay), so it never locks and takes no sleep inhibitor. Core
# dumps are switched off for the session.

# in_session, same and await_exit are predicates, called in conditions on purpose;
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

# Field $2 of process $1's /proc/<pid>/stat line, numbered as proc(5)
# numbers them (6 the session, 22 the start time), counted from after
# the command name, which may hold spaces. Fails with no such process.
stat_field() {
    { stat=$(cat "/proc/$1/stat"); } 2>/dev/null || return 1
    # shellcheck disable=SC2086 # the fields, split
    set -- "$2" ${stat##*) }
    # $2 is now field 3, the state, so field k is ${k-1}.
    eval "printf '%s\\n' \"\${$(($1 - 1))-}\""
}

# Whether process $1 belongs to session $2. A session is not joined from
# outside, so a live member of a session this tool made, which holds the
# session's id, is one this tool started.
in_session() {
    [ -n "$2" ] && [ "$(stat_field "$1" 6 || true)" = "$2" ]
}

# Whether pid $1 is still the process that started at $2, the start time
# read when the pid was recorded: a pid and its start time name one
# process. A recorded pid is checked this way before every signal. The
# shell reaps a background child as soon as it exits, even while it
# waits on something else, and from then on the pid, and a group or
# session id it led, may be anyone's.
same() {
    [ -n "$2" ] && [ "$(stat_field "$1" 22 || true)" = "$2" ]
}

# Wait until pid $1 is no longer the process that started at $2, $3
# tenths of a second at most ($STOP_TIMEOUT_TENTHS by default); fail if
# it still is.
await_exit() {
    n=0
    while same "$1" "$2" && [ "$n" -lt "${3:-$STOP_TIMEOUT_TENTHS}" ]; do
        sleep 0.1
        n=$((n + 1))
    done
    ! same "$1" "$2"
}

# --- inside the private D-Bus session ---------------------------------

if [ "${1-}" = '__inner' ]; then
    rt=$2
    shift 2
    # The outer shell signals this one by this pid, not by its name.
    echo "$$" >"$rt/inner.pid"
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
    shell_start=$(stat_field "$shell" 22 || true)
    group=''
    group_start=''
    logged=''

    teardown() {
        # The command's whole process group, while its leader is there to
        # show the group id is still the command's. What the command
        # leaves in its group after the leader has gone is stopped with
        # the session's scope.
        if [ -n "$group" ] && same "$group" "$group_start" &&
            kill -s TERM -- "-$group" 2>/dev/null; then
            await_exit "$group" "$group_start" || {
                ! same "$group" "$group_start" || kill -s KILL -- "-$group" 2>/dev/null || true
            }
        fi
        if same "$shell" "$shell_start"; then
            kill -TERM "$shell" 2>/dev/null || true
            if ! await_exit "$shell" "$shell_start"; then
                say "the compositor did not stop on TERM; killing it"
                ! same "$shell" "$shell_start" || kill -KILL "$shell" 2>/dev/null || true
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
        if ! same "$shell" "$shell_start"; then
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
    group_start=$(stat_field "$group" 22 || true)
    status=0
    wait "$group" || status=$?

    if ! same "$shell" "$shell_start"; then
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
for tool in "$GNOME_SHELL" dbus-run-session dbus-update-activation-environment setsid systemd-run systemctl; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        say "$tool is not installed"
        exit 69
    fi
done
# The session's scope is made by the user's service manager.
if ! systemctl --user show-environment >/dev/null 2>&1; then
    say "no systemd user service manager answers; the session's scope needs one"
    exit 69
fi

dir=$(dirname "$0")
dir=$(cd "$dir" && pwd)
self="$dir/${0##*/}"

# Under /tmp, not $TMPDIR: a Unix socket's path is capped at 108 bytes,
# and the compositor aborts when its runtime directory's path is longer
# than that leaves room for.
rt=$(mktemp -d /tmp/ani-gui-wl.XXXXXX)
chmod 700 "$rt"
session=''
session_start=''
# The scope's name carries the runtime directory's unique suffix.
unit="ani-gui-wl-${rt##*.}.scope"

# Stop what is still running in this session. The inner shell first, by
# the pid it reported, once it is shown to be in this session: it stops
# the command, then the compositor while its bus is still there. Then
# everything left in the session's scope. The session leader is
# signalled only while its pid is still the process this tool started.
reap() {
    if [ -n "$session" ] && same "$session" "$session_start"; then
        inner=$(cat "$rt/inner.pid" 2>/dev/null || true)
        if [ -n "$inner" ] && in_session "$inner" "$session"; then
            kill -TERM "$inner" 2>/dev/null || true
        fi
        # The inner shell may take a stop timeout for the command and
        # another for the compositor.
        await_exit "$session" "$session_start" $((STOP_TIMEOUT_TENTHS * 2 + 50)) || {
            ! same "$session" "$session_start" || kill -s KILL -- "-$session" 2>/dev/null || true
        }
    fi
    # TERM to everything left in the scope, KILL to what is still there
    # after its stop timeout. A scope that has emptied is gone already.
    # Only a scope this run made: the inner shell, which wrote its pid,
    # ran inside it, so systemd-run had created it under this name.
    if [ -e "$rt/inner.pid" ]; then
        systemctl --user stop "$unit" 2>/dev/null || true
    fi
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
say "session scope: $unit"

# A session of its own, so a Ctrl-C at the terminal reaches this shell
# alone, and the teardown above stops the rest in order; inside a scope
# of its own, so whatever leaves that session is still stopped with it.
# systemd-run reaches the user's service manager through the caller's
# environment, so the session's own environment is set after it. It
# would expand `$NAME` in the command's arguments itself; it is told not
# to, so the command reaches its shell as written.
setsid systemd-run --user --scope --quiet --collect --unit="$unit" \
    --expand-environment=no \
    -p TimeoutStopSec="$((STOP_TIMEOUT_TENTHS / 10))s" \
    env -u DISPLAY -u WAYLAND_DISPLAY -u WAYLAND_SOCKET -u XDG_SESSION_ID \
    -u DBUS_SESSION_BUS_ADDRESS -u GSETTINGS_BACKEND -u XAUTHORITY \
    XDG_RUNTIME_DIR="$rt" \
    XDG_SESSION_TYPE=wayland \
    dbus-run-session -- sh "$self" __inner "$rt" "$@" &
session=$!
session_start=$(stat_field "$session" 22 || true)
status=0
wait "$session" || status=$?
exit "$status"
