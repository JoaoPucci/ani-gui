/**
 * Going back to the search page restores what the reader left, in the
 * packaged app: SvelteKit's own router keeps the search page's snapshot
 * with the history entry and hands it back on a back press, which the
 * route-level acceptance tests can only play with a mocked router.
 *
 * The reader searches, orders by year, keeps TV series, loads a second
 * page, scrolls a card to the top of the screen, opens it, and goes
 * back — with the browser's back and with the topbar's chevron. The
 * search page must come back with the same query, order, filter,
 * results and scroll, without asking for any page again.
 *
 * The search, availability and image requests are stubbed; the show
 * page's own requests are left to the backend, since the scenario only
 * needs to leave the search page and come back.
 */
import { _electron as electron, expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { killTree, withColdLaunchRetry } from '../lib/cold-launch.cjs';
import fs, { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { appInfo, emptyHistory, topRated, trending } from './fixtures/kitsu';

const electronDir = path.resolve(__dirname, '..');
const packagedBinary = path.join(electronDir, 'dist/linux-unpacked/ani-gui');

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`);
	}
});

/** Show `n` of any search: odd ids are TV series, even ones films. */
function show(n: number) {
	return {
		id: String(n),
		canonical_title: `Show ${n}`,
		slug: `show-${n}`,
		synopsis: null,
		start_date: '2019-01-01',
		end_date: null,
		episode_count: 12,
		average_rating: null,
		subtype: n % 2 === 1 ? 'TV' : 'movie',
		status: 'finished',
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null
	};
}

const json = (body: unknown) => ({
	status: 200,
	contentType: 'application/json',
	body: JSON.stringify(body)
});

async function launchOnce(asked: string[], onLaunch: (app: ElectronApplication) => void) {
	const tmp = path.join(os.tmpdir(), `ani-gui-search-back-${process.pid}-${Date.now()}`);
	fs.mkdirSync(tmp, { recursive: true });
	const app = await electron.launch({
		executablePath: packagedBinary,
		// --mute-audio: nothing here plays sound, and a headless test
		// run must never reach the machine's speakers.
		args: ['--no-sandbox', '--mute-audio'],
		env: {
			...process.env,
			XDG_STATE_HOME: path.join(tmp, 'state'),
			XDG_CONFIG_HOME: path.join(tmp, 'config'),
			XDG_CACHE_HOME: path.join(tmp, 'cache'),
			XDG_DATA_HOME: path.join(tmp, 'data'),
			ANI_GUI_UNATTENDED: '1'
		}
	});
	onLaunch(app);
	const context = app.context();
	await context.route('**/api/app-info', (r) => r.fulfill(json(appInfo)));
	await context.route('**/api/history', (r) => r.fulfill(json(emptyHistory)));
	await context.route('**/api/kitsu/trending', (r) => r.fulfill(json(trending)));
	await context.route('**/api/kitsu/top-rated', (r) => r.fulfill(json(topRated)));
	await context.route('**/api/kitsu/search', (r) => r.fulfill(json([])));
	await context.route('**/api/image*', (r) => r.fulfill({ status: 503 }));
	await context.route('**/api/availability/batch', async (r) => {
		const { kitsu_ids } = r.request().postDataJSON() as { kitsu_ids: string[] };
		await r.fulfill(
			json({
				cached: Object.fromEntries(kitsu_ids.map((id) => [id, true])),
				playable_episode_counts: {}
			})
		);
	});
	// Twenty shows a page, five pages, for any query.
	await context.route('**/api/kitsu/search/page', async (r) => {
		const body = r.request().postDataJSON() as { query: string; offset: number; sort?: string };
		asked.push(`${body.query}@${body.offset}/${body.sort ?? 'relevance'}`);
		const hits = Array.from({ length: 20 }, (_, i) => show(body.offset + i + 1));
		await r.fulfill(json({ hits, next_offset: body.offset + 20 < 100 ? body.offset + 20 : null }));
	});
	const page = await app.firstWindow();
	return { app, page };
}

async function launch(asked: string[]) {
	let pending: ElectronApplication | null = null;
	return withColdLaunchRetry(
		async () => {
			const launched = await launchOnce(asked, (app) => {
				pending = app;
			});
			pending = null;
			return launched;
		},
		{
			cleanup: async () => {
				const dead = pending as ElectronApplication | null;
				pending = null;
				const pid = dead?.process().pid;
				if (pid) killTree(pid);
			}
		}
	);
}

const card = (page: Page, id: string) => page.locator(`[data-card-id="${id}"]`);
const cardTop = (page: Page, id: string) =>
	card(page, id).evaluate((el) => Math.round(el.getBoundingClientRect().top));

/** Search "frieren", order by year, keep TV series, load a second page
 *  and scroll show 21 (the first of page two) 40px below the top. The
 *  real layout may let infinite scroll load further pages on its own;
 *  what matters is that going back asks for none of them again. */
async function browse(page: Page) {
	const field = page.getByRole('searchbox');
	await field.fill('frieren');
	await field.press('Enter');
	await expect(card(page, '1')).toBeVisible();
	await page.getByRole('radio', { name: 'Year' }).click();
	await expect(card(page, '2')).toBeVisible();
	await page.getByRole('button', { name: 'TV', exact: true }).click();
	await expect(card(page, '2')).toHaveCount(0);
	await page.getByRole('button', { name: 'Load more results' }).click();
	await expect(card(page, '21')).toBeVisible();
	await card(page, '21').evaluate((el) => {
		el.scrollIntoView({ block: 'start', behavior: 'instant' });
		window.scrollBy({ top: -40, behavior: 'instant' });
	});
	// Let any entrance animation settle before measuring.
	await page.waitForTimeout(600);
	return cardTop(page, '21');
}

async function expectRestored(page: Page, top: number, asked: string[], askedBefore: string[]) {
	await expect(card(page, '21')).toBeVisible();
	await expect(page.locator('.eyebrow-value')).toHaveText('frieren');
	await expect(page.getByRole('radio', { name: 'Year' })).toHaveAttribute('aria-checked', 'true');
	await expect(page.getByRole('button', { name: 'TV', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await expect(card(page, '2')).toHaveCount(0);
	await expect.poll(() => cardTop(page, '21')).toBe(top);
	expect(asked).toEqual(askedBefore);
}

test('the back button returns to the search as it was left', async () => {
	const asked: string[] = [];
	const { app, page } = await launch(asked);
	try {
		const top = await browse(page);
		const askedBefore = [...asked];
		expect(askedBefore).toContain('frieren@20/year');

		await card(page, '21').locator('a').click();
		await expect(page).toHaveURL(/\/anime\/21/);
		await page.goBack();
		await expect(page).toHaveURL(/\/search\?q=frieren/);
		await expectRestored(page, top, asked, askedBefore);
	} finally {
		await app.close();
	}
});

test('the topbar chevron names the search and returns to it as it was left', async () => {
	const asked: string[] = [];
	const { app, page } = await launch(asked);
	try {
		const top = await browse(page);
		const askedBefore = [...asked];

		await card(page, '21').locator('a').click();
		await expect(page).toHaveURL(/\/anime\/21/);
		const crumbs = page.getByRole('navigation', { name: 'Breadcrumb' });
		await expect(crumbs.getByRole('link', { name: 'Search', exact: true })).toBeVisible();
		await crumbs.getByRole('link', { name: 'Go up one level' }).click();
		await expect(page).toHaveURL(/\/search\?q=frieren/);
		await expectRestored(page, top, asked, askedBefore);
	} finally {
		await app.close();
	}
});
