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
import { describe, it, expect } from 'vitest';
import {
	ASSETS,
	BUNDLED_TOOLS,
	DONATION_ETH_ADDRESS,
	type AssetCredit,
	type BundledTool
} from './credits';
import { isValidEthAddress } from './eth';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

/** Tools the packages provide without a fetcher staging them. */
const NOT_FETCHED = new Set(['ffmpeg']);

/**
 * The `dep` names both platform fetchers declare.
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
		for (const m of src.matchAll(/^\t\tdep: '([^']+)',$/gm)) found.add(m[1]);
	}
	return found;
}

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
		const staged = stagedDeps();

		expect(staged.size, 'the fetchers should declare something').toBeGreaterThan(0);
		for (const dep of staged) {
			expect(credited.has(dep), `${dep} is staged by a fetcher but not credited`).toBe(true);
		}
		for (const name of credited) {
			if (NOT_FETCHED.has(name)) continue;
			expect(staged.has(name), `${name} is credited but no fetcher stages it`).toBe(true);
		}
	});

	it('names the entries that are bundled without being fetched', () => {
		// ffmpeg is the one tool the packages provide by another route:
		// an apt `Recommends:` on the .deb and an install-time download
		// in the NSIS script, because ~80 MB is too much to stage. The
		// exemption above is declared here so it stays a decision
		// rather than a hole in the parity check.
		const names = BUNDLED_TOOLS.map((t) => t.name);
		for (const name of NOT_FETCHED) {
			expect(names, `${name} is exempted from the parity check but not credited`).toContain(name);
		}
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
