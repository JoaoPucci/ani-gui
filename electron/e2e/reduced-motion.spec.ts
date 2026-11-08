/**
 * Under reduced motion nothing in the packaged app moves — asserted in
 * its own Chromium, where CSS animations and transitions are real.
 *
 * The page is emulated as `prefers-reduced-motion: reduce` before the
 * app's first real load, the way a desktop setting reaches it, and
 * each main surface is then walked: Home (the hero, the rails, a
 * Continue Watching card hovered and deleted), a show's page (loaded
 * and scrolled) and the player with its overlay. At each step
 * `document.getAnimations()` — every CSS animation, CSS transition and
 * Web Animation on the page — must hold nothing that moves.
 *
 * "Moves" means running with time in it. A transition the stylesheet
 * gives no duration but a delay (the player's spinner waits 240 ms
 * before it shows, so a fast load never flashes it) is a pending
 * zero-length switch: it changes the property at once when the delay
 * ends and draws no frame in between, so it is not counted.
 *
 * What `getAnimations()` cannot see: the loader's Lottie, which draws
 * from its own frame loop, and the hero parallax, which is a transform
 * set from script on scroll. The acceptance tier covers both
 * (frontend/tests/acceptance/reduced-motion-*.test.ts): the loader
 * through its stub, the parallax on Home and a show's page by scrolling
 * them.
 *
 * One scenario runs the same delete with motion on and requires that
 * something does move, so the check cannot pass by looking at a page
 * where nothing could.
 *
 * Standing still must not mean hidden: one scenario checks that the
 * Continue Watching cards, once landed, show their artwork and text with
 * the animations that reveal them taken off.
 *
 * The backend's API is stubbed per request, as in home-continue.spec.ts,
 * so nothing depends on Kitsu or the provider being reachable.
 */
import {
	expect,
	test,
	type ElectronApplication,
	type Page
} from '@playwright/test';
import { killTree, withColdLaunchRetry } from '../lib/cold-launch.cjs';
import fs, { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
	appInfo,
	continueKitsuEpisode6,
	continueKitsuMatch,
	defaultSettings,
	topRated,
	trending
} from './fixtures/kitsu';
import { launchPackagedApp, packagedBinary } from './launch';

/** Two rows: the first resolves to the stubbed show and is the one
 *  deleted, so the second has a gap to close. */
const history = [
	{ id: 'allmanga-test-show', ep_no: '5', title: 'Continue Test Show' },
	{ id: 'allmanga-second-show', ep_no: '2', title: 'Second Test Show' }
];

const episodes = Array.from({ length: 12 }, (_, i) => ({
	...continueKitsuEpisode6,
	id: `ep-${i + 1}`,
	number: i + 1,
	relative_number: i + 1,
	canonical_title: `Episode ${i + 1}`
}));

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(`prereq missing: ${packagedBinary}\nrun: cd electron && pnpm run package`);
	}
});

/** A 1×1 PNG: artwork that loads and decodes, so a card can show it. */
const PIXEL = Buffer.from(
	'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
	'base64'
);

async function launchOnce(
	reduced: boolean,
	onLaunch: (app: ElectronApplication) => void,
	images = false
) {
	const tmp = path.join(os.tmpdir(), `ani-gui-reduced-motion-${process.pid}-${Date.now()}`);
	fs.mkdirSync(tmp, { recursive: true });
	const app = await launchPackagedApp({
		...process.env,
		XDG_STATE_HOME: path.join(tmp, 'state'),
		XDG_CONFIG_HOME: path.join(tmp, 'config'),
		XDG_CACHE_HOME: path.join(tmp, 'cache'),
		XDG_DATA_HOME: path.join(tmp, 'data'),
		ANI_GUI_UNATTENDED: '1'
	});
	onLaunch(app);
	const page = await app.firstWindow();

	let rows = [...history];
	await page.route('**/api/**', async (r) => {
		const u = new URL(r.request().url());
		const p = u.pathname;
		const method = r.request().method();
		const j = (body: unknown) =>
			r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });

		if (p === '/api/app-info') return j(appInfo);
		if (p === '/api/settings') return j(defaultSettings);
		if (p === '/api/kitsu/trending-anilist') return j(trending);
		if (p === '/api/kitsu/top-rated') return j(topRated);
		if (p.startsWith('/api/image')) {
			return images
				? r.fulfill({ status: 200, contentType: 'image/png', body: PIXEL })
				: r.fulfill({ status: 503 });
		}
		if (p === '/api/history') return j(rows);
		if (p.startsWith('/api/history/') && method === 'DELETE') {
			const id = decodeURIComponent(p.split('/')[3] ?? '');
			rows = rows.filter((row) => row.id !== id);
			return r.fulfill({ status: 204 });
		}
		if (p === '/api/watched-at') {
			return j(Object.fromEntries(rows.map((row) => [row.id, 1_700_000_000])));
		}
		if (p.startsWith('/api/allmanga-kitsu-map/')) {
			return j(p.includes(history[0].id) ? continueKitsuMatch.id : null);
		}
		if (p.startsWith('/api/kitsu/anime/')) return j(continueKitsuMatch);
		if (p.startsWith('/api/kitsu/episodes/')) return j(episodes);
		if (p.startsWith('/api/title-match')) return j(null);
		if (p === '/api/kitsu/search') return j([]);
		if (p === '/api/availability') {
			return j({ available: true, episode_count: 12, extra_episodes: [] });
		}
		if (p === '/api/play/stream') {
			return r.fulfill({
				status: 200,
				contentType: 'text/event-stream',
				body: `event: done\ndata: ${JSON.stringify({
					session_id: 'test-session',
					upstream_url: 'about:blank',
					referer: '',
					subtitle_url: null,
					episode: Number(u.searchParams.get('episode')),
					media_kind: 'Hls'
				})}\n\n`
			});
		}
		return j(null);
	});

	// The emulation outlives navigations on this page, so the app's
	// fresh load below — and the one reading of the preference it makes
	// at start — sees reduced motion from its first frame.
	await page.emulateMedia({ reducedMotion: reduced ? 'reduce' : 'no-preference' });

	// Bounce through about:blank for a fresh mount whose every request
	// flows through the handler above (see home-continue.spec.ts).
	const homeUrl = page.url();
	await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});
	await page.goto('about:blank');
	await page.goto(homeUrl, { waitUntil: 'domcontentloaded' });
	return { app, page };
}

async function launch(reduced = true, images = false) {
	let pending: ElectronApplication | null = null;
	return withColdLaunchRetry(
		async () => {
			const launched = await launchOnce(
				reduced,
				(app) => {
					pending = app;
				},
				images
			);
			pending = null;
			return launched;
		},
		{
			cleanup: async () => {
				const dead = pending;
				pending = null;
				if (dead) {
					killTree(dead.process().pid);
					await dead.close();
				}
			}
		}
	);
}

/** Everything on the page that is moving right now, described. */
function moving(page: Page): Promise<string[]> {
	return page.evaluate(() =>
		document
			.getAnimations()
			.filter((a) => {
				if (a.playState !== 'running') return false;
				const timing = a.effect?.getComputedTiming();
				const active = Number(timing?.activeDuration ?? 0);
				return active > 0;
			})
			.map((a) => {
				const target = (a.effect as KeyframeEffect | null)?.target as Element | null;
				const name =
					(a as unknown as { animationName?: string }).animationName ??
					(a as unknown as { transitionProperty?: string }).transitionProperty ??
					a.constructor.name;
				const cls = target?.getAttribute('class') ?? '';
				return `${a.constructor.name} ${name} on ${target?.tagName.toLowerCase()}.${cls}`;
			})
	);
}

/** Samples the page through `ms`: motion can start a frame after the
 *  step that caused it. */
async function expectStill(page: Page, step: string, ms = 600) {
	const deadline = Date.now() + ms;
	do {
		expect(await moving(page), `moving after: ${step}`).toEqual([]);
		await page.waitForTimeout(50);
	} while (Date.now() < deadline);
}

test('Home, a deleted Continue Watching card and a show’s page stand still', async () => {
	const { app, page } = await launch();
	try {
		expect(
			await page.evaluate(() => matchMedia('(prefers-reduced-motion: reduce)').matches)
		).toBe(true);

		await expect(page.getByText(/top rated/i).first()).toBeVisible({ timeout: 15_000 });
		const strip = page.getByRole('region', { name: /continue watching/i });
		const card = strip.locator('button.resume-card', { hasText: 'Continue Test Show' });
		await expect(card).toBeVisible({ timeout: 10_000 });
		await expectStill(page, 'Home loaded');

		const hero = page.locator('section.hero');
		await hero.hover();
		await expectStill(page, 'hovering the hero');
		await card.hover();
		await expectStill(page, 'hovering a Continue Watching card');

		// Delete the card the way a user does: its chip, then the dialog.
		await page.getByRole('button', { name: 'Remove Continue Test Show from history' }).click();
		const confirm = page.getByRole('button', { name: 'Remove', exact: true });
		await expect(confirm).toBeVisible();
		await expectStill(page, 'opening the delete confirmation', 300);
		await confirm.click();
		await expect(card).toHaveCount(0, { timeout: 5_000 });
		await expectStill(page, 'deleting a Continue Watching card');

		// A show's page, from a Top Rated card.
		await page
			.getByRole('region', { name: /top rated/i })
			.getByRole('link')
			.first()
			.click();
		await expect(page.locator('li[data-ep-num="12"]')).toBeVisible({ timeout: 10_000 });
		await expectStill(page, 'a show’s page loaded');

		await page.mouse.move(640, 400);
		await page.mouse.wheel(0, 400);
		await expect.poll(() => page.evaluate(() => window.scrollY)).toBeGreaterThan(0);
		await expectStill(page, 'scrolling a show’s page');
	} finally {
		killTree(app.process().pid);
		await app.close();
	}
});

test('a Continue Watching card that has landed shows its artwork and text', async () => {
	// The card's artwork, number, scrim and text fade in with keyframes
	// that start from transparent. With the animations taken off, what
	// shows is each element's own style, which must be the visible one:
	// a reveal that only an animation finishes would leave the card
	// blank. One card lands on its image (served here), the other,
	// matched to nothing, on its initials.
	const { app, page } = await launch(true, true);
	try {
		const strip = page.getByRole('region', { name: /continue watching/i });
		const imageCard = strip.locator('.resume-poster[data-look="image"]');
		const initialsCard = strip.locator('.resume-poster[data-look="placeholder"]');
		await expect(imageCard.locator('img[data-revealed]')).toHaveCount(1, { timeout: 15_000 });
		await expect(initialsCard).toHaveCount(1, { timeout: 10_000 });
		await expectStill(page, 'Continue Watching landed');

		const seen = await page.evaluate(() => {
			const out: { what: string; opacity: string; visibility: string; display: string }[] = [];
			const note = (what: string, el: Element, pseudo?: string) => {
				const cs = getComputedStyle(el, pseudo);
				out.push({ what, opacity: cs.opacity, visibility: cs.visibility, display: cs.display });
			};
			const strip = document.querySelector('.resume-poster')?.closest('section') ?? document;
			for (const poster of strip.querySelectorAll('.resume-poster:not([data-look="pending"])')) {
				const kind = poster.getAttribute('data-look');
				note(`${kind} scrim`, poster, '::after');
				for (const sel of ['img', '.resume-poster-placeholder', '.resume-ep-tag']) {
					const el = poster.querySelector(sel);
					if (el) note(`${kind} ${sel}`, el);
				}
				const body = poster.parentElement?.querySelector('.resume-body');
				if (body) note(`${kind} .resume-body`, body);
			}
			return out;
		});
		const kinds = seen.map((s) => s.what);
		for (const required of [
			'image img',
			'image .resume-ep-tag',
			'image scrim',
			'image .resume-body',
			'placeholder .resume-poster-placeholder',
			'placeholder .resume-ep-tag',
			'placeholder scrim',
			'placeholder .resume-body'
		]) {
			expect(kinds, `a landed card shows its ${required}`).toContain(required);
		}
		for (const s of seen) {
			expect(Number(s.opacity), `${s.what} is not transparent`).toBeGreaterThan(0.5);
			expect(s.visibility, `${s.what} is visible`).toBe('visible');
			expect(s.display, `${s.what} is displayed`).not.toBe('none');
		}
	} finally {
		killTree(app.process().pid);
		await app.close();
	}
});

test('the player and its overlay stand still', async () => {
	const { app, page } = await launch();
	try {
		const strip = page.getByRole('region', { name: /continue watching/i });
		const card = strip.locator('button.resume-card', { hasText: 'Continue Test Show' });
		await expect(card).toContainText('6', { timeout: 15_000 });
		await card.click();

		await expect(page.locator('section.player-frame')).toBeVisible({ timeout: 10_000 });
		await expectStill(page, 'opening the player', 1_500);

		await page.locator('section.player-frame').hover();
		await expectStill(page, 'hovering the player');

		// Another episode from the strip under the player, which shows the
		// page of five holding the one playing (6).
		await page.locator('li[data-ep-num="7"] button').first().click();
		await expectStill(page, 'switching episodes', 1_000);
	} finally {
		killTree(app.process().pid);
		await app.close();
	}
});

test('with motion on, the same delete does move', async () => {
	const { app, page } = await launch(false);
	try {
		expect(
			await page.evaluate(() => matchMedia('(prefers-reduced-motion: reduce)').matches)
		).toBe(false);
		const strip = page.getByRole('region', { name: /continue watching/i });
		const card = strip.locator('button.resume-card', { hasText: 'Continue Test Show' });
		await expect(card).toBeVisible({ timeout: 15_000 });

		// With motion on the hovered card lifts while the pointer arrives,
		// and its poster passes over the chip mid-lift; the chip is
		// clicked directly, since the motion under test is the delete's.
		await page
			.getByRole('button', { name: 'Remove Continue Test Show from history' })
			.dispatchEvent('click');
		await page.getByRole('button', { name: 'Remove', exact: true }).click();
		const seen: string[] = [];
		const deadline = Date.now() + 600;
		while (Date.now() < deadline && seen.length === 0) {
			seen.push(...(await moving(page)));
			await page.waitForTimeout(20);
		}
		expect(seen.length).toBeGreaterThan(0);
	} finally {
		killTree(app.process().pid);
		await app.close();
	}
});
