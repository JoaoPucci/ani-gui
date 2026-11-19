// Prepares an extracted AppImage tree before it is packed: AppRun and
// the desktop entry are rewritten so the app's own binary decides at
// launch whether Chromium's sandbox runs (see PROBE_BLOCK below), and
// the tree is opened to every user.
//
// The extracted tree reaches the repack with its directories at 0700,
// and the repack packs with `-all-root`, so without this every
// directory in the image is root-owned and closed to everyone else.
// A launch the runtime mounts as the starting user never notices;
// anything that mounts the image as root and runs the app as a user —
// a firejail sandbox, a system-wide install — is refused at the root
// directory.

const fs = require('node:fs');
const path = require('node:path');

/**
 * Walks `root` without following symlinks: every directory becomes
 * 0755, every file gains read for everyone, and a file executable by
 * its owner gains execute for everyone. Symlinks are left as they
 * are; their targets are opened where the walk meets them.
 */
function openTreeToAll(root) {
	const stat = fs.lstatSync(root);
	if (stat.isSymbolicLink()) return;
	if (stat.isDirectory()) {
		fs.chmodSync(root, 0o755);
		for (const entry of fs.readdirSync(root)) {
			openTreeToAll(path.join(root, entry));
		}
		return;
	}
	const ownerExec = stat.mode & 0o100;
	fs.chmodSync(root, (stat.mode & 0o777) | 0o444 | (ownerExec ? 0o111 : 0));
}

const { SANDBOX_PROBE_SWITCH, SANDBOX_PROBE_ANSWER } = require('./sandbox-probe.cjs');

/** How long AppRun waits for the probe before it starts the app unsandboxed. */
const PROBE_BOUND_SECONDS = 10;

// electron-builder writes the launch as two exec lines inside its
// `atexit` trap: `exec "$BIN"` and `exec "$BIN" "${args[@]}"`. Version
// 25 writes them bare, version 26 with its `"${NO_SANDBOX[@]}"` array
// after the binary, and images this repack made before carry
// `--no-sandbox` there.
const EXEC_BARE = /^(\s*)exec "\$BIN"(?: --no-sandbox| "\$\{NO_SANDBOX\[@\]\}")?$/gm;
const EXEC_ARGS = /^(\s*)exec "\$BIN"(?: --no-sandbox| "\$\{NO_SANDBOX\[@\]\}")? "\$\{args\[@\]\}"$/gm;
const PATCHED_BARE = 'exec "$BIN" "${ANI_GUI_SANDBOX[@]}"';
const PATCHED_ARGS = 'exec "$BIN" "${ANI_GUI_SANDBOX[@]}" "${args[@]}"';

// electron-builder 26's own decision: `--no-sandbox` when
// `unshare -Ur true` fails. Removed, so the binary's answer is the only
// one. The `HAVE_NO_SANDBOX` loop above it stays, unused.
const EB26_PROBE =
	/^NO_SANDBOX=\(\)\n(?:#.*\n)*if \[ \$HAVE_NO_SANDBOX -eq 0 \] && ! unshare -Ur true 2>\/dev\/null ; then\n {2}NO_SANDBOX=\(--no-sandbox\)\nfi\n/m;

// electron-builder's launcher: the function its EXIT trap runs, which
// execs the app. The trap is set before the function is defined, so
// the probe goes after the definition: an exit while the probe runs
// that is not an interrupt finds the launcher defined, rather than
// "atexit: command not found", and launches with the default below.
const LAUNCHER = /^atexit\(\)\n\{\n(?:.*\n)*?\}\n/gm;

const PROBE_MARK = "# ani-gui: Chromium's sandbox, decided by the app's own binary.";

// Written into AppRun after the launcher. The probe is the app's own
// binary started with the probe switch: Chromium sets its sandbox up
// before main.js runs and aborts the start when it cannot, so an answer
// from main.js means the sandbox came up for this binary on this host.
//
// - `--no-sandbox` is the default until the probe has answered, so
//   an exit before then launches the app unsandboxed rather than not
//   at all; the fallback relies on it. Its one line of output cannot
//   stop the launch. A write that fails (a closed stderr, a full disk)
//   returns a status, which `|| true` absorbs. A write to a pipe whose
//   reader has gone raises SIGPIPE, so the line is written from a
//   subshell that ignores it, and fails the same way. The ignored
//   SIGPIPE stays in that subshell: the app starts with the
//   disposition AppRun was given. (Without the subshell the app would
//   still start: with an EXIT trap set, as electron-builder's AppRun
//   sets one, bash catches SIGPIPE and runs that trap, the launcher.
//   The subshell keeps AppRun on its ordinary path instead.) The
//   block's place after the launcher covers any other exit before the
//   launch.
// - The probe's stdout goes to a file, not a pipe, so a child that
//   outlives it cannot hold the wait open. Its stderr and the shell's
//   job reports are dropped, so the only output a failed probe leaves
//   is the one line below.
// - The probe runs with a core limit of exactly 1 byte, set with
//   util-linux's prlimit (bash's ulimit counts kilobytes). A limit of 0
//   stops a core file, not the crash reporter: a core_pattern that
//   pipes to one (apport, systemd-coredump, ABRT) starts it whatever
//   the limit, and systemd-coredump records the crash with no core,
//   which KDE's DrKonqi shows as a crash notification. For a limit of
//   1 the kernel starts no pipe at all ("RLIMIT_CORE is set to 1,
//   aborting core", fs/coredump.c: the value it gives its own helpers
//   so that their crashes do not recurse), and 1 byte is below any
//   core file's size. Where 1 cannot be set (no prlimit, a hard limit
//   of 0 already, or a call the host refuses) the probe runs with core
//   dumps off. Not covered either way: a core_pattern that is a socket
//   (systemd-coredump from systemd 262 on, which registers one on Linux
//   6.19 or later), for which the kernel reads no limit. The host may
//   still log the abort (kernel trap and coredump lines, AppArmor
//   audit).
// - $BIN is not the binary but the wrapper script both packages put in
//   front of it (lib/linux-launcher.cjs). Given no terminal on any of
//   its three streams, as here, the wrapper execs the binary, so the
//   probe this block waits on and kills is the binary itself.
// - A watchdog kills a probe that has not returned by the bound, with
//   bash's own `wait`, not coreutils `timeout`.
// - Every child the block signals, it then waits for, the watchdog's
//   sleeper included: a child left unreaped goes to PID 1, and a
//   container's PID 1 that does not reap keeps it as a zombie. The
//   probe's subshells stop that way on SIGHUP as well as SIGTERM, so a
//   hangup to the whole group (the terminal closing) reaps them too.
//   Each stop handler ignores further stop signals before it starts
//   waiting: a second one (the subshell gets AppRun's SIGTERM as well
//   as the group's hangup) would otherwise cut a wait short, and the
//   handler would exit with a child still unreaped. And each handler
//   waits through ani_gui_reap, not one `wait`: a second signal that
//   lands before the handler's first line, together with the first,
//   stays recorded in bash even once ignored, and can end any later
//   `wait` at once with the child still running, or still uncollected.
//   ani_gui_reap waits again until the child is gone, within a bound.
//   That covers the launcher's own children. Chromium's helper
//   processes (its zygotes) are forked by the binary and outlive it,
//   the probe's as the app's own do at every quit, so on such a host
//   only a PID 1 that reaps (`docker run --init`) collects them.
// - The probe runs as a background job the launcher waits on, so an
//   interrupt (Ctrl-C, SIGTERM, or SIGHUP when the terminal closes) is
//   handled at once. It stops the probe and its watchdog, removes the
//   answer file, and ends AppRun by that same signal without launching
//   anything, so a shell that started it stops as well.
const PROBE_BLOCK = `${PROBE_MARK}
#
# The image cannot use Chromium's setuid sandbox: its read-only mount
# cannot carry chrome-sandbox's SUID bit. Whether the namespace sandbox
# starts depends on the host, and on Ubuntu 23.10+ AppArmor decides it
# per binary, while a sandbox that cannot start aborts the launch. So
# the binary is started once as a probe, and answers from main.js only
# once its sandbox is up. No answer within ${PROBE_BOUND_SECONDS} s, and the launch goes
# ahead with --no-sandbox. Written by electron/lib/appimage-tree.cjs.
ANI_GUI_SANDBOX=(--no-sandbox)
ani_gui_answer_file=
ani_gui_probe_job=
ani_gui_probe_starting=0
ani_gui_probe_prev_bg=
# Waits for child $1 until it has ended, however often a signal bash
# still holds cuts the wait short. It goes on only while the kernel
# names this shell as the pid's parent: bash answers a child it has
# already collected with the remembered status, not "not a child", and
# the pid may by then be another process's. A cut-short wait can also
# leave a dead child uncollected, so each retry runs one short sleep,
# which bash waits for in the foreground, and the retries are bounded
# (about a second): past that the child is left to PID 1 rather than the
# stop hanging. Builtins only otherwise, and statuses kept off set -e,
# which electron-builder's AppRun runs under.
ani_gui_reap()
{
  local rc stat tries=0
  while true ; do
    rc=0
    wait "$1" 2>/dev/null || rc=$?
    [ "$rc" -ne 127 ] || return 0
    { read -r stat < "/proc/$1/stat" ; } 2>/dev/null || return 0
    # After the name, which ends at the last ")": state, then parent.
    stat=\${stat##*) }
    stat=\${stat#* }
    [ "\${stat%% *}" = "$BASHPID" ] || return 0
    tries=$((tries + 1))
    [ "$tries" -le 100 ] || return 0
    sleep 0.01 </dev/null >/dev/null 2>&1 || true
  done
}
ani_gui_probe_stop()
{
  # A signal between starting the job and recording it finds it in $!,
  # once $! has moved past whatever background job AppRun had before.
  # At any other time $! names nothing of the probe's.
  local job="$ani_gui_probe_job"
  trap '' INT TERM HUP
  if [ -z "$job" ] && [ "$ani_gui_probe_starting" -eq 1 ] && [ "$!" != "$ani_gui_probe_prev_bg" ] ; then
    job=$!
  fi
  if [ -n "$job" ] ; then
    kill "$job" 2>/dev/null || true
    ani_gui_reap "$job"
  fi
  if [ -n "$ani_gui_answer_file" ] ; then
    rm -f "$ani_gui_answer_file"
  fi
  trap - EXIT INT TERM HUP
  kill -s "$1" $$
  exit 1
}
ani_gui_sandbox_starts()
{
  local answer status
  ani_gui_answer_file="$(mktemp 2>/dev/null)" || return 1
  ani_gui_probe_prev_bg=$!
  ani_gui_probe_starting=1
  (
    exec 2>/dev/null
    # As in the stop handler: $! stands in for a child started but not
    # yet recorded, never for one this subshell inherited or reaped.
    prev_bg=$!
    probe=
    watchdog=
    # Each child signalled here is also waited for: one still unreaped
    # when this subshell exits goes to PID 1, and in a container whose
    # PID 1 does not reap, it stays there as a zombie.
    stop_children()
    {
      trap '' TERM HUP
      if [ -n "$probe" ] ; then kill -9 "$probe" || true ; ani_gui_reap "$probe" ; elif [ -z "$watchdog" ] && [ "$!" != "$prev_bg" ] ; then kill -9 "$!" || true ; ani_gui_reap "$!" ; fi
      if [ -n "$watchdog" ] ; then kill "$watchdog" || true ; ani_gui_reap "$watchdog" ; elif [ -n "$probe" ] && [ "$!" != "$probe" ] ; then kill "$!" || true ; ani_gui_reap "$!" ; fi
      exit 143
    }
    trap stop_children TERM HUP
    prlimit --pid "$BASHPID" --core=1:1 >/dev/null 2>&1 || ulimit -c 0
    "$BIN" ${SANDBOX_PROBE_SWITCH} </dev/null >"$ani_gui_answer_file" &
    probe=$!
    (
      sleeper=
      trap 'trap "" TERM HUP ; if [ -n "$sleeper" ] ; then kill "$sleeper" || true ; ani_gui_reap "$sleeper" ; elif [ "$!" != "$probe" ] ; then kill "$!" || true ; ani_gui_reap "$!" ; fi ; exit 0' TERM HUP
      sleep ${PROBE_BOUND_SECONDS} &
      sleeper=$!
      wait "$sleeper"
      kill -9 "$probe"
    ) </dev/null >/dev/null &
    watchdog=$!
    probe_status=0
    wait "$probe" || probe_status=$?
    probe=
    kill "$watchdog" || true
    wait "$watchdog" || true
    watchdog=
    exit "$probe_status"
  ) &
  ani_gui_probe_job=$!
  ani_gui_probe_starting=0
  status=0
  wait "$ani_gui_probe_job" 2>/dev/null || status=$?
  ani_gui_probe_job=
  ani_gui_probe_prev_bg=
  answer="$(cat "$ani_gui_answer_file" 2>/dev/null)" || answer=
  rm -f "$ani_gui_answer_file"
  [ "$status" -eq 0 ] && [ "$answer" = "${SANDBOX_PROBE_ANSWER}" ]
}
ani_gui_user_no_sandbox=0
for arg in "\${args[@]}" ; do
  if [ "$arg" = --no-sandbox ] ; then
    ani_gui_user_no_sandbox=1
  fi
done
if [ -n "$APPIMAGE_EXIT_AFTER_INSTALL" ] || [ $ani_gui_user_no_sandbox -eq 1 ] ; then
  ANI_GUI_SANDBOX=()
else
  trap 'ani_gui_probe_stop INT' INT
  trap 'ani_gui_probe_stop TERM' TERM
  trap 'ani_gui_probe_stop HUP' HUP
  if ani_gui_sandbox_starts ; then
    ANI_GUI_SANDBOX=()
  else
    ( trap '' PIPE ; echo "ani-gui: Chromium's sandbox cannot start on this system. Starting without it (--no-sandbox)." >&2 ) || true
  fi
  trap - INT TERM HUP
fi
`;

const count = (text, re) => (text.match(re) || []).length;

/**
 * The AppRun electron-builder wrote, with the sandbox decided by the
 * probe: the probe block after the launcher, electron-builder 26's
 * `unshare` decision removed, and both exec lines passing the probe's
 * array. Returns null for an AppRun that already carries it. Throws on
 * anything it does not recognise: electron-builder's template drifted
 * and this needs an update.
 */
function rewriteAppRun(original, appRunPath) {
	const drifted = (what) =>
		new Error(`AppRun patch ${what} — has electron-builder's template changed? ${appRunPath}`);
	if (original.includes(PROBE_MARK)) {
		if (!original.includes(PATCHED_BARE) || !original.includes(PATCHED_ARGS)) {
			throw drifted('found the probe without both of its exec lines');
		}
		return null;
	}
	const execs = count(original, EXEC_BARE) + count(original, EXEC_ARGS);
	if (execs === 0) throw drifted('matched nothing');
	if (count(original, EXEC_BARE) !== 1 || count(original, EXEC_ARGS) !== 1) {
		throw drifted(execs === 1 ? 'matched one exec line of two' : `matched ${execs} exec lines, not two`);
	}
	let text = original;
	if (text.includes('unshare')) {
		if (!EB26_PROBE.test(text)) throw drifted("did not recognise electron-builder's unshare probe");
		text = text.replace(EB26_PROBE, '');
	}
	if (count(text, LAUNCHER) !== 1) throw drifted('did not find the one atexit() launcher');
	return text
		.replace(LAUNCHER, (launcher) => `${launcher}\n${PROBE_BLOCK}`)
		.replace(EXEC_BARE, (_, indent) => `${indent}${PATCHED_BARE}`)
		.replace(EXEC_ARGS, (_, indent) => `${indent}${PATCHED_ARGS}`);
}

// electron-builder writes the image's desktop entry as
// `Exec=AppRun --no-sandbox %U` (its `appImage.executableArgs`, which
// version 25 cannot set empty). A launch through desktop integration
// would pass that flag as the user's own, and skip the probe.
const DESKTOP_FLAGGED = 'Exec=AppRun --no-sandbox %U';
const DESKTOP_PLAIN = 'Exec=AppRun %U';

/**
 * The image's desktop entry starting AppRun with no flag of its own, or
 * null when it already does. Throws on any other Exec line.
 */
function rewriteDesktop(original, desktopPath) {
	const execs = original.match(/^Exec=.*$/gm) || [];
	if (execs.length === 1 && execs[0] === DESKTOP_PLAIN) return null;
	if (execs.length === 1 && execs[0] === DESKTOP_FLAGGED) {
		return original.replace(DESKTOP_FLAGGED, DESKTOP_PLAIN);
	}
	throw new Error(
		`desktop entry starts AppRun some other way (${execs.join(' | ') || 'no Exec line'}) — has electron-builder changed? ${desktopPath}`
	);
}

function desktopEntryOf(appDir) {
	const entries = fs.readdirSync(appDir).filter((f) => f.endsWith('.desktop'));
	if (entries.length !== 1) {
		throw new Error(`expected one desktop entry at the image root, found ${entries.length}: ${appDir}`);
	}
	return path.join(appDir, entries[0]);
}

/**
 * Everything an extracted tree needs before it is packed: AppRun and the
 * desktop entry rewritten when they need it, and the tree opened either
 * way — an image a previous repack produced may still have closed
 * directories. Both files are checked before either is written, so a
 * repack that stops leaves them as they were. Returns which of the two
 * were changed.
 */
function prepareTree(appDir) {
	const appRunPath = path.join(appDir, 'AppRun');
	if (!fs.existsSync(appRunPath)) {
		throw new Error(`expected AppRun at ${appRunPath} after extract`);
	}
	const appRun = rewriteAppRun(fs.readFileSync(appRunPath, 'utf8'), appRunPath);
	const desktopPath = desktopEntryOf(appDir);
	const desktop = rewriteDesktop(fs.readFileSync(desktopPath, 'utf8'), desktopPath);
	if (appRun !== null) fs.writeFileSync(appRunPath, appRun, { mode: 0o755 });
	if (desktop !== null) fs.writeFileSync(desktopPath, desktop);
	openTreeToAll(appDir);
	return { appRun: appRun !== null, desktop: desktop !== null };
}

module.exports = { openTreeToAll, prepareTree, PROBE_BOUND_SECONDS };
