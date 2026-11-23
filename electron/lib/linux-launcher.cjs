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
# on_stop). The relays run in sessions of their own too, and ignore
# SIGHUP and SIGTERM, so the app's last lines still reach the terminal
# while it shuts down; once the terminal has gone, each ends at its next
# write to it, and the app's writes after that fail as a closed pipe's
# do, which it ignores (lib/stdio-guard.cjs). With setpriv, the app also
# gets a SIGTERM if this launcher is killed outright, rather than
# outliving it.
# The relays' pipes take descriptors bash picks, and only those are
# closed: every other one this launcher was given is passed on to the
# app, as the exec above passes it. The AppImage runtime gives one
# (fd 3): the end of a pipe that keeps the image mounted while any
# process holds it, so closing it unmounts the app's own files from
# under it.
out=1
err=2
opened=()
relays=()
if [ -t 1 ]; then
  exec {out}> >(trap '' HUP TERM; exec setsid cat)
  relays+=("$!")
  opened+=("$out")
fi
if [ -t 2 ]; then
  exec {err}> >(trap '' HUP TERM; for fd in "\${opened[@]}"; do exec {fd}>&-; done; exec setsid cat >&2)
  relays+=("$!")
  opened+=("$err")
fi
launch=(setsid)
if command -v setpriv >/dev/null 2>&1 && setpriv --pdeathsig TERM -- true 2>/dev/null; then
  launch+=(setpriv --pdeathsig TERM --)
fi
# A background job of this shell starts with SIGINT and SIGQUIT ignored;
# the app starts with the dispositions this launcher was given.
(
  trap - INT QUIT
  exec </dev/null >&"$out"
  exec 2>&"$err"
  for fd in "\${opened[@]}"; do exec {fd}>&-; done
  exec "\${launch[@]}" "\${app[@]}"
) &
pid=$!
for fd in "\${opened[@]}"; do exec {fd}>&-; done

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
# Hundredths of a second since boot, in \`now\`: a clock that a change
# of the date does not move, read without starting a process.
clock() {
  local up rest
  read -r up rest 2>/dev/null </proc/uptime || return 1
  now=$((10#\${up/./}))
}
# A pause of $1 hundredths of a second, inside this shell: a pipe only
# this shell holds, read until the time is up. A pause in a process of
# its own, as \`sleep\`, would be cut short by the stop it waits out (a
# Ctrl-C to the group, a scope's SIGTERM to every process), which only
# a disposition this shell set aside first could spare it. A trap runs
# during the read and the read carries on, so the clock is only a
# guard.
exec {pausefd}<> <(:)
pause() {
  local left=$1 end=
  clock && end=$((now + left))
  while ((left > 0)); do
    read -r -t "$((left / 100)).$((left / 10 % 10))$((left % 10))" -u "$pausefd" _
    [ -n "$end" ] && clock || return 0
    left=$((end - now))
  done
}
# Every stop comes here, and whether it counts is decided here, by the
# state the launcher is in when bash gets to it. Bash does not run a
# trap when its signal arrives: it marks the signal and runs the traps
# later, between commands, an inner pass inside another trap included.
# So a signal of one burst can be run long after the rest, and a trap
# swapped or set aside in the meantime would act for it, or have bash
# complain on the terminal about it. None is: these stay in place
# until the launcher exits.
# The first stop is handed on (stop_app). After it, a SIGINT or SIGTERM
# ends the app by force (force_app), once more than 0.3 s have passed
# since the first stop was handed on. The signals one event delivers
# come well inside that (a closed terminal's SIGHUPs and SIGCONT, a
# scope stop's SIGTERM and SIGHUP: under a millisecond apart), and a
# second Ctrl-C meant as one comes after them. A SIGHUP never forces:
# it says the terminal has gone, and the quit carries on without it.
# A trap can run between any two commands of another, so each test of
# the launcher's state and its record are one command.
sent=0
armed=0
handed=
reaped=0
on_stop() {
  ((reaped == 0)) || return 0
  if ((sent++ == 0)); then
    stop_app
    return 0
  fi
  [ "$1" != HUP ] && ((armed)) || return 0
  if [ -n "$handed" ] && clock && ((now - handed <= 30)); then
    return 0
  fi
  force_app
}
stop_app() {
  # A stop meant for every process may still be on its way to the app.
  # An app that quits within the pause can already be reaped after it,
  # its status file gone, and then nothing is sent; only a pid reused
  # within those 0.2 s would be mistaken for it.
  pause 20
  if needs_term; then
    kill -s TERM "$pid" 2>/dev/null
  fi
  clock && handed=$now
  armed=1
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
  # Read without a subshell: one would take a stop arriving now by
  # default, and die of it.
  local stat=
  { read -r -d '' stat <"/proc/$pid/stat"; } 2>/dev/null
  [ -n "$stat" ] || return 0
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
reaped=1

# The relays end once nothing holds their pipes: the app and its helper
# processes. A restarted app (lib/relaunch.cjs) holds them on, so they
# are given a moment, not waited for.
for ((i = 0; i < 40; i++)); do
  alive=0
  for relay in "\${relays[@]}"; do
    kill -0 "$relay" 2>/dev/null && alive=1
  done
  [ "$alive" -eq 1 ] || break
  pause 5
done
exit "$status"
}
main; exit
`;
}

module.exports = { launcherScript };
