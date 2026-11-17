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

# Everything below is one function, read whole before any of it runs:
# bash reads a script from its file as it goes, and the AppImage's file
# sits on a mount that can be gone before this launcher is done.
main() {

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
# SIGTERM, and ends it by force on a second Ctrl-C or kill (see
# stop_app). The relays run in sessions of their own too, and ignore
# SIGHUP and SIGTERM, so the app's last lines still reach the terminal
# while it shuts down; once the terminal has gone, each ends at its next
# write to it, and the app's writes after that fail as a closed pipe's
# do, which it ignores (lib/stdio-guard.cjs). With setpriv, the app also
# gets a SIGTERM if this launcher is killed outright, rather than
# outliving it.
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
# handler for SIGTERM has gone while one for SIGINT or SIGHUP remains (a
# stop that sends SIGHUP as well as SIGTERM resets both).
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
armed=
# Hundredths of a second since boot, in \`now\`: a clock that a change
# of the date does not move, read without starting a process.
clock() {
  local up rest
  read -r up rest </proc/uptime
  now=$((10#\${up/./}))
}
# Every stop comes here. The first is handed on (stop_app). After it, a
# SIGINT or SIGTERM ends the app by force (force_app), once 0.3 s have
# passed since the app was handed its SIGTERM. The signals one event
# delivers come well inside that (a closed terminal's SIGHUPs and
# SIGCONT, a scope stop's SIGTERM and SIGHUP: under a millisecond
# apart), and a second Ctrl-C meant as one comes after them. A SIGHUP
# never forces: it says the terminal has gone, and the quit carries on
# without it.
# The window is measured, not slept through with the stops set aside,
# because bash does not run a trap when its signal arrives. It marks
# the signal and runs the traps later, so a signal of the first stop's
# burst can be marked before stop_app has set the rest aside, and run
# only once stop_app is done: it is still one of the first stop's.
# Bash can also run a trap between any two commands of another, so the
# test for the first stop and its record are one command.
on_stop() {
  if ((sent++ == 0)); then
    stop_app
    return 0
  fi
  [ "$1" != HUP ] && [ -n "$armed" ] || return 0
  clock
  ((now - armed >= 30)) || return 0
  force_app
}
stop_app() {
  # For the pause below, the launcher ignores the rest: nothing more is
  # passed on, and the pause, which inherits that, is not cut short by
  # the same stop reaching it too (a Ctrl-C to the group, a scope's
  # SIGTERM to every process).
  trap '' HUP INT TERM
  # A stop meant for every process may still be on its way to the app.
  # An app that quits within the pause can already be reaped after it,
  # its status file gone, and then nothing is sent; only a pid reused
  # within those 0.2 s would be mistaken for it.
  sleep 0.2
  if needs_term; then
    kill -s TERM "$pid" 2>/dev/null
  fi
  clock
  armed=$now
  trap 'on_stop HUP' HUP
  trap 'on_stop INT' INT
  trap 'on_stop TERM' TERM
}
# A SIGKILL to the app's process group, which setsid made its own: the
# app and the helpers it started there. The backend leads a group of
# its own and stops itself once the app's end of its stdin closes
# (backend/src/parent_watch.rs). An app that has exited is left alone:
# bash reaps it as soon as it exits, so its status file is gone, or it
# is a zombie for the moment before that. (Only for the moment before
# the wait below notices: after it, every stop is ignored.) While any
# helper is left in the group, its id is not given to another process.
forced=0
force_app() {
  ((forced++ == 0)) || return 0
  trap '' HUP INT TERM
  local stat
  { stat=$(<"/proc/$pid/stat"); } 2>/dev/null || return 0
  case "\${stat##*) }" in
    [ZXx]*) return 0 ;;
  esac
  # A group already gone by now gets no SIGKILL, and no line claims one.
  kill -s KILL -- "-$pid" 2>/dev/null || return 0
  # A closed stream must not end the launcher before it has the app's
  # status.
  trap '' PIPE
  printf '%s\\n' "[launcher] a second stop: ani-gui ended by force (SIGKILL); what it changed in its last moments may not be saved" >&2
}
trap 'on_stop HUP' HUP
trap 'on_stop INT' INT
trap 'on_stop TERM' TERM

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
# A stop after this point has no app to act for: it is reaped.
trap '' HUP INT TERM

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
}
main; exit
`;
}

module.exports = { launcherScript };
