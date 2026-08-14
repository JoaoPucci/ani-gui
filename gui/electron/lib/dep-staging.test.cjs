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

const { needsExtraction, assertDepShape, retiredStagedBinaries } = require('./dep-staging.cjs');

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
