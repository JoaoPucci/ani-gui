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
 * `--no-sandbox` goes between the binary and the user's args on both:
 * the image mounts read-only through FUSE, so `chrome-sandbox` inside
 * it cannot carry the SUID bit Chromium's setuid sandbox demands, and
 * the flag has to be on argv when the process spawns. Returns true
 * when the file was changed, false when it already carried the patch,
 * as an image a previous repack produced does. Throws when neither
 * pattern matches and the patch is absent: electron-builder's template
 * drifted and this needs an update.
 */
function patchAppRun(appRunPath) {
	const original = fs.readFileSync(appRunPath, 'utf8');
	if (original.includes('exec "$BIN" --no-sandbox')) return false;
	const patched = original
		.replace(/^(\s*)exec "\$BIN"$/m, '$1exec "$BIN" --no-sandbox')
		.replace(/^(\s*)exec "\$BIN" "\$\{args\[@\]\}"$/m, '$1exec "$BIN" --no-sandbox "${args[@]}"');
	if (patched === original) {
		throw new Error(
			`AppRun patch matched nothing — has electron-builder's template changed? ${appRunPath}`
		);
	}
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
