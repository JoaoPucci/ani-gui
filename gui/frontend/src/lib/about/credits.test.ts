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
