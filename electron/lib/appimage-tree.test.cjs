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

test('a symlink is never followed out of the tree', () => {
	const { root, app } = extractLike();
	try {
		const outside = path.join(root, 'outside');
		fs.mkdirSync(outside, { mode: 0o700 });
		fs.chmodSync(outside, 0o700);
		fs.symlinkSync(outside, path.join(app, 'resources', 'away'));
		openTreeToAll(app);
		assert.equal(mode(outside), 0o700);
	} finally {
		fs.rmSync(root, { recursive: true, force: true });
	}
});

test('a file already open to more keeps what it had', () => {
	const { root, app } = extractLike();
	try {
		const shared = path.join(app, 'resources', 'shared.dat');
		fs.writeFileSync(shared, 'data');
		fs.chmodSync(shared, 0o664);
		openTreeToAll(app);
		assert.equal(mode(shared), 0o664);
	} finally {
		fs.rmSync(root, { recursive: true, force: true });
	}
});

// What the image starts Electron with.
//
// The image cannot use Chromium's setuid sandbox: it runs from a
// read-only FUSE mount, so `chrome-sandbox` inside it cannot be SUID.
// Chromium's namespace sandbox needs no helper, but whether it starts
// depends on the host, and on Ubuntu 23.10+ on an AppArmor restriction
// whose exceptions are granted per binary, so nothing but the app's own
// binary can answer for it; a sandbox that fails to start aborts the
// launch. The repack therefore rewrites AppRun to start the binary once
// as the sandbox probe (lib/sandbox-probe.cjs) and pass `--no-sandbox`
// only when the probe gets no answer. electron-builder's own decisions
// go: 25 makes none (the repack used to add `--no-sandbox` to both
// exec lines), 26 adds the flag when `unshare -Ur true` fails — an
// answer for `/usr/bin/unshare`, not for the app — and both write the
// image's desktop entry as `Exec=AppRun --no-sandbox %U`, which a
// launch through desktop integration would pass as the user's own flag.
//
// The fixtures are the AppRun each electron-builder version writes for
// this app (25's as it shipped in the 0.12.0 image, with the repack's
// flag taken back out) and the desktop entry it writes.
const { prepareTree } = require('./appimage-tree.cjs');
const { SANDBOX_PROBE_SWITCH, SANDBOX_PROBE_ANSWER } = require('./sandbox-probe.cjs');
const { spawnSync } = require('node:child_process');

const fixture = (name) => fs.readFileSync(path.join(__dirname, 'fixtures', name), 'utf8');
const EB25 = fixture('AppRun.electron-builder-25');
const EB26 = fixture('AppRun.electron-builder-26');
const DESKTOP = fixture('ani-gui.desktop');
// What the repack wrote into every AppRun before this change.
const EB25_FLAGGED = EB25.replace(/^(\s*)exec "\$BIN"/gm, '$1exec "$BIN" --no-sandbox');

const EXEC_BARE = /^\s*exec "\$BIN" "\$\{ANI_GUI_SANDBOX\[@\]\}"$/m;
const EXEC_ARGS = /^\s*exec "\$BIN" "\$\{ANI_GUI_SANDBOX\[@\]\}" "\$\{args\[@\]\}"$/m;

/** An extract-shaped tree with this AppRun and desktop entry, closed. */
function withImage(appRun, desktop = DESKTOP) {
	const tree = extractLike();
	fs.writeFileSync(path.join(tree.app, 'AppRun'), appRun, { mode: 0o700 });
	fs.chmodSync(path.join(tree.app, 'AppRun'), 0o700);
	if (desktop !== null) {
		fs.writeFileSync(path.join(tree.app, 'ani-gui.desktop'), desktop, { mode: 0o600 });
	}
	fs.chmodSync(tree.app, 0o700);
	return tree;
}

const read = (app, name) => fs.readFileSync(path.join(app, name), 'utf8');

function inImage(appRun, fn, desktop) {
	const tree = withImage(appRun, desktop);
	try {
		fn(tree.app);
	} finally {
		fs.rmSync(tree.root, { recursive: true, force: true });
	}
}

for (const [name, template] of [
	['electron-builder 25', EB25],
	['electron-builder 26', EB26],
	['an image the repack flagged before', EB25_FLAGGED],
]) {
	test(`${name}: AppRun asks the app's own binary whether the sandbox starts`, () => {
		inImage(template, (app) => {
			assert.deepEqual(prepareTree(app), { appRun: true, desktop: true });
			const appRun = read(app, 'AppRun');
			assert.match(appRun, EXEC_BARE);
			assert.match(appRun, EXEC_ARGS);
			assert.doesNotMatch(appRun, /exec "\$BIN" --no-sandbox/, 'no flag fixed at build time');
			assert.doesNotMatch(appRun, /NO_SANDBOX\[@\]/, "electron-builder's array is not used");
			assert.doesNotMatch(appRun, /unshare/, "electron-builder's probe is gone");
			assert.ok(appRun.includes(`"$BIN" ${SANDBOX_PROBE_SWITCH}`), 'the probe starts the binary');
			assert.ok(appRun.includes(SANDBOX_PROBE_ANSWER), 'and waits for its answer');
			const syntax = spawnSync('bash', ['-n', path.join(app, 'AppRun')], { encoding: 'utf8' });
			assert.equal(syntax.status, 0, syntax.stderr);
		});
	});
}

test("the image's desktop entry no longer passes --no-sandbox as the user's own flag", () => {
	inImage(EB25, (app) => {
		prepareTree(app);
		const desktop = read(app, 'ani-gui.desktop');
		assert.match(desktop, /^Exec=AppRun %U$/m);
		assert.doesNotMatch(desktop, /--no-sandbox/);
		assert.equal(
			desktop,
			DESKTOP.replace('Exec=AppRun --no-sandbox %U', 'Exec=AppRun %U'),
			'every other line is kept',
		);
	});
});

test('a desktop entry that already starts AppRun plainly is kept', () => {
	inImage(
		EB25,
		(app) => {
			assert.deepEqual(prepareTree(app), { appRun: true, desktop: false });
			assert.match(read(app, 'ani-gui.desktop'), /^Exec=AppRun %U$/m);
		},
		DESKTOP.replace('Exec=AppRun --no-sandbox %U', 'Exec=AppRun %U'),
	);
});

// The repack once skipped an image whose AppRun already carried the
// patch, before it opened the tree: rerun over an image a previous
// repack had produced, it left that image's directories closed.
// Preparing a tree patches what needs it and opens the tree either way.
test('a tree prepared before is left as it is, and still opened', () => {
	inImage(EB26, (app) => {
		prepareTree(app);
		const appRun = read(app, 'AppRun');
		const desktop = read(app, 'ani-gui.desktop');
		fs.chmodSync(app, 0o700);
		fs.chmodSync(path.join(app, 'resources'), 0o700);
		assert.deepEqual(prepareTree(app), { appRun: false, desktop: false });
		assert.equal(read(app, 'AppRun'), appRun);
		assert.equal(read(app, 'ani-gui.desktop'), desktop);
		assert.equal(mode(app), 0o755);
		assert.equal(mode(path.join(app, 'resources')), 0o755);
		assert.equal(mode(path.join(app, 'AppRun')), 0o755);
	});
});

// A template that drifts must stop the repack rather than ship an
// AppRun that is half rewritten, or one whose sandbox decision is still
// electron-builder's. Everything is checked before anything is written,
// so a stopped repack leaves both files as they were.
function assertStops(appRun, desktop, pattern) {
	inImage(
		appRun,
		(app) => {
			assert.throws(() => prepareTree(app), pattern);
			assert.equal(read(app, 'AppRun'), appRun);
			if (desktop !== null) assert.equal(read(app, 'ani-gui.desktop'), desktop);
		},
		desktop,
	);
}

test('an AppRun with neither exec line stops the repack', () => {
	assertStops('#!/bin/sh\natexit()\n{\n  exec ./other\n}\n', DESKTOP, /AppRun patch matched nothing/);
});

test('an AppRun where only one exec line is recognised stops the repack', () => {
	const drifted = EB25.replace('exec "$BIN" "${args[@]}"', 'exec "$BIN" --some-new-flag "${args[@]}"');
	assertStops(drifted, DESKTOP, /AppRun patch matched one exec line of two/);
});

test('an AppRun without the atexit launcher the probe goes in front of stops the repack', () => {
	assertStops(EB25.replace(/^atexit\(\)$/m, 'launch()'), DESKTOP, /atexit/);
});

test("an electron-builder probe the repack does not recognise stops the repack", () => {
	const drifted = EB26.replace('! unshare -Ur true 2>/dev/null', '! unshare --user --map-root-user true');
	assertStops(drifted, DESKTOP, /unshare/);
});

test('a desktop entry starting AppRun some other way stops the repack', () => {
	const drifted = DESKTOP.replace('Exec=AppRun --no-sandbox %U', 'Exec=AppRun --no-sandbox --other %U');
	assertStops(EB25, drifted, /desktop entry/);
});

test('an image with no desktop entry at its root stops the repack', () => {
	assertStops(EB25, null, /desktop entry/);
});
