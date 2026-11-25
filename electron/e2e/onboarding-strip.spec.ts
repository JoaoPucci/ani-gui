/**
 * The watch-order strip's feature tip, on the packaged app with its
 * real backend deciding: the renderer's stubs answer every route but
 * the feature tips' own (`/api/onboarding*`), which reach the backend,
 * so its first-ever detection, its windows and its record are what the
 * page shows.
 *
 * The build is numbered 0.18.1 on master, outside the tip's window
 * (0.19.0 to 0.21.0), so the launch names 0.19.0 as the release the
 * test-run backend counts at (`onboardingVersion`). An update is a
 * profile that already holds a watch history; a first-ever install an
 * empty one.
 *
 * - after an update, the show page dims around the strip and the
 *   balloon takes focus on "Got it", and the app's "/" shortcut stands
 *   down under it;
 * - the window's own buttons are still what a press there reaches;
 * - a press outside closes it, and the press reaches nothing under the
 *   dim: a link under it does not open;
 * - a press on the strip itself does what it does, and closes the tip;
 * - closed, it stays closed across a fresh read of the tips (the
 *   backend recorded it);
 * - Escape closes it too, on another update;
 * - a first-ever install never sees it;
 * - in a contrast theme the dim is still there, and the lit strip has a
 *   Highlight outline; with reduced motion the dim does not fade in.
 */
import type { ElectronApplication, Page } from '@playwright/test';
import { expect, test } from './policy-guard';
import { existsSync } from 'node:fs';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { appInfo, defaultSettings, topRated, trending } from './fixtures/kitsu';
import { appWindow, launchPackagedApp, packagedBinary, stopApp } from './launch';
import { answer } from './stubs';

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package:e2e`);
	}
});

/** The backend's recorded watch-order answers (tests/fixtures/watch-order/scenarios). */
function scenarioAnswer(file: string, name: string): unknown {
	const scenario = JSON.parse(
		fs.readFileSync(
			path.resolve(__dirname, '../../tests/fixtures/watch-order/scenarios', `${file}.json`),
			'utf8'
		)
	) as { cases: { name: string; answer: unknown }[] };
	const found = scenario.cases.find((c) => c.name.includes(name));
	if (!found) throw new Error(`no case "${name}" in ${file}.json`);
	return found.answer;
}

const stripShow = { ...topRated[0], id: '42', canonical_title: 'Saga: Second Arc' };
const stripAnswer = scenarioAnswer(
	'detail-watch-order-strip',
	'shows what comes before and after, and the place in the run'
);

type Profile = 'update' | 'first-ever';

async function launch(profile: Profile): Promise<{ app: ElectronApplication; page: Page }> {
	const tmp = path.join(os.tmpdir(), `ani-gui-onboarding-${process.pid}-${Date.now()}`);
	fs.mkdirSync(tmp, { recursive: true });
	if (profile === 'update') {
		// An earlier release ran here: it left a watch history.
		const state = path.join(tmp, 'state', 'ani-gui');
		fs.mkdirSync(state, { recursive: true });
		fs.writeFileSync(path.join(state, 'history'), '');
	}
	const app = await launchPackagedApp(
		{
			...process.env,
			XDG_STATE_HOME: path.join(tmp, 'state'),
			XDG_CONFIG_HOME: path.join(tmp, 'config'),
			XDG_CACHE_HOME: path.join(tmp, 'cache'),
			XDG_DATA_HOME: path.join(tmp, 'data')
		},
		{ profile: tmp, onboardingVersion: '0.19.0' }
	);
	const page = await appWindow(app);
	await page.route('**/api/**', async (r) => {
		const p = new URL(r.request().url()).pathname;
		const j = (body: unknown) =>
			answer(r, { status: 200, contentType: 'application/json', body: JSON.stringify(body) });
		// The feature tips are the backend's own.
		if (p.startsWith('/api/onboarding')) return r.continue();
		if (p === '/api/app-info') return j(appInfo);
		if (p === '/api/settings') {
			return r.request().method() === 'PUT' ? answer(r, { status: 204 }) : j(defaultSettings);
		}
		if (p === '/api/home/hero') return r.continue();
		if (p === '/api/kitsu/trending-anilist') return j(trending);
		if (p === '/api/kitsu/top-rated') return j([stripShow]);
		if (p.startsWith('/api/image')) return answer(r, { status: 503 });
		if (p === '/api/history') return j([]);
		if (p === '/api/history/card-lines') return j({});
		if (p === '/api/watched-at') return j({});
		if (p === '/api/availability/batch') {
			const { kitsu_ids } = r.request().postDataJSON() as { kitsu_ids: string[] };
			return j({
				cached: Object.fromEntries(kitsu_ids.map((id) => [id, true])),
				playable_episode_counts: {}
			});
		}
		if (p === '/api/availability/warm') return answer(r, { status: 202 });
		if (p === '/api/update-check') return answer(r, { status: 204 });
		if (p.startsWith('/api/kitsu/anime/')) return j(stripShow);
		if (p.startsWith('/api/kitsu/episodes/')) return j([]);
		if (p === '/api/download/default-dir') return j(null);
		if (p === '/api/availability') {
			return j({ available: true, episode_count: 12, extra_episodes: [] });
		}
		if (p === `/api/watch-order/${stripShow.id}/show`) return j(stripAnswer);
		if (p === `/api/kitsu/franchise/${stripShow.id}/first-hop`) return j({ signal: 'strip' });
		if (p === '/api/kitsu/search') return j([]);
		if (p.startsWith('/api/play/stream')) return answer(r, { status: 503 });
		return j(null);
	});
	await remount(page);
	return { app, page };
}

/** Load the app's first page again, now that the stubs answer it. */
async function remount(page: Page): Promise<void> {
	const home = new URL('/', page.url()).toString();
	await page.goto('about:blank');
	await page.goto(home, { waitUntil: 'domcontentloaded' });
}

async function openShow(page: Page): Promise<void> {
	await page.locator(`a[href$="/anime/${stripShow.id}"]`).first().click();
	await expect.poll(() => page.url()).toContain(`/anime/${stripShow.id}`);
	await expect(page.locator('[data-onboard="watch-order-strip"]')).toBeVisible({ timeout: 15_000 });
}

const balloon = (page: Page) => page.getByTestId('onboarding-balloon');
const scrim = (page: Page) => page.getByTestId('onboarding-scrim');

test('after an update the strip is explained once, and a press outside closes it and goes no further', async () => {
	test.setTimeout(180_000);
	const { app, page } = await launch('update');
	try {
		await page.setViewportSize({ width: 1280, height: 800 });
		await openShow(page);
		await expect(balloon(page)).toBeVisible({ timeout: 10_000 });
		await expect(balloon(page)).toContainText('0.19.0');
		await expect(page.getByRole('button', { name: /got it/i })).toBeFocused();

		// The app's own shortcuts stand down under it.
		await page.keyboard.press('/');
		await expect(page.locator('.topbar-search input')).not.toBeFocused();

		// The window's buttons are what a press on them reaches.
		const buttons = page.locator('.winctl-btn');
		for (let i = 0; i < (await buttons.count()); i++) {
			const hit = await buttons.nth(i).evaluate((b) => {
				const r = b.getBoundingClientRect();
				const at = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
				// The press reaches it: the button under the point, and not inert.
				return at !== null && b.contains(at) && b.closest('[inert]') === null;
			});
			expect(hit, `window button ${i}`).toBe(true);
		}

		// A press on a link under the dim (the rail's way home),
		// past the first moments, closes the tip and opens nothing.
		await page.waitForTimeout(500);
		const before = page.url();
		const under = page.locator('a.brand').first();
		const box = (await under.boundingBox())!;
		const cx = box.x + box.width / 2;
		const cy = box.y + box.height / 2;
		expect(
			await page.evaluate(
				([x, y]) => document.elementFromPoint(x, y)?.closest('[data-testid="onboarding-scrim"]') !== null,
				[cx, cy]
			),
			'the press lands on the dim'
		).toBe(true);
		await page.mouse.click(cx, cy);
		await expect(balloon(page)).toHaveCount(0);
		await expect(scrim(page)).toHaveCount(0);
		expect(page.url()).toBe(before);

		// The backend recorded it: a fresh read does not bring it back.
		await remount(page);
		await openShow(page);
		await page.waitForTimeout(2_000);
		await expect(balloon(page)).toHaveCount(0);
	} finally {
		await stopApp(app);
	}
});

test('a press on the strip itself does what it does and closes the tip', async () => {
	test.setTimeout(180_000);
	const { app, page } = await launch('update');
	try {
		await openShow(page);
		await expect(balloon(page)).toBeVisible({ timeout: 10_000 });
		// Past the first moments: a press then was aimed before the tip.
		await page.waitForTimeout(500);
		const tile = page.locator('[data-onboard="watch-order-strip"] a[href^="/anime/"]').first();
		const href = (await tile.getAttribute('href'))!;
		await tile.click();
		await expect(balloon(page)).toHaveCount(0);
		await expect.poll(() => new URL(page.url()).pathname).toBe(href);
	} finally {
		await stopApp(app);
	}
});

test('Escape closes it', async () => {
	test.setTimeout(180_000);
	const { app, page } = await launch('update');
	try {
		await openShow(page);
		await expect(balloon(page)).toBeVisible({ timeout: 10_000 });
		await page.keyboard.press('Escape');
		await expect(balloon(page)).toHaveCount(0);
	} finally {
		await stopApp(app);
	}
});

test('a first-ever install is never shown it', async () => {
	test.setTimeout(180_000);
	const { app, page } = await launch('first-ever');
	try {
		await openShow(page);
		await page.waitForTimeout(2_500);
		await expect(balloon(page)).toHaveCount(0);
	} finally {
		await stopApp(app);
	}
});

test('in a contrast theme the dim stays and the strip is outlined, and reduced motion fades nothing in', async () => {
	test.setTimeout(180_000);
	const { app, page } = await launch('update');
	try {
		await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
		await openShow(page);
		await expect(balloon(page)).toBeVisible({ timeout: 10_000 });
		const look = await scrim(page).evaluate((s) => {
			const dim = getComputedStyle(s.querySelector('.dim')!);
			const edge = getComputedStyle(s.querySelector('.edge')!);
			return {
				opacity: Number(dim.opacity),
				clip: dim.clipPath,
				outline: edge.outlineStyle,
				animation: dim.animationName
			};
		});
		expect(look.opacity).toBeGreaterThan(0.5);
		expect(look.clip).toContain('path');
		expect(look.outline).toBe('solid');
		expect(look.animation).toBe('none');
	} finally {
		await stopApp(app);
	}
});
