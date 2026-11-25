/**
 * The tools 0.19.0 ships before a feature uses them, on the packaged app
 * with the real backend answering the feature tips' routes, started the
 * way the maintainer starts them: the Debug page's "Show it".
 *
 * - the entry point: "Show it" on `franchise-entry`, then the show page,
 *   where the franchise link is lit with "Show me how" and "Got it";
 *   "Show me how" opens the franchise page, and its walkthrough runs
 *   there ("How it works", no version);
 * - the short sequence: Next, Back, Next, Next to Done, the lit place
 *   moving with each step, focus on the primary action, Tab staying in
 *   the balloon;
 * - Skip ends the walkthrough at once;
 * - nothing shown this way is marked seen.
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
const franchiseAnswer = scenarioAnswer(
	'franchise-page',
	'shows the name the franchise read gives and counts what it lists'
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
		if (p.startsWith('/api/home/hero')) return r.continue();
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
		if (p === `/api/watch-order/${stripShow.id}/franchise`) return j(franchiseAnswer);
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

/** Navigate inside the app, as a link press does, keeping the session. */
async function go(page: Page, href: string): Promise<void> {
	await page.evaluate((to) => {
		const a = document.createElement('a');
		a.href = to;
		document.body.appendChild(a);
		a.click();
		a.remove();
	}, href);
	await expect.poll(() => new URL(page.url()).pathname).toBe(href);
}

async function showIt(page: Page, key: string): Promise<void> {
	await go(page, '/diagnostics');
	const row = page.locator(`[data-testid="tip-key"][data-id="${key}"]`);
	await row.getByRole('button', { name: /show it/i }).click();
	await expect(row).toContainText(/waiting/i);
}

const balloon = (page: Page) => page.getByTestId('onboarding-balloon');
const lit = (page: Page) =>
	page.evaluate(() => {
		const f = document.querySelector<HTMLElement>('[data-testid="onboarding-scrim"] .frame');
		return f ? f.style.translate : null;
	});

test('the entry point leads to its walkthrough, which steps to Done and marks nothing seen', async () => {
	test.setTimeout(240_000);
	const { app, page } = await launch('update');
	try {
		await page.setViewportSize({ width: 1280, height: 800 });
		// The strip's own tip, pending after an update, is marked seen
		// first, so the show page's one tip is the one asked for.
		await go(page, '/diagnostics');
		const strip = page.locator('[data-testid="tip-key"][data-id="watch-order"] input[type="checkbox"]');
		await strip.check();
		await expect(strip).toBeEnabled();
		await showIt(page, 'franchise-entry');
		await go(page, `/anime/${stripShow.id}`);
		// The tip waits for its place to be on screen and still.
		const link = page.locator('[data-onboard="watch-order-franchise"]');
		await link.scrollIntoViewIfNeeded();
		await expect(balloon(page)).toBeVisible({ timeout: 15_000 });
		await expect(balloon(page).getByRole('button')).toHaveText([/show me how/i, /got it/i]);
		await balloon(page).getByRole('button', { name: /show me how/i }).click();
		await expect.poll(() => new URL(page.url()).pathname).toBe(`/anime/${stripShow.id}/franchise`);

		await expect(balloon(page)).toBeVisible({ timeout: 15_000 });
		await expect(balloon(page)).toContainText(/how it works/i);
		await expect(balloon(page)).not.toContainText('0.19.0');
		await expect(balloon(page)).toContainText(/1 of 3/);
		const next = balloon(page).getByRole('button', { name: /next/i });
		await expect(next).toBeFocused();
		// Tab stays in the balloon.
		await page.keyboard.press('Tab');
		await page.keyboard.press('Tab');
		await expect(balloon(page).locator(':focus')).toHaveCount(1);

		const first = await lit(page);
		await next.click();
		await expect(balloon(page)).toContainText(/2 of 3/);
		await expect.poll(() => lit(page)).not.toBe(first);
		await balloon(page).getByRole('button', { name: /back/i }).click();
		await expect(balloon(page)).toContainText(/1 of 3/);
		await balloon(page).getByRole('button', { name: /next/i }).click();
		await balloon(page).getByRole('button', { name: /next/i }).click();
		await expect(balloon(page)).toContainText(/3 of 3/);
		await balloon(page).getByRole('button', { name: /done/i }).click();
		await expect(balloon(page)).toHaveCount(0);

		// Nothing shown this way was marked seen.
		await go(page, '/diagnostics');
		for (const key of ['franchise-entry', 'franchise-tour']) {
			await expect(
				page.locator(`[data-testid="tip-key"][data-id="${key}"] input[type="checkbox"]`)
			).not.toBeChecked();
		}
	} finally {
		await stopApp(app);
	}
});

test('Skip ends a walkthrough at once', async () => {
	test.setTimeout(240_000);
	const { app, page } = await launch('update');
	try {
		await showIt(page, 'franchise-tour');
		await go(page, `/anime/${stripShow.id}/franchise`);
		await expect(balloon(page)).toBeVisible({ timeout: 15_000 });
		await balloon(page).getByRole('button', { name: /skip/i }).click();
		await expect(balloon(page)).toHaveCount(0);
	} finally {
		await stopApp(app);
	}
});
