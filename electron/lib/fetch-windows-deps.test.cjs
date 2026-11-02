// What the Windows package carries, and what its installer downloads
// instead.
//
// fetch-windows-deps.mjs declares both as data: `DEPS`, which it
// downloads, verifies and stages into the NSIS payload, and
// `INSTALL_TIME_DEPS`, which it leaves alone because the installer
// script fetches them while it installs. ffmpeg belongs to the second
// list. Carrying gyan.dev's GPL build would oblige an offer of its
// corresponding source, which cannot be made — the build does not
// publish the versions of the libraries it links — and the zip is
// ~90 MB besides. tests/arch/windows_deps.sh holds installer.nsh to
// the pin declared here.
//
// The module guards its entry point, so importing it downloads nothing.

const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const FETCHER = pathToFileURL(path.join(__dirname, '..', 'scripts', 'fetch-windows-deps.mjs')).href;

test('the package does not carry ffmpeg', async () => {
	const { DEPS } = await import(FETCHER);
	const carried = DEPS.filter((d) => d.dep === 'ffmpeg' || /ffmpeg/i.test(d.binary));
	assert.deepEqual(carried, []);
});

test('the package still carries the transport and the downloader', async () => {
	// Taking ffmpeg out must not take its neighbours with it.
	const { DEPS } = await import(FETCHER);
	assert.deepEqual(DEPS.map((d) => d.dep).sort(), ['curl-impersonate', 'yt-dlp']);
});

test('the installer downloads ffmpeg, once, from the essentials build the pin names', async () => {
	const { INSTALL_TIME_DEPS } = await import(FETCHER);
	const ffmpeg = INSTALL_TIME_DEPS.filter((d) => d.dep === 'ffmpeg');
	assert.equal(ffmpeg.length, 1);
	const [d] = ffmpeg;
	const v = d.version;
	assert.match(v, /^\d+\.\d+(\.\d+)?$/);
	// URL and archive path are spelled out rather than derived, so the
	// installer can carry them as literals; derived here, they cannot
	// name a different build than the version.
	assert.equal(
		d.url,
		`https://github.com/GyanD/codexffmpeg/releases/download/${v}/ffmpeg-${v}-essentials_build.zip`
	);
	assert.equal(d.archivePath, `ffmpeg-${v}-essentials_build/bin/ffmpeg.exe`);
	assert.equal(d.binary, 'ffmpeg.exe');
});

test('nothing is both carried and downloaded at install time', async () => {
	const { DEPS, INSTALL_TIME_DEPS } = await import(FETCHER);
	const carried = new Set(DEPS.map((d) => d.dep));
	assert.deepEqual(
		INSTALL_TIME_DEPS.filter((d) => carried.has(d.dep)).map((d) => d.dep),
		[]
	);
});
