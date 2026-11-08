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

// The launcher the probe goes in front of, at the top level of the script.
const LAUNCHER = /^atexit\(\)$/gm;

const PROBE_MARK = "# ani-gui: Chromium's sandbox, decided by the app's own binary.";

// Written into AppRun ahead of the launcher. The probe is the app's own
// binary started with the probe switch: Chromium sets its sandbox up
// before main.js runs and aborts the start when it cannot, so an answer
// from main.js means the sandbox came up for this binary on this host.
// Its stdout goes to a file, not a pipe, so a child that outlives it
// cannot hold the wait open; core dumps are off and its stderr and the
// shell's job reports are dropped, so a failed probe leaves no trace but
// the one line below. A watchdog kills a probe that has not returned by
// the bound — with bash's own `wait`, not coreutils `timeout`.
const PROBE_BLOCK = `${PROBE_MARK}
#
# The image cannot use Chromium's setuid sandbox: its read-only mount
# cannot carry chrome-sandbox's SUID bit. Whether the namespace sandbox
# starts depends on the host, and on Ubuntu 23.10+ AppArmor decides it
# per binary, while a sandbox that cannot start aborts the launch. So
# the binary is started once as a probe, and answers from main.js only
# once its sandbox is up. No answer within ${PROBE_BOUND_SECONDS} s, and the launch goes
# ahead with --no-sandbox. Written by electron/lib/appimage-tree.cjs.
ANI_GUI_SANDBOX=()
ani_gui_sandbox_starts()
{
  local answer_file answer status
  answer_file="$(mktemp 2>/dev/null)" || return 1
  status=0
  (
    exec 2>/dev/null
    ulimit -c 0
    "$BIN" ${SANDBOX_PROBE_SWITCH} </dev/null >"$answer_file" &
    probe=$!
    (
      trap 'kill "$sleeper"; exit 0' TERM
      sleep ${PROBE_BOUND_SECONDS} &
      sleeper=$!
      wait "$sleeper"
      kill -9 "$probe"
    ) </dev/null >/dev/null &
    watchdog=$!
    probe_status=0
    wait "$probe" || probe_status=$?
    kill "$watchdog" || true
    wait "$watchdog" || true
    exit "$probe_status"
  ) || status=$?
  answer="$(cat "$answer_file" 2>/dev/null)" || answer=
  rm -f "$answer_file"
  [ "$status" -eq 0 ] && [ "$answer" = "${SANDBOX_PROBE_ANSWER}" ]
}
ani_gui_user_no_sandbox=0
for arg in "\${args[@]}" ; do
  if [ "$arg" = --no-sandbox ] ; then
    ani_gui_user_no_sandbox=1
  fi
done
if [ -z "$APPIMAGE_EXIT_AFTER_INSTALL" ] && [ $ani_gui_user_no_sandbox -eq 0 ] && ! ani_gui_sandbox_starts ; then
  ANI_GUI_SANDBOX=(--no-sandbox)
  echo "ani-gui: Chromium's sandbox cannot start on this system. Starting without it (--no-sandbox)." >&2
fi

`;

const count = (text, re) => (text.match(re) || []).length;

/**
 * The AppRun electron-builder wrote, with the sandbox decided by the
 * probe: the probe block ahead of the launcher, electron-builder 26's
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
		.replace(LAUNCHER, () => `${PROBE_BLOCK}atexit()`)
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
