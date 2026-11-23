#!/bin/sh
# Start an AppImage the way the AppImageHub catalog's test does, and
# fail unless the app comes up.
#
#   tools/appimage-catalog-check.sh <AppImage> <out-dir>
#
# The catalog (AppImage/appimage.github.io, code/worker.sh) launches a
# listed AppImage on GitHub's ubuntu-22.04 runner, glibc 2.35: Xvfb at
# 800x600 with icewm, `firejail --quiet --noprofile --net=none
# --appimage`, and a visible window within 30 seconds. This runs that
# launch, and is stricter where the catalog's own criterion passes an
# app that did not start:
#
#   - the window has to be one of the app's own (class ani-gui): icewm's
#     taskbar is a visible window from the moment icewm starts;
#   - it has to still be there 20 seconds after it appeared;
#   - the app's log has to show its backend reporting ready, and no
#     "startup failed": the dialog the app shows when its backend does
#     not start is a window of the app's class too.
#
# The launch has no network, as the catalog's has none. Before it, the
# same firejail is asked to list the network interfaces it leaves, and
# anything beyond loopback stops the run: firejail inside a container
# cannot make a namespace, says so, and runs its command unsandboxed,
# which would leave the app online while the run read as offline.
#
# One difference from the catalog is deliberate: its worker installs a
# firejail build of its own over the system's (a 2023 Alpine build,
# made setuid) and sets kernel.unprivileged_userns_clone=1, while this
# uses the firejail Ubuntu 22.04 ships (0.9.66) and the runner's
# kernel settings, where unprivileged user namespaces are already on.
# A failure particular to either firejail build would show in one and
# not the other; the glibc, the display and the launch are the same.
#
# It writes <out-dir>/app.log (everything the app printed),
# <out-dir>/screen.png (the screen at the verdict) and
# <out-dir>/xvfb.log. Each failed criterion prints a line beginning
# `FAIL:`; the exit status is 0 when the app came up, 1 when it did
# not, and 2 when the environment could not run the check.
#
# It needs Xvfb, icewm, xdotool, xdpyinfo (x11-utils), import
# (imagemagick) and firejail, which mounts the AppImage's image itself
# through a loop device. .github/workflows/packages.yml installs them
# and runs this on the AppImage it built.
#
# What this does NOT check: anything after the first page renders.
# Playback, search and every other request need the network the
# launch does not have; the check says the AppImage starts on the
# catalog's system, nothing more.

set -u

if [ "$#" -ne 2 ]; then
    echo "usage: $0 <AppImage> <out-dir>" >&2
    exit 2
fi
case "$1" in
    /*) appimage=$1 ;;
    *) appimage=$PWD/$1 ;;
esac
out=$2
if [ ! -f "$appimage" ]; then
    echo "no AppImage at $appimage" >&2
    exit 2
fi
mkdir -p "$out" || exit 2
chmod +x "$appimage" || exit 2

# A crash inside the check writes no core file, and nothing the app
# starts looks for a Wayland session. POSIX leaves `ulimit -c`
# undefined; dash, the runners' /bin/sh, and bash both take it.
# shellcheck disable=SC3045
ulimit -c 0
unset WAYLAND_DISPLAY

WINDOW_WAIT=30
HOLD=20
CLASS=ani-gui

work=$(mktemp -d) || exit 2
xvfb_pid=
wm_pid=
app_pid=

# Only the processes started here are stopped: firejail ends its
# sandbox, and everything in it, when it is told to stop.
# shellcheck disable=SC2329 # the EXIT trap calls it
cleanup() {
    for pid in $app_pid $wm_pid $xvfb_pid; do
        kill "$pid" 2>/dev/null
    done
    for pid in $app_pid $wm_pid $xvfb_pid; do
        wait "$pid" 2>/dev/null
    done
    rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 2' HUP INT TERM

ldd --version | head -n 1
firejail --version | head -n 1
if [ -r /proc/sys/kernel/unprivileged_userns_clone ]; then
    echo "kernel.unprivileged_userns_clone = $(cat /proc/sys/kernel/unprivileged_userns_clone)"
fi

net=$(firejail --quiet --noprofile --net=none ls /sys/class/net 2>&1)
if [ "$net" != lo ]; then
    printf 'firejail --net=none left more than loopback:\n%s\n' "$net" >&2
    exit 2
fi
echo "firejail --net=none leaves only loopback"

# Xvfb picks a free display and writes its number to the descriptor,
# so the check never shares a display with anything else running.
Xvfb -displayfd 3 -screen 0 800x600x24 3>"$work/display" >"$out/xvfb.log" 2>&1 &
xvfb_pid=$!
display=
i=0
while [ "$i" -lt 10 ]; do
    display=$(tr -d '\n' <"$work/display")
    [ -n "$display" ] && break
    sleep 1
    i=$((i + 1))
done
if [ -z "$display" ]; then
    echo "Xvfb did not start; see $out/xvfb.log" >&2
    exit 2
fi
DISPLAY=:$display
export DISPLAY
xdpyinfo >/dev/null 2>&1 || {
    echo "display $DISPLAY does not answer" >&2
    exit 2
}

icewm >/dev/null 2>&1 &
wm_pid=$!
sleep 2

app_window() {
    xdotool search --onlyvisible --class "$CLASS" >/dev/null 2>&1
}

screenshot() {
    import -window root "$out/screen.png" 2>/dev/null ||
        echo "could not take a screenshot" >&2
}

firejail --quiet --noprofile --net=none --appimage "$appimage" >"$out/app.log" 2>&1 &
app_pid=$!

seen=0
i=0
while [ "$i" -lt "$WINDOW_WAIT" ]; do
    sleep 1
    i=$((i + 1))
    if app_window; then
        seen=$i
        break
    fi
    if ! kill -0 "$app_pid" 2>/dev/null; then
        break
    fi
done

if [ "$seen" -eq 0 ]; then
    screenshot
    if kill -0 "$app_pid" 2>/dev/null; then
        echo "FAIL: no window of class $CLASS within ${WINDOW_WAIT} s"
    else
        wait "$app_pid"
        echo "FAIL: the app exited (status $?) after ${i} s without a window of class $CLASS"
        app_pid=
    fi
    grep 'startup failed' "$out/app.log"
    exit 1
fi
names=$(xdotool search --onlyvisible --class "$CLASS" getwindowname %@ 2>/dev/null | tr '\n' '|')
echo "a window of class $CLASS after ${seen} s: ${names}"

sleep "$HOLD"
screenshot
failed=0
if ! app_window; then
    echo "FAIL: no window of class $CLASS ${HOLD} s after the first appeared"
    failed=1
fi
if ! grep -q 'backend ready' "$out/app.log"; then
    echo "FAIL: the backend never reported ready"
    failed=1
fi
if grep -q 'startup failed' "$out/app.log"; then
    echo "FAIL: the app logged a startup failure:"
    grep 'startup failed' "$out/app.log"
    failed=1
fi
if [ "$failed" -eq 0 ]; then
    echo "PASS: the app came up offline and stayed up"
fi
exit "$failed"
