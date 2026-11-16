"use strict";

// The `ani-gui` launcher the Linux packages run in place of the app's
// binary, which build-resources/after-pack.cjs renames to
// `ani-gui.bin`. Why it sets the platform hint is told there; what it
// does on a terminal, in the script itself. It is bash, as the
// AppImage's AppRun already is, and every Debian system has it.

/** The launcher for the packed executable `exe`. */
function launcherScript(exe) {
  return `#!/bin/bash
# ani-gui launcher — select native Wayland before Electron's Ozone init.
# \`=auto\` resolves to Wayland under a Wayland session and X11 otherwise, so
# this is a no-op on X11. Setting it here (pre-init) avoids the app.relaunch()
# re-exec that crashed on Wayland. Honors a value the user already exported.
# \`--enable-wayland-ime\` keeps IME composition (IBus/Fcitx — Japanese/Chinese/
# Korean) working under native Wayland; Chromium does not infer it from the
# ozone hint. It's safe here because every launch, a Restart's relaunch
# included, starts on the platform the hint picks: nothing re-execs into
# another one.
: "\${ELECTRON_OZONE_PLATFORM_HINT:=auto}"
export ELECTRON_OZONE_PLATFORM_HINT
HERE="$(dirname "$(readlink -f "$0")")"
app=("$HERE/${exe}.bin" --enable-wayland-ime "$@")

# Off a terminal (the desktop, a pipe, a file), the launcher becomes the
# app. So it does without setsid, which only util-linux-less hosts lack.
if { [ ! -t 0 ] && [ ! -t 1 ] && [ ! -t 2 ]; } || ! command -v setsid >/dev/null 2>&1; then
  exec "\${app[@]}"
fi

# On a terminal, the app must not hold it. Node puts a terminal it holds
# back as it found it when the process exits, and once the terminal has
# closed that step aborts the app (SIGABRT) after it has shut down. So
# the app gets /dev/null for stdin and a pipe for each stream that is the
# terminal, which \`cat\` relays to it, and runs in a session of its own:
# whatever the terminal does to its session and foreground group (a
# hangup, Ctrl-C) reaches this launcher alone, which hands the app one
# SIGTERM. The relays run in sessions of their own too, and ignore
# SIGHUP and SIGTERM, so the app's last lines still reach the terminal
# while it shuts down; a terminal that has gone ends them, and the app's
# later writes fail as a closed pipe's do, which it ignores
# (lib/stdio-guard.cjs). With setpriv, the app also gets a
# SIGTERM if this launcher is killed outright, rather than outliving it.
out=1
err=2
relays=()
if [ -t 1 ]; then
  exec 3> >(trap '' HUP TERM; exec setsid cat)
  relays+=("$!")
  out=3
fi
if [ -t 2 ]; then
  exec 4> >(trap '' HUP TERM; exec setsid cat >&2 3>&-)
  relays+=("$!")
  err=4
fi
launch=(setsid)
if command -v setpriv >/dev/null 2>&1 && setpriv --pdeathsig TERM -- true 2>/dev/null; then
  launch+=(setpriv --pdeathsig TERM --)
fi
# A background job of this shell starts with SIGINT and SIGQUIT ignored;
# the app starts with the dispositions this launcher was given.
(trap - INT QUIT; exec "\${launch[@]}" "\${app[@]}" </dev/null >&"$out" 2>&"$err" 3>&- 4>&-) &
pid=$!
exec 3>&- 4>&-

# Whether the app still needs a SIGTERM. A stop sent to every process at
# once (a session's scope stopped, a kill by name) reaches the app as well as
# this launcher, and Chromium answers a second stop signal by dying on
# the spot: its handler for each goes back to the default as it runs.
# So none is sent while a SIGTERM is pending for the app, or once its
# handler for SIGTERM has gone while those for SIGINT and SIGHUP remain.
# An app with no handler yet takes the SIGTERM's default, as it would
# have before this launcher existed.
needs_term() {
  local key value pending=0 caught=0
  while read -r key value; do
    case "$key" in
      SigPnd: | ShdPnd:) pending=$((pending | 0x$value)) ;;
      SigCgt:) caught=$((0x$value)) ;;
    esac
  done 2>/dev/null <"/proc/$pid/status" || return 1
  ((pending & 0x4000)) && return 1
  ((caught & 0x4000)) && return 0
  ((caught & 0x3)) && return 1
  return 0
}
sent=0
stop_app() {
  [ "$sent" -eq 0 ] || return 0
  sent=1
  # A stop meant for every process may still be on its way to the app.
  sleep 0.2
  if needs_term; then
    kill -s TERM "$pid" 2>/dev/null
  fi
}
trap stop_app HUP INT TERM

# \`wait\` also returns above 128 when a signal's trap interrupts it, with
# the app still running.
while :; do
  wait "$pid"
  status=$?
  [ "$status" -gt 128 ] || break
  kill -0 "$pid" 2>/dev/null && continue
  # Gone: this status may be the trap's, and the app's is still kept.
  wait "$pid"
  again=$?
  [ "$again" -eq 127 ] || status=$again
  break
done

# The relays end once nothing holds their pipes: the app and its helper
# processes. A restarted app (lib/relaunch.cjs) holds them on, so they
# are given a moment, not waited for.
for ((i = 0; i < 40; i++)); do
  alive=0
  for relay in "\${relays[@]}"; do
    kill -0 "$relay" 2>/dev/null && alive=1
  done
  [ "$alive" -eq 1 ] || break
  sleep 0.05
done
exit "$status"
`;
}

module.exports = { launcherScript };
