#!/usr/bin/env bats
#
# What tools/headless-wayland.sh does with the command it is given, by
# running it.
#
# The tool is how the e2e suite runs locally: inside a private, headless
# GNOME Shell session, so the packaged app's windows open on a
# compositor nobody can see instead of on the desktop of whoever ran
# the suite. These cases hold its lifecycle: the command runs in a
# session of its own that leads to that compositor alone, its exit
# status comes back, and the compositor and the session's directory are
# gone afterwards — on success, on failure, when the compositor dies
# before it is ready or while the command runs, and when the tool is
# itself signalled.
#
# The compositor is a stand-in put on PATH as `gnome-shell`: it records
# its arguments and environment, binds the Wayland socket it is asked
# for, reports that the shell has started, and stops on TERM. The real
# one is not started here — CI runners have none, and a crash of a real
# one is reported on the desktop of whoever ran the suite. The D-Bus
# session is real (dbus-run-session).

load '../helpers/loader'

setup() {
    TOOL="$REPO_ROOT/tools/headless-wayland.sh"
    STUB_DIR=$(mktemp -d "$BATS_TEST_TMPDIR/stub-XXXXXX")
    export STUB_DIR
    mkdir -p "$STUB_DIR/bin"
    cat >"$STUB_DIR/bin/gnome-shell" <<'EOF'
#!/usr/bin/env python3
import os, signal, socket, sys, time

d = os.environ['STUB_DIR']
mode = os.environ.get('STUB_MODE', 'normal')
with open(d + '/args', 'w') as f:
    f.write('\n'.join(sys.argv[1:]) + '\n')
with open(d + '/env', 'w') as f:
    f.write('\n'.join(f'{k}={v}' for k, v in os.environ.items()) + '\n')
with open(d + '/pid', 'w') as f:
    f.write(str(os.getpid()))
keyfile = os.path.join(os.environ.get('XDG_CONFIG_HOME', '/nonexistent'), 'glib-2.0/settings/keyfile')
if os.path.exists(keyfile):
    with open(keyfile) as src, open(d + '/keyfile', 'w') as dst:
        dst.write(src.read())
if mode == 'early-exit':
    print('stub compositor: cannot start', file=sys.stderr, flush=True)
    sys.exit(1)
name = [a.split('=', 1)[1] for a in sys.argv if a.startswith('--wayland-display=')][0]
s = socket.socket(socket.AF_UNIX)
s.bind(os.path.join(os.environ['XDG_RUNTIME_DIR'], name))
s.listen()
print('GNOME Shell started at stub', file=sys.stderr, flush=True)

def stop(*_):
    with open(d + '/stopped', 'w') as f:
        f.write('%.6f\n' % time.time())
    sys.exit(0)

signal.signal(signal.SIGTERM, signal.SIG_IGN if mode == 'ignore-term' else stop)
if mode == 'die-during':
    time.sleep(1)
    print('stub compositor: died', file=sys.stderr, flush=True)
    sys.exit(2)
while True:
    time.sleep(0.1)
EOF
    chmod +x "$STUB_DIR/bin/gnome-shell"
    PATH="$STUB_DIR/bin:$PATH"
    export PATH
    # The caller's own session, as a desktop terminal carries it.
    export DISPLAY=':0' WAYLAND_DISPLAY='wayland-0' XDG_SESSION_TYPE='x11'
    export DBUS_SESSION_BUS_ADDRESS='unix:path=/nonexistent/desktop-bus'
}

# The runtime directory the tool made, read off the stand-in's
# environment.
runtime_dir() {
    sed -n 's/^XDG_RUNTIME_DIR=//p' "$STUB_DIR/env"
}

@test "with no command it is a usage error" {
    run sh "$TOOL"
    [ "$status" -eq 64 ]
    [[ "$output" == *usage* ]]
}

@test "the command runs in a session that leads to the private compositor alone" {
    run sh "$TOOL" sh -c '
        echo "rt=$XDG_RUNTIME_DIR"
        echo "wd=$WAYLAND_DISPLAY"
        echo "marker=$ANI_GUI_HEADLESS_WAYLAND"
        echo "display=${DISPLAY-unset}"
        echo "session=$XDG_SESSION_TYPE"
        echo "home=$HOME"
        echo "bus=$DBUS_SESSION_BUS_ADDRESS"
        [ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] && echo socket=live'
    [ "$status" -eq 0 ]
    rt=$(runtime_dir)
    [[ "$rt" == /tmp/ani-gui-wl.* ]]
    [[ "$output" == *"rt=$rt"* ]]
    [[ "$output" == *"wd=ani-gui-e2e"* ]]
    [[ "$output" == *"marker=$rt/ani-gui-e2e"* ]]
    [[ "$output" == *"display=unset"* ]]
    [[ "$output" == *"session=wayland"* ]]
    [[ "$output" == *"home=$HOME"* ]]
    [[ "$output" == *"socket=live"* ]]
    [[ "$output" != *"bus=unix:path=/nonexistent/desktop-bus"* ]]
}

@test "the compositor runs headless on Wayland, without X, in directories of its own" {
    run sh "$TOOL" true
    [ "$status" -eq 0 ]
    rt=$(runtime_dir)
    grep -qxF -- '--headless' "$STUB_DIR/args"
    grep -qxF -- '--wayland' "$STUB_DIR/args"
    grep -qxF -- '--no-x11' "$STUB_DIR/args"
    grep -qxF -- '--wayland-display=ani-gui-e2e' "$STUB_DIR/args"
    grep -qxF "HOME=$rt/home" "$STUB_DIR/env"
    grep -qxF "XDG_CONFIG_HOME=$rt/config" "$STUB_DIR/env"
    grep -qxF "XDG_DATA_HOME=$rt/data" "$STUB_DIR/env"
    grep -qxF "XDG_CACHE_HOME=$rt/cache" "$STUB_DIR/env"
    grep -qxF "XDG_STATE_HOME=$rt/state" "$STUB_DIR/env"
    run ! grep -q '^DISPLAY=' "$STUB_DIR/env"
    run ! grep -q '^WAYLAND_DISPLAY=' "$STUB_DIR/env"
    run ! grep -qxF 'DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent/desktop-bus' "$STUB_DIR/env"
}

@test "the compositor reads settings of its own, and they forbid locking" {
    # A lock would reach the desktop's logind session through the system
    # bus the compositor keeps.
    run sh "$TOOL" true
    [ "$status" -eq 0 ]
    grep -qxF 'GSETTINGS_BACKEND=keyfile' "$STUB_DIR/env"
    grep -qxF 'lock-enabled=false' "$STUB_DIR/keyfile"
    grep -qxF 'disable-lock-screen=true' "$STUB_DIR/keyfile"
    grep -qxF 'idle-delay=uint32 0' "$STUB_DIR/keyfile"
}

@test "services the session's bus starts get the session's own directories" {
    mkdir -p "$STUB_DIR/share/dbus-1/services"
    printf '#!/bin/sh\nenv >"%s"\n' "$STUB_DIR/service.env" >"$STUB_DIR/probe-service"
    chmod +x "$STUB_DIR/probe-service"
    printf '[D-BUS Service]\nName=org.anigui.HeadlessProbe\nExec=%s\n' \
        "$STUB_DIR/probe-service" >"$STUB_DIR/share/dbus-1/services/org.anigui.HeadlessProbe.service"
    export XDG_DATA_DIRS="$STUB_DIR/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
    run sh "$TOOL" sh -c "
        dbus-send --session --print-reply --reply-timeout=2000 --dest=org.anigui.HeadlessProbe / org.freedesktop.DBus.Peer.Ping >/dev/null 2>&1
        n=0
        until [ -s '$STUB_DIR/service.env' ] || [ \$n -ge 50 ]; do sleep 0.1; n=\$((n + 1)); done"
    [ "$status" -eq 0 ]
    rt=$(runtime_dir)
    grep -qxF "HOME=$rt/home" "$STUB_DIR/service.env"
    grep -qxF "XDG_CONFIG_HOME=$rt/config" "$STUB_DIR/service.env"
    grep -qxF 'GVFS_DISABLE_FUSE=1' "$STUB_DIR/service.env"
}

@test "a process the command leaves in its group is stopped, even one the sweep cannot see" {
    # `env -i` empties the environment, as Chromium's children overwrite
    # theirs: only the signal to the command's process group reaches it.
    left="$STUB_DIR/hidden.pid"
    run sh "$TOOL" sh -c "(exec env -i sleep 60 </dev/null >/dev/null 2>&1) & echo \$! >'$left'"
    [ "$status" -eq 0 ]
    [ -s "$left" ]
    run ! kill -0 "$(cat "$left")" 2>/dev/null
}

@test "a process the command leaves behind is stopped" {
    left="$STUB_DIR/left.pid"
    run sh "$TOOL" sh -c "setsid sleep 60 </dev/null >/dev/null 2>&1 & echo \$! >'$left'"
    [ "$status" -eq 0 ]
    [ -s "$left" ]
    run ! kill -0 "$(cat "$left")" 2>/dev/null
}

@test "the command's status comes back, and the session is torn down after a failure" {
    run sh "$TOOL" sh -c 'exit 7'
    [ "$status" -eq 7 ]
    rt=$(runtime_dir)
    [ ! -e "$rt" ]
    [ -f "$STUB_DIR/stopped" ]
    run ! kill -0 "$(cat "$STUB_DIR/pid")" 2>/dev/null
}

@test "the session is torn down after a success, quietly" {
    run sh "$TOOL" true
    [ "$status" -eq 0 ]
    [[ "$output" != *environ* ]]
    [ ! -e "$(runtime_dir)" ]
    [ -f "$STUB_DIR/stopped" ]
}

@test "a compositor that exits before it is ready fails the run, with its log" {
    STUB_MODE=early-exit run sh "$TOOL" sh -c 'echo command-ran'
    [ "$status" -eq 69 ]
    [[ "$output" == *"exited before it was ready"* ]]
    [[ "$output" == *"stub compositor: cannot start"* ]]
    [[ "$output" != *command-ran* ]]
    [ ! -e "$(runtime_dir)" ]
}

@test "a compositor that dies while the command runs fails the run, with its log" {
    STUB_MODE=die-during run sh "$TOOL" sh -c 'sleep 3'
    [ "$status" -eq 70 ]
    [[ "$output" == *"exited while the command ran"* ]]
    [[ "$output" == *"stub compositor: died"* ]]
    [ ! -e "$(runtime_dir)" ]
}

@test "a compositor that ignores TERM is killed" {
    STUB_MODE=ignore-term run sh "$TOOL" true
    [ "$status" -eq 0 ]
    [[ "$output" == *"did not stop on TERM"* ]]
    run ! kill -0 "$(cat "$STUB_DIR/pid")" 2>/dev/null
    [ ! -e "$(runtime_dir)" ]
}

@test "signalled, it stops the command at once, then the compositor, and removes the session" {
    started="$STUB_DIR/command.pid"
    termed="$STUB_DIR/command-term"
    sh "$TOOL" sh -c "
        trap 'python3 -c \"import time; print(\\\"%.6f\\\" % time.time())\" >\"$termed\"; exit 0' TERM
        echo \$\$ >'$started'
        while :; do sleep 0.1; done" 3>&- &
    tool=$!
    n=0
    until [ -s "$started" ]; do
        n=$((n + 1))
        [ "$n" -le 150 ] || return 1
        sleep 0.1
    done
    rt=$(runtime_dir)
    signalled=$SECONDS
    kill -TERM "$tool"
    wait "$tool" || true
    # Handled at once, not after a timeout.
    [ $((SECONDS - signalled)) -lt 8 ]
    run ! kill -0 "$(cat "$started")" 2>/dev/null
    run ! kill -0 "$(cat "$STUB_DIR/pid")" 2>/dev/null
    [ ! -e "$rt" ]
    # The command first, while the compositor it draws on is still there.
    [ -s "$termed" ] && [ -s "$STUB_DIR/stopped" ]
    python3 -c 'import sys; a, b = (float(open(f).read()) for f in sys.argv[1:]); sys.exit(0 if a <= b else 1)' \
        "$termed" "$STUB_DIR/stopped"
}
