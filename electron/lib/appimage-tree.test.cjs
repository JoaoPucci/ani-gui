// The permissions the AppImage's tree is packed with.
//
// The tree the repack extracts reaches mksquashfs with its directories
// at 0700, and `-all-root` makes them root's: every directory of every
// repacked release came out root-owned and closed to everyone else.
// The AppImage runtime mounts the image as the user who starts it, so
// a plain launch never noticed; anything that mounts it as root and
// runs the app as a user — a firejail sandbox, the AppImage catalog's
// test, a system-wide install — was refused at the root directory,
// `AppRun: Permission denied`. The repack opens the tree first.

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { openTreeToAll } = require('./appimage-tree.cjs');

const mode = (p) => fs.lstatSync(p).mode & 0o777;

/** A tree shaped like an extract: closed directories, an executable,
 *  a data file and a symlink. */
function extractLike() {
	const root = fs.mkdtempSync(path.join(os.tmpdir(), 'appimage-tree-'));
	const app = path.join(root, 'squashfs-root');
	fs.mkdirSync(path.join(app, 'resources', 'bin'), { recursive: true, mode: 0o700 });
	fs.writeFileSync(path.join(app, 'AppRun'), '#!/bin/sh\n', { mode: 0o700 });
	fs.writeFileSync(path.join(app, 'resources', 'app.asar'), 'data', { mode: 0o600 });
	fs.symlinkSync('app.asar', path.join(app, 'resources', 'link'));
	for (const dir of [app, path.join(app, 'resources'), path.join(app, 'resources', 'bin')]) {
		fs.chmodSync(dir, 0o700);
	}
	return { root, app };
}

test('every directory, the tree root included, opens to everyone', () => {
	const { root, app } = extractLike();
	try {
		openTreeToAll(app);
		for (const dir of [app, path.join(app, 'resources'), path.join(app, 'resources', 'bin')]) {
			assert.equal(mode(dir), 0o755, dir);
		}
	} finally {
		fs.rmSync(root, { recursive: true, force: true });
	}
});

test('every file is readable by everyone, and an executable runnable by everyone', () => {
	const { root, app } = extractLike();
	try {
		openTreeToAll(app);
		assert.equal(mode(path.join(app, 'AppRun')), 0o755);
		assert.equal(mode(path.join(app, 'resources', 'app.asar')), 0o644);
	} finally {
		fs.rmSync(root, { recursive: true, force: true });
	}
});

test('a symlink is left as it is, and its target is opened once, as a file', () => {
	const { root, app } = extractLike();
	try {
		const link = path.join(app, 'resources', 'link');
		openTreeToAll(app);
		assert.equal(fs.readlinkSync(link), 'app.asar');
		assert.ok(fs.lstatSync(link).isSymbolicLink());
		assert.equal(mode(path.join(app, 'resources', 'app.asar')), 0o644);
	} finally {
		fs.rmSync(root, { recursive: true, force: true });
	}
});
