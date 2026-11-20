// The shape rules every bundled-dependency entry has to satisfy, and
// the one decision that branches on them.
//
// Both fetch scripts (Linux, Windows) declare their deps as data and
// walk that list. A malformed entry used to surface as a confusing
// mid-download failure — tar hunting for an archive path that was
// never declared — which is a slow way to learn about a typo. These
// are the checks that turn it into an immediate, named error.

const test = require('node:test');
const assert = require('node:assert/strict');

const {
	needsExtraction,
	assertDepShape,
	retiredStagedBinaries,
	parityGaps,
} = require('./dep-staging.cjs');

const archived = {
	name: 'fzf',
	archiveName: 'fzf.tar.gz',
	binary: 'fzf',
	archivePath: 'fzf',
};
const direct = {
	name: 'yt-dlp',
	archiveName: 'yt-dlp_linux',
	binary: 'yt-dlp',
	directBinary: true,
};

test('an archive dep needs extracting', () => {
	assert.equal(needsExtraction(archived), true);
});

test('a self-contained executable does not', () => {
	// Upstream publishes the binary itself as the release asset, so
	// the verified download IS the binary.
	assert.equal(needsExtraction(direct), false);
});

test('a dep that is neither is rejected by name', () => {
	// The failure mode this replaces: tar looking for an undeclared
	// path, several MB into a download.
	assert.throws(
		() => assertDepShape({ name: 'broken', archiveName: 'x.tar.gz', binary: 'x' }),
		/broken.*archivePath/,
	);
});

test('a dep claiming both is rejected, because only one can be honoured', () => {
	assert.throws(
		() => assertDepShape({ ...direct, archivePath: 'somewhere/yt-dlp' }),
		/yt-dlp.*directBinary/,
	);
});

test('a well-formed dep of either kind passes', () => {
	assert.doesNotThrow(() => assertDepShape(archived));
	assert.doesNotThrow(() => assertDepShape(direct));
});

test('a binary dropped from DEPS is named for removal', () => {
	// The staging dir is copied wholesale into the package via
	// `extraResources`, and each dep only ever overwrites the one file
	// it owns. So retiring an entry leaves its binary behind in any
	// checkout that staged it before, and the next build ships a tool
	// the app cannot invoke — silently, because nothing reads the
	// directory back.
	const staged = ['curl-impersonate', 'yt-dlp', 'fzf', 'aria2c'];
	const deps = [{ binary: 'curl-impersonate' }, { binary: 'yt-dlp' }];
	assert.deepEqual(retiredStagedBinaries(staged, deps).sort(), ['aria2c', 'fzf']);
});

test('a directory holding exactly the declared set is left alone', () => {
	const deps = [{ binary: 'yt-dlp' }];
	assert.deepEqual(retiredStagedBinaries(['yt-dlp'], deps), []);
});

test('an empty directory yields nothing to remove', () => {
	// First run on a fresh clone. Nothing staged yet, so nothing stale.
	assert.deepEqual(retiredStagedBinaries([], [{ binary: 'yt-dlp' }]), []);
});

test('several deps sharing one archive each keep their own binary', () => {
	// Linux stages the impersonate binary plus a wrapper per browser
	// out of a single tarball. Each is its own entry with its own
	// `binary`, so a name-based sweep must not treat the wrappers as
	// strays just because they share a `dep`.
	const deps = [
		{ binary: 'curl-impersonate' },
		{ binary: 'curl_firefox135' },
		{ binary: 'curl_chrome136' },
	];
	const staged = ['curl-impersonate', 'curl_firefox135', 'curl_chrome136', 'fzf'];
	assert.deepEqual(retiredStagedBinaries(staged, deps), ['fzf']);
});

// ── one-platform dependencies ───────────────────────────────────────
//
// The two fetchers are held to the same dependency set, because a
// runtime dependency that lands on one packaged platform alone is how
// the transport gap shipped. A dependency that genuinely belongs to
// one platform says so on its own entry, with the reason, and the
// parity comparison honours that declaration and nothing else.

const linuxOnly = { platform: 'linux', reason: 'the reason' };
const winOnly = { platform: 'win32', reason: 'the reason' };

test('a one-platform declaration names a packaged platform and a reason', () => {
	assert.doesNotThrow(() => assertDepShape({ ...direct, onlyOn: winOnly }));
	assert.doesNotThrow(() => assertDepShape({ ...direct, onlyOn: linuxOnly }));
	assert.throws(
		() => assertDepShape({ ...direct, onlyOn: { platform: 'darwin', reason: 'x' } }),
		/yt-dlp.*onlyOn/,
	);
	assert.throws(() => assertDepShape({ ...direct, onlyOn: { platform: 'win32' } }), /yt-dlp.*reason/);
	assert.throws(
		() => assertDepShape({ ...direct, onlyOn: { platform: 'win32', reason: '  ' } }),
		/yt-dlp.*reason/,
	);
	assert.throws(() => assertDepShape({ ...direct, onlyOn: 'win32' }), /yt-dlp.*onlyOn/);
	// A name every object inherits is not a platform.
	assert.throws(
		() => assertDepShape({ ...direct, onlyOn: { platform: 'toString', reason: 'x' } }),
		/yt-dlp.*onlyOn/,
	);
});

const shared = (name) => ({ name, dep: name });

test('the same set on both platforms has no gaps', () => {
	assert.deepEqual(parityGaps([shared('yt-dlp')], [shared('yt-dlp')]), []);
});

test('an undeclared dependency on one side is a gap, in either direction', () => {
	const gaps = parityGaps([shared('yt-dlp'), shared('aria2')], [shared('yt-dlp'), shared('ca')]);
	assert.equal(gaps.length, 2);
	assert.match(gaps.join('\n'), /aria2 is staged for Linux but not for Windows/);
	assert.match(gaps.join('\n'), /ca is staged for Windows but not for Linux/);
});

test('a dependency declared for the one platform that stages it is not a gap', () => {
	const ca = { name: 'ca', dep: 'ca', onlyOn: winOnly };
	assert.deepEqual(parityGaps([shared('yt-dlp')], [shared('yt-dlp'), ca]), []);
	const helper = { name: 'helper', dep: 'helper', onlyOn: linuxOnly };
	assert.deepEqual(parityGaps([shared('yt-dlp'), helper], [shared('yt-dlp')]), []);
});

test('a declaration naming the other platform is a gap', () => {
	// Staged by the Linux fetcher, declared as Windows-only: one of the
	// two is wrong, and nothing says which.
	const ca = { name: 'ca', dep: 'ca', onlyOn: winOnly };
	const gaps = parityGaps([shared('yt-dlp'), ca], [shared('yt-dlp')]);
	assert.equal(gaps.length, 1);
	assert.match(gaps[0], /ca .*win32.*Linux/);
});

test('every entry of a one-platform dependency carries the declaration', () => {
	// A dependency can span several entries (Linux stages a binary and
	// its wrappers out of one archive); one undeclared entry leaves the
	// dependency undeclared.
	const gaps = parityGaps(
		[shared('yt-dlp')],
		[shared('yt-dlp'), { name: 'ca', dep: 'ca', onlyOn: winOnly }, { name: 'ca-2', dep: 'ca' }],
	);
	assert.equal(gaps.length, 1);
	assert.match(gaps[0], /ca is staged for Windows but not for Linux/);
});

test('a declaration on a dependency both platforms stage is stale, and a gap', () => {
	// The exemption outlived its reason: the other platform stages it
	// now, and the declaration would hide that platform dropping it.
	const gaps = parityGaps(
		[shared('yt-dlp')],
		[{ name: 'yt-dlp', dep: 'yt-dlp', onlyOn: winOnly }],
	);
	assert.equal(gaps.length, 1);
	assert.match(gaps[0], /yt-dlp .*both/);
});
