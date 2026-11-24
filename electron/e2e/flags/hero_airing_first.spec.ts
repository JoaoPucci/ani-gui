/**
 * The home hero's airing preference end to end, on the packaged app,
 * in each state.
 *
 * `hero_airing_first` (backend/src/hero) reorders the shows with art
 * the hero rotates through, airing ones first. Home's trending list is
 * stubbed with three finished shows with art and then one airing show
 * with art; the pick itself is the real backend's (`POST
 * /api/home/hero` passes through to it), under the flags the launch
 * decided. Off, the hero opens on the first finished show, as it did
 * before the flag existed. On, it opens on the airing one. The Debug
 * page then shows the walk the pick returned: the flag's value and
 * where it came from, and what the pick did with each show.
 *
 * Off is the default and every unverified file's answer: no file named,
 * and a file turning it on that nobody signed. On is a file signed by
 * the test-run key, which only a test-run build trusts.
 *
 * Reduced motion is emulated so the hero holds its first show while it
 * is read (the rotation does not run under it).
 *
 * Pre-condition: `pnpm run package:e2e` has produced
 * `dist-e2e/linux-unpacked/`.
 */
import type { Page } from '@playwright/test';
import { expect, test } from '../policy-guard';
import fs, { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { appInfo, defaultSettings } from '../fixtures/kitsu';
import { writeFlagsFile, type Signer } from '../flags-file';
import { appWindow, launchPackagedApp, packagedBinary, stopApp } from '../launch';
import { answer } from '../stubs';

const FLAG = 'hero_airing_first';

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package:e2e`);
	}
});

const show = (id: string, title: string, status: string) => ({
	id,
	canonical_title: title,
	titles: { en_jp: title },
	abbreviated_titles: [],
	slug: `hero-airing-${id}`,
	synopsis: `${title}, a show the hero airing spec lists.`,
	start_date: '2020-01-01',
	end_date: status === 'finished' ? '2020-03-01' : null,
	episode_count: 12,
	average_rating: 80,
	subtype: 'TV',
	status,
	age_rating: null,
	popularity_rank: null,
	poster_image: null,
	cover_image: {
		tiny: null,
		small: null,
		large: `https://media.kitsu.app/anime/cover_images/${id}/large.jpg`,
		original: null
	}
});

const trending = [
	show('9101', 'First Finished', 'finished'),
	show('9102', 'Second Finished', 'finished'),
	show('9103', 'Third Finished', 'finished'),
	show('9104', 'Airing Now', 'current')
];

/** A scratch profile for one launch: its HOME and XDG directories. */
function scratchProfile(): { dir: string; env: Record<string, string> } {
	const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ani-gui-e2e-hero-airing-'));
	const env: Record<string, string> = { HOME: dir, ANI_GUI_UNATTENDED: '1' };
	for (const [name, sub] of [
		['XDG_CONFIG_HOME', 'config'],
		['XDG_CACHE_HOME', 'cache'],
		['XDG_STATE_HOME', 'state'],
		['XDG_DATA_HOME', 'data']
	] as const) {
		env[name] = path.join(dir, sub);
	}
	return { dir, env };
}

/** Stub every API read Home makes except the pick, which the real
 *  backend answers, and the walk the Debug page reads from it. */
async function stubHome(page: Page): Promise<void> {
	await page.route('**/api/**', async (r) => {
		const p = new URL(r.request().url()).pathname;
		const j = (body: unknown) =>
			answer(r, { status: 200, contentType: 'application/json', body: JSON.stringify(body) });
		if (p === '/api/home/hero' || p === '/api/home/hero/walk') return r.continue();
		if (p === '/api/app-info') return j(appInfo);
		if (p === '/api/settings') return j(defaultSettings);
		if (p === '/api/kitsu/trending-anilist') return j(trending);
		if (p === '/api/kitsu/top-rated') return j([]);
		if (p === '/api/availability/batch') return j({ cached: {} });
		if (p.startsWith('/api/image')) return answer(r, { status: 503 });
		if (p === '/api/history') return j([]);
		if (p === '/api/watched-at') return j({});
		return j(null);
	});
}

type Seen = {
	flags: unknown;
	hero: string;
	walkFlag: string | null;
	walkOrigin: string | null;
	steps: { id: string | null; verdict: string | null; slot: string | null }[];
};

/** Launch with the flags file `file` returns (or none), read the hero's
 *  first show on Home, then the walk on the Debug page. */
async function launchWith(file: (dir: string) => string | null): Promise<Seen> {
	const { dir, env } = scratchProfile();
	const named = file(dir);
	const app = await launchPackagedApp(
		{ ...(process.env as Record<string, string>), ...env },
		{ profile: dir, ...(named === null ? {} : { flagsFile: named }) }
	);
	try {
		const page = await appWindow(app);
		await stubHome(page);
		await page.emulateMedia({ reducedMotion: 'reduce' });
		// A fresh mount, so every request Home makes meets the stubs
		// (see home-continue.spec.ts).
		const home = page.url();
		await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});
		await page.goto('about:blank');
		await page.goto(home, { waitUntil: 'domcontentloaded' });
		const title = page.locator('.hero-title');
		await expect(title).toBeVisible({ timeout: 30_000 });
		const hero = (await title.textContent())?.trim() ?? '';
		const flags = await page.evaluate(() => window.aniGui?.flags);
		await page.locator('a[href$="/diagnostics"]').first().click();
		const walk = page.getByTestId('hero-walk');
		await expect(walk.getByTestId('hero-walk-step').first()).toBeVisible({ timeout: 15_000 });
		const flag = walk.getByTestId('hero-walk-flag');
		const steps = await walk
			.getByTestId('hero-walk-step')
			.evaluateAll((rows) =>
				rows.map((row) => ({
					id: row.getAttribute('data-id'),
					verdict: row.getAttribute('data-verdict'),
					slot: row.getAttribute('data-slot')
				}))
			);
		return {
			flags,
			hero,
			walkFlag: await flag.getAttribute('data-on'),
			walkOrigin: await flag.getAttribute('data-origin'),
			steps
		};
	} finally {
		await stopApp(app);
	}
}

const turnOn = (signer: Signer) => (dir: string) => writeFlagsFile(dir, { [FLAG]: true }, signer);

/** The walk with the preference off: the three finished shows taken
 *  in trending order, and the hero settled before the airing one. */
const OFF_STEPS = [
	{ id: '9101', verdict: 'taken', slot: '0' },
	{ id: '9102', verdict: 'taken', slot: '1' },
	{ id: '9103', verdict: 'taken', slot: '2' }
];

test('hero_airing_first off: no flags file named, the default', async () => {
	const seen = await launchWith(() => null);
	expect(seen).toMatchObject({
		flags: { [FLAG]: false },
		hero: 'First Finished',
		walkFlag: 'false',
		walkOrigin: 'test',
		steps: OFF_STEPS
	});
});

test('hero_airing_first off: a file turns it on, but nobody signed it', async () => {
	const seen = await launchWith(turnOn('unsigned'));
	expect(seen).toMatchObject({
		flags: { [FLAG]: false },
		hero: 'First Finished',
		walkFlag: 'false',
		walkOrigin: 'test',
		steps: OFF_STEPS
	});
});

test('hero_airing_first on: a file turns it on, signed by the test-run key', async () => {
	const seen = await launchWith(turnOn('test-run key'));
	expect(seen).toMatchObject({
		flags: { [FLAG]: true },
		hero: 'Airing Now',
		walkFlag: 'true',
		walkOrigin: 'test',
		// The airing show leads; the third finished show, which the pick
		// off would have taken, is passed over for it.
		steps: [
			{ id: '9101', verdict: 'taken', slot: '1' },
			{ id: '9102', verdict: 'taken', slot: '2' },
			{ id: '9103', verdict: 'skipped', slot: null },
			{ id: '9104', verdict: 'taken', slot: '0' }
		]
	});
});
