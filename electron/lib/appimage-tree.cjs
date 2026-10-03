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

module.exports = { openTreeToAll };
