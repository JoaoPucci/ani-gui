/**
 * Sanity guards for the hand-curated credits list. The module itself
 * is data-only — these tests don't exercise logic, they catch the
 * dumb errors a future "just added a dep" edit might trip:
 *
 *   - missing or empty required fields
 *   - duplicate display names within a category (a clue someone
 *     pasted the same entry twice)
 *   - URLs that aren't https (the page renders these as outbound
 *     links opened via shell.openExternal; non-https is a smell)
 *   - a malformed donation address (caught here in addition to the
 *     dedicated guard in eth.ts so a future edit to the constant
 *     fails CI rather than silently shipping a broken value)
 *
 * Also pulls the credits module into the coverage graph, which keeps
 * the frontend lines/statements ratchet honest when a data file
 * grows.
 */
import { describe, it, expect } from 'vitest';
import {
	ASSETS,
	BACKEND_DEPS,
	BUNDLED_TOOLS,
	DONATION_ETH_ADDRESS,
	FRONTEND_DEPS,
	type CreditEntry
} from './credits';
import { isValidEthAddress } from './eth';

function assertCreditEntryShape(entry: CreditEntry, label: string) {
	expect(entry.name, `${label}: name`).toBeTruthy();
	expect(typeof entry.name, `${label}: name is string`).toBe('string');
	expect(entry.license, `${label}: license`).toBeTruthy();
	expect(entry.note, `${label}: note`).toBeTruthy();
	expect(entry.url.startsWith('https://'), `${label}: url is https (${entry.url})`).toBe(true);
	// version may be null (the bundled ani-cli + ffmpeg cases) — but
	// when it's a string it must not be empty.
	if (entry.version !== null) {
		expect(typeof entry.version, `${label}: version is string when set`).toBe('string');
		expect(entry.version.length, `${label}: version not empty`).toBeGreaterThan(0);
	}
}

function assertUniqueNames(entries: ReadonlyArray<{ name: string }>, label: string) {
	const seen = new Set<string>();
	for (const entry of entries) {
		expect(seen.has(entry.name), `${label}: duplicate name "${entry.name}"`).toBe(false);
		seen.add(entry.name);
	}
}

describe('credits — frontend deps', () => {
	it('has at least one entry (a regression catches an accidental wipe)', () => {
		expect(FRONTEND_DEPS.length).toBeGreaterThan(0);
	});

	it('each entry has the required shape', () => {
		for (const dep of FRONTEND_DEPS) {
			assertCreditEntryShape(dep, `FRONTEND_DEPS[${dep.name}]`);
		}
	});

	it('names are unique within the list', () => {
		assertUniqueNames(FRONTEND_DEPS, 'FRONTEND_DEPS');
	});
});

describe('credits — backend deps', () => {
	it('has at least one entry', () => {
		expect(BACKEND_DEPS.length).toBeGreaterThan(0);
	});

	it('each entry has the required shape', () => {
		for (const dep of BACKEND_DEPS) {
			assertCreditEntryShape(dep, `BACKEND_DEPS[${dep.name}]`);
		}
	});

	it('names are unique within the list', () => {
		assertUniqueNames(BACKEND_DEPS, 'BACKEND_DEPS');
	});
});

describe('credits — bundled tools', () => {
	it('has at least one entry', () => {
		expect(BUNDLED_TOOLS.length).toBeGreaterThan(0);
	});

	it('each entry has the required shape', () => {
		for (const tool of BUNDLED_TOOLS) {
			assertCreditEntryShape(tool, `BUNDLED_TOOLS[${tool.name}]`);
		}
	});

	it('names are unique within the list', () => {
		assertUniqueNames(BUNDLED_TOOLS, 'BUNDLED_TOOLS');
	});
});

describe('credits — assets', () => {
	it('has at least one entry', () => {
		expect(ASSETS.length).toBeGreaterThan(0);
	});

	it('each asset has the required shape', () => {
		for (const asset of ASSETS) {
			expect(asset.name, `${asset.name}: name`).toBeTruthy();
			expect(asset.author, `${asset.name}: author`).toBeTruthy();
			expect(asset.license, `${asset.name}: license`).toBeTruthy();
			expect(asset.note, `${asset.name}: note`).toBeTruthy();
			expect(asset.url.startsWith('https://'), `${asset.name}: url is https (${asset.url})`).toBe(
				true
			);
		}
	});

	it('credits the Lottie animation that drives LoadingOverlay', () => {
		// The Lottie attribution is the load-bearing reason this section
		// exists at all — losing it would be a real regression, not just
		// cosmetic.
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
