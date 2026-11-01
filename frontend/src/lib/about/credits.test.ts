/**
 * Sanity guards for the hand-curated credits list. The module is
 * data-only — these tests catch the dumb errors a future "just
 * added a dep" edit might trip:
 *
 *   - missing or empty required fields
 *   - duplicate display names within a category
 *   - URLs that aren't https (the page renders these as outbound
 *     links opened via shell.openExternal; non-https is a smell)
 *   - a malformed donation address (caught here in addition to
 *     eth.ts's dedicated guard)
 *
 * Also pulls the credits module into the coverage graph, which
 * keeps the frontend lines/statements ratchet honest when a data
 * file grows.
 */
import { afterEach, describe, it, expect } from 'vitest';
import { m } from '$lib/paraglide/messages';
import { getLocale, overwriteGetLocale } from '$lib/paraglide/runtime';
import {
	ASSETS,
	assetName,
	BUNDLED_TOOLS,
	DONATION_ETH_ADDRESS,
	ffmpegSourceOffer,
	type AssetCredit,
	type BundledTool
} from './credits';
import { isValidEthAddress } from './eth';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

/**
 * The `dep` names both platform fetchers stage — their `DEPS` lists.
 *
 * Read as text rather than imported: the fetchers live outside the
 * frontend root, so vitest cannot resolve them as modules. `dep` is a
 * declared literal on every entry — `windows_deps.sh` compares the two
 * platforms through that same field — so finding it is a lookup, not
 * an interpretation of what the script does.
 */
function stagedDeps(): Set<string> {
	const scripts = path.resolve(
		path.dirname(fileURLToPath(import.meta.url)),
		'../../../../electron/scripts'
	);
	const found = new Set<string>();
	for (const file of ['fetch-linux-deps.mjs', 'fetch-windows-deps.mjs']) {
		const src = readFileSync(path.join(scripts, file), 'utf8');
		// Only the `DEPS` array, bounded by its column-zero opening and
		// closing lines: the Linux fetcher's `SYSTEM_DEPS` names ffmpeg
		// too, as a dependency it does *not* stage, and counting that
		// would credit Linux's declaration as a staged binary.
		const start = src.indexOf('\nexport const DEPS = [\n');
		const end = src.indexOf('\n];\n', start);
		if (start < 0 || end < 0) {
			throw new Error(`${file}: no column-zero \`export const DEPS = [\` ... \`];\` to read`);
		}
		for (const m of src.slice(start, end).matchAll(/^\t\tdep: '([^']+)',$/gm)) found.add(m[1]);
	}
	return found;
}

/** A fetcher's `DEPS`, imported: the version and URL of an entry are
 *  fields of a parsed object, not text to pick out of the script. The
 *  fetchers guard their entry point, so importing one downloads
 *  nothing. */
type FetcherDep = { name: string; dep: string; version: string; url: string };
const SCRIPTS = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	'../../../../electron/scripts'
);
async function fetcherDeps(file: string): Promise<FetcherDep[]> {
	const mod = await import(/* @vite-ignore */ path.join(SCRIPTS, file));
	return mod.DEPS as FetcherDep[];
}
const FETCHED = {
	linux: await fetcherDeps('fetch-linux-deps.mjs'),
	windows: await fetcherDeps('fetch-windows-deps.mjs')
};
const README = readFileSync(path.resolve(SCRIPTS, '../../README.md'), 'utf8');

// Read while the file is collected, not inside the test that compares
// it: a test's 5 s budget is wall time, and a cold read queued behind
// another build's disk writes can spend it before anything is compared.
const STAGED = stagedDeps();

function assertCommonShape(entry: { name: string; license: string; url: string }, label: string) {
	expect(entry.name, `${label}: name`).toBeTruthy();
	expect(typeof entry.name, `${label}: name is string`).toBe('string');
	expect(entry.license, `${label}: license`).toBeTruthy();
	expect(entry.url.startsWith('https://'), `${label}: url is https (${entry.url})`).toBe(true);
}

function assertUniqueNames(entries: ReadonlyArray<{ name: string }>, label: string) {
	const seen = new Set<string>();
	for (const entry of entries) {
		expect(seen.has(entry.name), `${label}: duplicate name "${entry.name}"`).toBe(false);
		seen.add(entry.name);
	}
}

describe('credits — bundled tools', () => {
	it('has at least one entry', () => {
		expect(BUNDLED_TOOLS.length).toBeGreaterThan(0);
	});

	it('each entry has the required shape', () => {
		for (const tool of BUNDLED_TOOLS as ReadonlyArray<BundledTool>) {
			assertCommonShape(tool, `BUNDLED_TOOLS[${tool.name}]`);
			expect(tool.noteId, `${tool.name}: noteId`).toBeTruthy();
			if (tool.version !== null) {
				expect(typeof tool.version, `${tool.name}: version is string when set`).toBe('string');
				expect(tool.version.length, `${tool.name}: version not empty`).toBeGreaterThan(0);
			}
		}
	});

	it('names are unique within the list', () => {
		assertUniqueNames(BUNDLED_TOOLS, 'BUNDLED_TOOLS');
	});

	it('credits exactly what the fetchers stage', () => {
		// The page's whole claim is that this list is what the packages
		// bundle, so both directions are wrong in their own way: a
		// missing entry hides a binary the user received, and a stale
		// one tells them the app needs something it never invokes.
		//
		// Checked against the fetchers themselves rather than a second
		// hand-written list — one list compared to another only proves
		// they were typed the same day. fzf and aria2c outlived the
		// script in both places at once precisely because nothing tied
		// them together.
		const credited = new Set(BUNDLED_TOOLS.map((t) => t.name));
		const staged = STAGED;

		expect(staged.size, 'the fetchers should declare something').toBeGreaterThan(0);
		for (const dep of staged) {
			expect(credited.has(dep), `${dep} is staged by a fetcher but not credited`).toBe(true);
		}
		for (const name of credited) {
			expect(staged.has(name), `${name} is credited but no fetcher stages it`).toBe(true);
		}
	});

	it('credits each tool at the version the fetchers pin', () => {
		// A hand-typed version drifts from the binary the package
		// carries the first time someone bumps one and not the other.
		for (const tool of BUNDLED_TOOLS) {
			if (tool.version === null) continue;
			const pinned = [...FETCHED.linux, ...FETCHED.windows].filter((d) => d.dep === tool.name);
			expect(pinned.length, `${tool.name}: no fetcher pins it`).toBeGreaterThan(0);
			for (const d of pinned) {
				expect(d.version, `${tool.name}: ${d.name} pins ${d.version}`).toBe(tool.version);
			}
		}
	});

	it("offers ffmpeg's source for the exact build the Windows installer ships", () => {
		// The installer redistributes a GPL build, which obliges an offer
		// of its corresponding source. The offer is derived from the
		// fetcher's pin, so it cannot name a different build than the
		// one in the package.
		const win = FETCHED.windows.find((d) => d.dep === 'ffmpeg');
		expect(win, 'the Windows fetcher stages ffmpeg').toBeTruthy();
		const v = win!.version;
		expect(win!.url, 'the staged binary comes from that build page').toBe(
			`https://github.com/GyanD/codexffmpeg/releases/download/${v}/ffmpeg-${v}-essentials_build.zip`
		);
		const ffmpeg = BUNDLED_TOOLS.find((t) => t.name === 'ffmpeg');
		expect(ffmpeg?.sourceOffer).toEqual(ffmpegSourceOffer(v));
		expect(ffmpegSourceOffer(v)).toEqual({
			version: v,
			buildUrl: `https://github.com/GyanD/codexffmpeg/releases/tag/${v}`,
			sourceUrl: `https://github.com/FFmpeg/FFmpeg/tree/n${v}`
		});
	});

	it('repeats the source offer in the README, at the same build', () => {
		const offer = BUNDLED_TOOLS.find((t) => t.name === 'ffmpeg')?.sourceOffer;
		expect(offer).toBeTruthy();
		expect(README, 'README links the build page').toContain(offer!.buildUrl);
		expect(README, 'README links the FFmpeg source tag').toContain(offer!.sourceUrl);
	});

	it('noteIds are unique (and thus typesafe against the page-side switch)', () => {
		const seen = new Set<string>();
		for (const tool of BUNDLED_TOOLS) {
			expect(seen.has(tool.noteId), `duplicate noteId "${tool.noteId}"`).toBe(false);
			seen.add(tool.noteId);
		}
	});
});

describe('credits — assets', () => {
	it('has at least one entry', () => {
		expect(ASSETS.length).toBeGreaterThan(0);
	});

	it('each asset has the required shape', () => {
		for (const asset of ASSETS as ReadonlyArray<AssetCredit>) {
			assertCommonShape(asset, `ASSETS[${asset.name}]`);
			expect(asset.author, `${asset.name}: author`).toBeTruthy();
			expect(asset.noteId, `${asset.name}: noteId`).toBeTruthy();
		}
	});

	it('credits the Lottie animation that drives LoadingOverlay', () => {
		// The Lottie attribution is the load-bearing reason this section
		// exists at all — losing it would be a real regression.
		const lottie = ASSETS.find((a) => /lottie/i.test(a.name));
		expect(lottie, 'a Lottie credit must be present').toBeTruthy();
		expect(lottie?.url).toMatch(/lottiefiles\.com/);
	});
});

describe('credits — donation address', () => {
	it('matches the EIP-55 address shape', () => {
		expect(isValidEthAddress(DONATION_ETH_ADDRESS)).toBe(true);
	});
});

describe('assetName', () => {
	// The About page shows this label to the reader, so it comes from
	// the message catalogue in their locale, like the asset's note.
	const originalGetLocale = getLocale;
	afterEach(() => {
		overwriteGetLocale(originalGetLocale);
	});

	it('labels every credited asset from the message catalogue', () => {
		for (const asset of ASSETS) {
			overwriteGetLocale(() => 'pt-BR');
			const portuguese = assetName(asset.noteId);
			overwriteGetLocale(() => 'en');
			expect(portuguese, asset.noteId).not.toBe(assetName(asset.noteId));
		}
		overwriteGetLocale(() => 'pt-BR');
		expect(assetName('lottie_loading')).toBe(
			m.about_asset_name_lottie_loading({}, { locale: 'pt-BR' })
		);
	});
});
