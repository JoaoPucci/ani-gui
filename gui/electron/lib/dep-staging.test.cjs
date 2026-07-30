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

const { needsExtraction, assertDepShape } = require('./dep-staging.cjs');

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
