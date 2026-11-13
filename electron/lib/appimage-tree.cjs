// Opens an extracted AppImage tree to every user before it is packed.
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

/**
 * Patch an extracted AppRun in place. electron-builder generates an
 * AppRun whose two exec lines are exactly:
 *
 *   exec "$BIN"
 *   exec "$BIN" "${args[@]}"
 *
 * or, from electron-builder 26 on, the same two lines with
 * `"${NO_SANDBOX[@]}"` after the binary — an array that carries
 * `--no-sandbox` only when an `unshare -Ur true` probe fails.
 *
 * `--no-sandbox` goes between the binary and the user's args on both,
 * in place of that array where it is present. The image mounts
 * read-only through FUSE, so `chrome-sandbox` inside it cannot carry
 * the SUID bit Chromium's setuid sandbox demands, and the flag has to
 * be on argv when the process spawns. The probe would keep Chromium's
 * namespace sandbox where user namespaces work, but `unshare`
 * succeeding does not prove Chromium's own sandbox will start —
 * AppArmor's user-namespace restriction grants its exceptions per
 * binary, so `unshare` and the app can get different answers — and a
 * failed start aborts the launch. So the flag stays unconditional, as
 * it was before electron-builder 26, at the cost of the namespace
 * sandbox on hosts where it would have worked. The probe itself still
 * runs; its result goes unused.
 *
 * Returns true when the file was changed, false when it already
 * carried the patch, as an image a previous repack produced does.
 * Throws, leaving the file as it was, unless both exec lines match:
 * electron-builder's template drifted and this needs an update.
 */
const EXEC_BARE = /^(\s*)exec "\$BIN"(?: "\$\{NO_SANDBOX\[@\]\}")?$/m;
const EXEC_ARGS = /^(\s*)exec "\$BIN"(?: "\$\{NO_SANDBOX\[@\]\}")? "\$\{args\[@\]\}"$/m;

function patchAppRun(appRunPath) {
	const original = fs.readFileSync(appRunPath, 'utf8');
	if (original.includes('exec "$BIN" --no-sandbox')) return false;
	const matched = [EXEC_BARE, EXEC_ARGS].filter((re) => re.test(original)).length;
	if (matched === 0) {
		throw new Error(
			`AppRun patch matched nothing — has electron-builder's template changed? ${appRunPath}`
		);
	}
	if (matched === 1) {
		throw new Error(
			`AppRun patch matched one exec line of two — has electron-builder's template changed? ${appRunPath}`
		);
	}
	const patched = original
		.replace(EXEC_BARE, '$1exec "$BIN" --no-sandbox')
		.replace(EXEC_ARGS, '$1exec "$BIN" --no-sandbox "${args[@]}"');
	fs.writeFileSync(appRunPath, patched, { mode: 0o755 });
	return true;
}

/**
 * Everything an extracted tree needs before it is packed: AppRun
 * patched when it does not carry the patch yet, and the tree opened
 * either way — an image a previous repack produced carries the patch
 * and may still have closed directories. Returns whether AppRun was
 * patched.
 */
function prepareTree(appDir) {
	const appRunPath = path.join(appDir, 'AppRun');
	if (!fs.existsSync(appRunPath)) {
		throw new Error(`expected AppRun at ${appRunPath} after extract`);
	}
	const patched = patchAppRun(appRunPath);
	openTreeToAll(appDir);
	return patched;
}

module.exports = { openTreeToAll, prepareTree };
