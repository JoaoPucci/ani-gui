// Pre-build step for `package:release`: download the binaries the app
// spawns that aren't safe to assume on every Linux host, and stage
// them under `build-resources/linux/bin/`. electron-builder copies the
// dir into both the .deb and AppImage payloads via
// `linux.extraResources` in `package.json`, landing at
// `<install>/resources/bin/<binary>` at runtime. The Rust backend
// searches that dir ahead of PATH, so a bundled binary beats a system
// install (see `resolve_bundled_bin` in `backend/src/app.rs`).
//
// Why this exists: without them a Linux desktop fails at the two
// points that matter and says little about why — every play dies on
// the provider's interstitial, or every download dies on a missing
// downloader.
//
// Bundled today:
//   - curl-impersonate — the transport native resolution spawns. See
//                the block above its entries below.
//   - yt-dlp   — the downloader. It and ffmpeg are not equivalent:
//                  yt-dlp  --fragment-retries infinite -N 16
//                  ffmpeg  -c copy
//                Sixteen parallel chunks with infinite per-chunk
//                retries against one at a time with none. Bundling
//                gives every user the faster, more failure-tolerant
//                downloader rather than only those who happened to
//                install it. ~37 MB, one self-contained file.
//
// NOT bundled, by design:
//   - ffmpeg   — too large (~80 MB compressed). Declared as a
//                `Recommends:` on the .deb so apt pulls the distro
//                build; AppImage users fall back to system PATH or
//                see the typed FfmpegMissing modal. Mirrors the
//                Windows installer which fetches ffmpeg at install
//                time rather than embedding it in the .exe.
//
// This script is the Linux analog of `fetch-windows-deps.mjs`. Keep
// the two in lockstep when adding a new bundled dep — there are no
// exceptions, and `tests/arch/{linux,windows}_deps.sh` hold each side
// to it.
//
// The two stage curl-impersonate differently, which is a difference in
// what upstream ships rather than in what each platform needs. Here the
// per-browser wrappers are shell scripts and carry the fingerprint in
// their own flags, so the staged set is the binary plus wrappers.
// Windows gets those wrappers as `.bat` files, which the resolver
// refuses to name (see `fetch.rs` EXE_SUFFIXES), so it stages the bare
// binary alone and the failover list pairs it with an `--impersonate`
// target instead.

import { createWriteStream, existsSync, mkdirSync, statSync } from 'node:fs';
import { copyFile, mkdir, readdir, rm } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { pipeline } from 'node:stream/promises';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';

import { needsExtraction, assertDepShape, retiredStagedBinaries } from '../lib/dep-staging.cjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const electronDir = path.resolve(__dirname, '..');
const repoRoot = path.resolve(electronDir, '..');

// Each dep declares: pinned version, download URL, archive name +
// SHA-256, binary name (what gets dropped in build-resources), and
// the path to the binary inside the archive (so we can extract just
// the file). SHA-256 is captured on first download by re-running the
// script after a version bump — the failure prints actual vs
// expected so you can paste the new hash.
//
// fzf and aria2c used to be staged here. They were the shell
// script's dependencies — its interactive picker and its downloader —
// and the app dropped them when it stopped running the script: it has
// no picker, and the native downloader asks yt-dlp for concurrency
// (`-N 16`) rather than handing off to aria2c.
export const DEPS = [
	{
		name: 'yt-dlp',
		dep: 'yt-dlp',
		version: '2025.09.26',
		// Upstream ships a self-contained executable, not an archive,
		// so there is nothing to extract — see `directBinary` below.
		archiveName: 'yt-dlp_linux',
		url: 'https://github.com/yt-dlp/yt-dlp/releases/download/2025.09.26/yt-dlp_linux',
		sha256: 'd2f07382138f4bd882254996502636f5a67a8c5ee5ab8a25807e2784a4878642',
		binary: 'yt-dlp',
		directBinary: true,
	},
	// curl-impersonate: the transport the native anidb resolution
	// spawns. The provider's TLS-fingerprinting protection 403s plain
	// curl, so without this every play / availability / download
	// attempt dies with a network error on machines that never
	// hand-installed it. One tarball carries the patched curl binary
	// plus per-browser wrapper scripts; the backend walks its
	// failover names (fetch.rs CURL_FAILOVER) through the bundled bin
	// dir before PATH, so the staged set is the binary plus the three
	// failover wrappers this release provides. Entries share the
	// archive — the download is cached by name, so it fetches once.
	//
	// Source: github.com/lexiforest/curl-impersonate (the maintained
	// fork; ani-cli 5.0's own instructions point users at it).
	{
		name: 'curl-impersonate',
		dep: 'curl-impersonate',
		version: '2.0.0',
		archiveName: 'curl-impersonate-v2.0.0.x86_64-linux-gnu.tar.gz',
		url: 'https://github.com/lexiforest/curl-impersonate/releases/download/v2.0.0/curl-impersonate-v2.0.0.x86_64-linux-gnu.tar.gz',
		sha256: '0e2bd63823ea588c0fb829c0aef2e9f9af191039e9e11f7541773c86445bfe77',
		binary: 'curl-impersonate',
		// The tarball is flat — every file sits at the archive root.
		archivePath: 'curl-impersonate',
		tarFlag: '-xzf',
	},
	{
		name: 'curl-impersonate-firefox135',
		dep: 'curl-impersonate',
		version: '2.0.0',
		archiveName: 'curl-impersonate-v2.0.0.x86_64-linux-gnu.tar.gz',
		url: 'https://github.com/lexiforest/curl-impersonate/releases/download/v2.0.0/curl-impersonate-v2.0.0.x86_64-linux-gnu.tar.gz',
		sha256: '0e2bd63823ea588c0fb829c0aef2e9f9af191039e9e11f7541773c86445bfe77',
		binary: 'curl_firefox135',
		archivePath: 'curl_firefox135',
		tarFlag: '-xzf',
	},
	{
		name: 'curl-impersonate-chrome136',
		dep: 'curl-impersonate',
		version: '2.0.0',
		archiveName: 'curl-impersonate-v2.0.0.x86_64-linux-gnu.tar.gz',
		url: 'https://github.com/lexiforest/curl-impersonate/releases/download/v2.0.0/curl-impersonate-v2.0.0.x86_64-linux-gnu.tar.gz',
		sha256: '0e2bd63823ea588c0fb829c0aef2e9f9af191039e9e11f7541773c86445bfe77',
		binary: 'curl_chrome136',
		archivePath: 'curl_chrome136',
		tarFlag: '-xzf',
	},
	{
		name: 'curl-impersonate-chrome116',
		dep: 'curl-impersonate',
		version: '2.0.0',
		archiveName: 'curl-impersonate-v2.0.0.x86_64-linux-gnu.tar.gz',
		url: 'https://github.com/lexiforest/curl-impersonate/releases/download/v2.0.0/curl-impersonate-v2.0.0.x86_64-linux-gnu.tar.gz',
		sha256: '0e2bd63823ea588c0fb829c0aef2e9f9af191039e9e11f7541773c86445bfe77',
		binary: 'curl_chrome116',
		archivePath: 'curl_chrome116',
		tarFlag: '-xzf',
	},
];

const cacheDir = path.join(electronDir, '.linux-deps-cache');
const stagedBinDir = path.join(electronDir, 'build-resources', 'linux', 'bin');

// Dev-mode parity: the Rust backend's `AppState::build` looks for
// bundled deps under `<resource_dir>/bin`, where `resource_dir` is
// the directory holding the backend exe. In dev that's
// `backend/target/<profile>/`, so dropping the deps there makes
// the dev loop work without polluting global PATH.
const devTargetBinDirs = [
	path.join(repoRoot, 'backend', 'target', 'debug', 'bin'),
	path.join(repoRoot, 'backend', 'target', 'release', 'bin'),
];

async function sha256(filePath) {
	const buf = await readFile(filePath);
	return createHash('sha256').update(buf).digest('hex');
}

/**
 * Download `dep` into the local cache, verify the SHA-256, and return
 * the cached archive path. Cache hits are reused; mismatches trigger
 * a redownload.
 */
async function downloadOnce(dep) {
	const cached = path.join(cacheDir, dep.archiveName);
	if (existsSync(cached) && statSync(cached).size > 0) {
		const got = await sha256(cached);
		if (got === dep.sha256) {
			console.log(`[fetch-linux-deps] cache hit: ${cached}`);
			return cached;
		}
		console.warn(`[fetch-linux-deps] cached ${dep.name} checksum mismatch — redownloading`);
		await rm(cached);
	}
	mkdirSync(cacheDir, { recursive: true });
	console.log(`[fetch-linux-deps] downloading: ${dep.url}`);
	const resp = await fetch(dep.url, { redirect: 'follow' });
	if (!resp.ok) throw new Error(`download failed: HTTP ${resp.status} for ${dep.url}`);
	await pipeline(resp.body, createWriteStream(cached));
	const got = await sha256(cached);
	if (got !== dep.sha256) {
		throw new Error(
			`SHA-256 mismatch for ${dep.archiveName}\n` +
				`  expected: ${dep.sha256}\n` +
				`  got:      ${got}\n` +
				`If upstream rotated the asset (or this is a first-run version bump),` +
				` recompute and update DEPS[${dep.name}].sha256.`,
		);
	}
	console.log(`[fetch-linux-deps] verified ${dep.archiveName}`);
	return cached;
}

/**
 * Extract a single named entry from a tar archive into `destDir`.
 * GNU tar handles both gzip (`-xzf`) and bzip2 (`-xjf`) natively on
 * any Linux build host.
 */
function extractEntry(archive, archivePath, destDir, tarFlag) {
	return new Promise((resolve, reject) => {
		const proc = spawn('tar', [tarFlag, archive, '-C', destDir, archivePath], {
			stdio: 'inherit',
		});
		proc.on('error', reject);
		proc.on('exit', (code) => {
			if (code === 0) resolve();
			else reject(new Error(`tar ${tarFlag} '${archivePath}' from '${archive}' exited ${code}`));
		});
	});
}

/**
 * Stage one dep: download the archive, extract just the binary,
 * `chmod +x` it (tar preserves the source mode but the binary needs
 * to stay executable across hosts), and flatten into the bin dir.
 * Then mirror into the dev-mode cargo target dirs that exist on disk.
 */
async function stageDep(dep) {
	const archive = await downloadOnce(dep);

	await mkdir(stagedBinDir, { recursive: true });
	const stagedBinary = path.join(stagedBinDir, dep.binary);
	if (existsSync(stagedBinary)) await rm(stagedBinary);

	if (!needsExtraction(dep)) {
		// Nothing to unpack: upstream publishes the executable itself
		// as the release asset, so the verified download IS the binary.
		console.log(`[fetch-linux-deps] staging ${dep.binary} directly (no archive)`);
		await copyFile(archive, stagedBinary);
	} else {
		const scratchDir = path.join(cacheDir, `extract-${dep.name}`);
		await mkdir(scratchDir, { recursive: true });
		console.log(`[fetch-linux-deps] extracting ${dep.binary} from ${dep.archiveName}`);
		await extractEntry(archive, dep.archivePath, scratchDir, dep.tarFlag);

		const extractedBinary = path.join(scratchDir, dep.archivePath);
		if (!existsSync(extractedBinary)) {
			throw new Error(`expected ${extractedBinary} after extracting ${dep.archiveName}`);
		}
		await copyFile(extractedBinary, stagedBinary);
	}
	// Force executable bit; tar preserves source perms but we don't
	// want to depend on that across hosts. 0o755 matches Linux
	// convention for binaries.
	// eslint-disable-next-line no-bitwise
	const { chmod } = await import('node:fs/promises');
	await chmod(stagedBinary, 0o755);
	console.log(`[fetch-linux-deps] staged → ${stagedBinary}`);

	for (const devDir of devTargetBinDirs) {
		const profileDir = path.dirname(devDir);
		if (!existsSync(profileDir)) continue;
		await mkdir(devDir, { recursive: true });
		const devBinary = path.join(devDir, dep.binary);
		if (existsSync(devBinary)) await rm(devBinary);
		await copyFile(stagedBinary, devBinary);
		await chmod(devBinary, 0o755);
		console.log(`[fetch-linux-deps] dev copy → ${devBinary}`);
	}
}

/**
 * Delete binaries a previous run staged that `DEPS` no longer names.
 *
 * Staging is per-entry, and `extraResources` copies the directory
 * whole — so without this a retired dependency keeps shipping from any
 * checkout that staged it before, with nothing in the build output to
 * say so. Runs over the dev mirrors for the same reason: a `cargo run`
 * finds them ahead of PATH.
 */
async function sweepRetired() {
	for (const dir of [stagedBinDir, ...devTargetBinDirs]) {
		if (!existsSync(dir)) continue;
		const stale = retiredStagedBinaries(await readdir(dir), DEPS);
		for (const name of stale) {
			await rm(path.join(dir, name), { recursive: true, force: true });
			console.log(`[fetch-linux-deps] removed retired ${path.join(dir, name)}`);
		}
	}
}

async function main() {
	// Validate every entry before touching the network: a typo should
	// cost a syntax error, not a partial download.
	for (const dep of DEPS) assertDepShape(dep);

	// Before staging: drop anything an earlier run left that this
	// list no longer declares.
	await sweepRetired();

	for (const dep of DEPS) {
		console.log(`[fetch-linux-deps] === ${dep.name} ${dep.version} ===`);
		await stageDep(dep);
	}
	console.log(
		`[fetch-linux-deps] done — ${DEPS.map((d) => `${d.name} ${d.version}`).join(', ')} staged for Linux packaging`,
	);
}

// Only fetch when run as a program. `tests/arch/windows_deps.sh` imports
// this module to read `DEPS`, and an import that downloaded several
// hundred megabytes would make the check unrunnable — the inventory has
// to be readable without doing the work it describes.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	main().catch((err) => {
		console.error('[fetch-linux-deps] failed:', err.message);
		process.exit(1);
	});
}
