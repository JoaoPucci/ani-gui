// Pre-build step for `package:win`: download the binaries the app
// spawns that a Windows host will not have, and stage them under
// `build-resources/win/bin/`. electron-builder copies the dir into
// the NSIS payload via `win.extraResources` in `package.json`,
// landing at `<install>/resources/bin/<binary>` at runtime. The Rust
// backend searches that dir ahead of PATH, so a bundled binary beats
// a system install (see `resolve_bundled_bin` in
// `gui/backend/src/app.rs`).
//
// Why this exists: a Windows machine has none of these, and without
// them playback and downloads fail with nothing useful to say —
// every play dies on the provider's interstitial, every download on a
// missing downloader. Bundling removes the dependency on the user's
// environment.
//
// Bundled today:
//   - curl-impersonate.exe — the transport native resolution spawns.
//   - yt-dlp.exe           — the downloader.
// Not bundled: ffmpeg. Too large (~80 MB compressed) to ship in the
// installer, so the NSIS script fetches it at install time — see
// `build-resources/installer.nsh`.
import { createWriteStream, existsSync, mkdirSync, statSync } from 'node:fs';
import { copyFile, mkdir, rm } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { pipeline } from 'node:stream/promises';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';

import { needsExtraction, assertDepShape } from '../lib/dep-staging.cjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const electronDir = path.resolve(__dirname, '..');
const repoRoot = path.resolve(electronDir, '..', '..');

// Each dep declares: pinned version, GitHub-releases URL, archive
// name + SHA-256, binary name (what gets dropped in build-resources),
// and the path to the binary inside the archive (so we can extract
// just the file, not the whole archive). The SHA-256 is captured on
// first download by re-running this script after a version bump:
// failure prints actual vs expected so you can paste the new hash.
// fzf.exe and aria2c.exe used to be staged here, mirroring Linux.
// They were the shell script's dependencies — its interactive picker
// and its downloader — and both platforms dropped them when the app
// stopped running the script.
export const DEPS = [
	{
		name: 'yt-dlp',
		dep: 'yt-dlp',
		version: '2025.09.26',
		// Upstream ships a self-contained .exe, not an archive, so
		// there is nothing to unzip — see `directBinary` below.
		archiveName: 'yt-dlp.exe',
		url: 'https://github.com/yt-dlp/yt-dlp/releases/download/2025.09.26/yt-dlp.exe',
		sha256: 'f930cb6bef322cb692fb9e778ee52952619e75f07f2b063d554ff2100cebf7d9',
		binary: 'yt-dlp.exe',
		directBinary: true,
	},
	// curl-impersonate: the transport native anidb resolution spawns.
	// The provider's TLS-fingerprinting front 403s plain curl, so
	// without this every play, availability probe and download dies
	// with a network error — the exact footgun bundling exists to
	// remove, and the reason a Windows build could install and browse
	// while resolving nothing.
	//
	// Only the patched binary is staged, no wrappers. Upstream's
	// per-browser entries are `.bat` files here, and the resolver's
	// suffix table is deliberately narrower than PATHEXT so it never
	// names something the spawn cannot treat as curl. The binary takes
	// its fingerprint from `--impersonate` instead, which the failover
	// list pairs with it (fetch.rs CURL_FAILOVER).
	//
	// Source: github.com/lexiforest/curl-impersonate (the maintained
	// fork; ani-cli 5.0's own instructions point users at it).
	{
		name: 'curl-impersonate',
		dep: 'curl-impersonate',
		version: '2.0.0',
		archiveName: 'curl-impersonate-v2.0.0.x86_64-win32.tar.gz',
		url: 'https://github.com/lexiforest/curl-impersonate/releases/download/v2.0.0/curl-impersonate-v2.0.0.x86_64-win32.tar.gz',
		sha256: 'd2e5905f8adf76f042afe78d1758a978253afddf4eb7bdcb8ddfb38c2f0e530c',
		binary: 'curl-impersonate.exe',
		// Entries carry a `./` prefix in this tarball, and tar matches
		// the name as written — dropping the prefix fails the extract
		// with "not found in archive".
		archivePath: './curl-impersonate.exe',
	},
];

const cacheDir = path.join(electronDir, '.win-deps-cache');
const stagedBinDir = path.join(electronDir, 'build-resources', 'win', 'bin');

// Dev-mode parity: the Rust backend's `AppState::build` looks for
// bundled deps under `<resource_dir>/bin`, where `resource_dir` is
// the directory holding the backend exe. In dev that's
// `gui/backend/target/<profile>/`, so dropping the deps there makes
// the dev loop work without polluting global PATH or the system
// winget store. Both profiles get a copy so the user can switch
// between `cargo build` and `cargo build --release`.
const devTargetBinDirs = [
	path.join(repoRoot, 'gui', 'backend', 'target', 'debug', 'bin'),
	path.join(repoRoot, 'gui', 'backend', 'target', 'release', 'bin'),
];

async function sha256(filePath) {
	const buf = await readFile(filePath);
	return createHash('sha256').update(buf).digest('hex');
}

/**
 * Download `dep` into the local cache, verify the SHA-256, and return
 * the cached zip path. Cache hits are reused; mismatches trigger a
 * redownload. After a version bump, the placeholder SHA fails on
 * purpose and prints the real hash to copy back into this file.
 */
async function downloadOnce(dep) {
	const cachedZip = path.join(cacheDir, dep.archiveName);
	if (existsSync(cachedZip) && statSync(cachedZip).size > 0) {
		const got = await sha256(cachedZip);
		if (got === dep.sha256) {
			console.log(`[fetch-win-deps] cache hit: ${cachedZip}`);
			return cachedZip;
		}
		console.warn(`[fetch-win-deps] cached ${dep.name} checksum mismatch — redownloading`);
		await rm(cachedZip);
	}
	mkdirSync(cacheDir, { recursive: true });
	console.log(`[fetch-win-deps] downloading: ${dep.url}`);
	const resp = await fetch(dep.url, { redirect: 'follow' });
	if (!resp.ok) throw new Error(`download failed: HTTP ${resp.status} for ${dep.url}`);
	await pipeline(resp.body, createWriteStream(cachedZip));
	const got = await sha256(cachedZip);
	if (got !== dep.sha256) {
		// Don't delete the cached file on first-run capture — keep it
		// so a re-run with the corrected hash hits the cache.
		throw new Error(
			`SHA-256 mismatch for ${dep.archiveName}\n` +
				`  expected: ${dep.sha256}\n` +
				`  got:      ${got}\n` +
				`If upstream rotated the asset (or this is a first-run version bump),` +
				` recompute and update DEPS[${dep.name}].sha256.`
		);
	}
	console.log(`[fetch-win-deps] verified ${dep.archiveName}`);
	return cachedZip;
}

/**
 * Use the system `tar` to extract a single named entry from a zip
 * into `destDir`. The entry path (`archivePath`) is preserved
 * relative to `destDir`, so deeply-nested binaries land in nested
 * subdirs that the caller flattens via `flattenInto`.
 *
 * Windows 10+ ships bsdtar (zip support included). Linux build hosts
 * need bsdtar via libarchive-tools — `tar` (GNU) on its own can't
 * read zips.
 */
function extractZipEntry(zipPath, archivePath, destDir) {
	return new Promise((resolve, reject) => {
		const proc = spawn('tar', ['-xf', zipPath, '-C', destDir, archivePath], {
			stdio: 'inherit',
			windowsHide: true,
		});
		proc.on('error', reject);
		proc.on('exit', (code) => {
			if (code === 0) resolve();
			else
				reject(
					new Error(
						`tar -xf '${archivePath}' from '${zipPath}' exited ${code}. ` +
							`On Linux build hosts, install bsdtar (apt: libarchive-tools).`
					)
				);
		});
	});
}

/**
 * Stage one dep: download the archive, extract just the binary, and
 * flatten it into the bin dir. Then mirror into the dev-mode cargo
 * target dirs that exist on disk.
 */
async function stageDep(dep) {
	const zip = await downloadOnce(dep);

	await mkdir(stagedBinDir, { recursive: true });
	const stagedBinary = path.join(stagedBinDir, dep.binary);
	if (existsSync(stagedBinary)) await rm(stagedBinary);

	if (!needsExtraction(dep)) {
		// Nothing to unpack: upstream publishes the executable itself
		// as the release asset, so the verified download IS the binary.
		console.log(`[fetch-win-deps] staging ${dep.binary} directly (no archive)`);
		await copyFile(zip, stagedBinary);
	} else {
		// Extract the archive entry into a per-dep scratch dir so an
		// archive's nested layout doesn't collide with another dep's.
		// Then copy the binary into the flat staged bin dir.
		const scratchDir = path.join(cacheDir, `extract-${dep.name}`);
		await mkdir(scratchDir, { recursive: true });
		console.log(`[fetch-win-deps] extracting ${dep.binary} from ${dep.archiveName}`);
		await extractZipEntry(zip, dep.archivePath, scratchDir);

		const extractedBinary = path.join(scratchDir, dep.archivePath);
		if (!existsSync(extractedBinary)) {
			throw new Error(`expected ${extractedBinary} after extracting ${dep.archiveName}`);
		}
		await copyFile(extractedBinary, stagedBinary);
	}
	console.log(`[fetch-win-deps] staged → ${stagedBinary}`);

	for (const devDir of devTargetBinDirs) {
		// Only populate if the cargo target exists — don't create it
		// from scratch (cargo would later wipe it). Skipping when the
		// target dir is absent keeps the script no-op on machines that
		// haven't run `cargo build` yet.
		const profileDir = path.dirname(devDir);
		if (!existsSync(profileDir)) continue;
		await mkdir(devDir, { recursive: true });
		const devBinary = path.join(devDir, dep.binary);
		if (existsSync(devBinary)) await rm(devBinary);
		await copyFile(stagedBinary, devBinary);
		console.log(`[fetch-win-deps] dev copy → ${devBinary}`);
	}
}

async function main() {
	// Validate every entry before touching the network: a typo should
	// cost a syntax error, not a partial download.
	for (const dep of DEPS) assertDepShape(dep);

	for (const dep of DEPS) {
		console.log(`[fetch-win-deps] === ${dep.name} ${dep.version} ===`);
		await stageDep(dep);
	}
	console.log(
		`[fetch-win-deps] done — ${DEPS.map((d) => `${d.name} ${d.version}`).join(', ')} staged for Windows packaging`
	);
}

// Only fetch when run as a program — see the note in
// `fetch-linux-deps.mjs`. The arch check imports this for `DEPS`.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	main().catch((err) => {
		console.error('[fetch-win-deps] failed:', err.message);
		process.exit(1);
	});
}
