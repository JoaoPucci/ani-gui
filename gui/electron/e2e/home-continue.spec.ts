/**
 * Acceptance coverage for the home Continue Watching strip — the
 * user-visible flow PR #50 reshaped.
 *
 * AGENTS.md §2 requires acceptance coverage when a user-visible
 * flow changes; this suite covers the four states the Continue
 * card transitions through:
 *
 *   1. Loading (pre-onRowReady)        → non-interactive, no /search
 *   2. Match resolved, count cached    → button, badge shows last+1
 *   3. Match resolved, at the cap      → button, badge shows last (replay)
 *   4. Match unresolvable              → /search fallback link
 *
 * The backend's IPC endpoints (history, watched-at, settings, kitsu
 * detail/episodes, availability, play) are stubbed via `page.route()`
 * so the assertions don't depend on Kitsu/allmanga reachability.
 *
 * The resolveKitsuMatch path takes the `allmanga-kitsu-map` short-
 * circuit (step 0 in match.ts) — stubbing that endpoint plus the
 * kitsu-anime-detail it points to is enough to drive a deterministic
 * match. Live `kitsuSearch` is stubbed for the orphan case only.
 */
import { _electron as electron, expect, test, type Page } from '@playwright/test';
import fs, { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
	appInfo,
	atCapHistory,
	continueHistory,
	continueKitsuEpisode6,
	continueKitsuMatch,
	defaultSettings,
	emptyHistory,
	orphanHistory,
	topRated,
	trending
} from './fixtures/kitsu';

const electronDir = path.resolve(__dirname, '..');
const packagedBinary = path.join(electronDir, 'dist/linux-unpacked/ani-gui');

test.beforeAll(() => {
	if (!existsSync(packagedBinary)) {
		throw new Error(
			`prereq missing: ${packagedBinary}\nrun: cd gui/electron && pnpm run package`
		);
	}
});

interface StubOptions {
	history: typeof continueHistory | typeof emptyHistory;
	/** Resolves the allmanga show id to a kitsu id (the short-circuit
	 *  path in match.ts step 0). `null` forces the loader to fall
	 *  through to title-match / kitsuSearch — the orphan case. */
	allmangaKitsuMap?: string | null;
	/** Delay the availability probe to widen the loading window for
	 *  the "non-interactive loading" assertions. */
	availabilityDelayMs?: number;
	/** Hook fired whenever the renderer posts to /api/play — lets
	 *  the test assert the click handler ran with the right episode. */
	onPlay?: (body: unknown) => void;
}

async function launchAppWithContinueStubs(opts: StubOptions) {
	const tmp = path.join(os.tmpdir(), `ani-gui-continue-${process.pid}-${Date.now()}`);
	fs.mkdirSync(tmp, { recursive: true });
	const cleanEnv = {
		...process.env,
		XDG_STATE_HOME: path.join(tmp, 'state'),
		XDG_CONFIG_HOME: path.join(tmp, 'config'),
		XDG_CACHE_HOME: path.join(tmp, 'cache'),
		XDG_DATA_HOME: path.join(tmp, 'data')
	};

	const app = await electron.launch({
		executablePath: packagedBinary,
		args: ['--no-sandbox'],
		env: cleanEnv
	});
	const context = app.context();
	const page = await app.firstWindow();

	const watchedAt: Record<string, number> = {};
	for (const h of opts.history) watchedAt[h.id] = 1_700_000_000;

	// Single consolidated route handler registered on the PAGE (not
	// the context). page.route() applies to the specific page from the
	// moment it's registered onward; the bounce below then forces a
	// fresh navigation whose fetches all flow through this handler.
	// Earlier attempts to register on context.route before firstWindow
	// raced the renderer's initial onMount fetch batch (CI runs
	// 26891889692, 26892237750, 26893103223 — test #4 screenshot
	// consistently showed real-Kitsu content despite the bounce).
	// Registering AFTER firstWindow on the page means there's no
	// pre-page window where the route can be missed.
	await page.route('**/api/**', async (r) => {
		const u = new URL(r.request().url());
		const p = u.pathname;
		const j = (body: unknown) =>
			r.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(body)
			});

		if (p === '/api/app-info') return j(appInfo);
		if (p === '/api/settings') return j(defaultSettings);
		if (p === '/api/kitsu/trending-anilist') return j(trending);
		if (p === '/api/kitsu/top-rated') return j(topRated);
		if (p.startsWith('/api/image')) return r.fulfill({ status: 503 });
		if (p === '/api/history') return j(opts.history);
		if (p === '/api/watched-at') return j(watchedAt);

		if (p.startsWith('/api/allmanga-kitsu-map/')) {
			if (opts.allmangaKitsuMap !== null && p.includes(continueHistory[0].id)) {
				return j(opts.allmangaKitsuMap ?? continueKitsuMatch.id);
			}
			return j(null);
		}
		if (p.startsWith('/api/kitsu/anime/')) return j(continueKitsuMatch);
		if (p.startsWith('/api/kitsu/episodes/')) return j([continueKitsuEpisode6]);
		if (p.startsWith('/api/title-match')) return j(null);
		if (p === '/api/kitsu/search') return j([]);

		if (p === '/api/availability') {
			if (opts.availabilityDelayMs) {
				await new Promise((resolve) => setTimeout(resolve, opts.availabilityDelayMs));
			}
			return j({
				available: true,
				episode_count: continueKitsuMatch.episode_count,
				extra_episodes: []
			});
		}

		if (p === '/api/play/stream') {
			const body = {
				title: u.searchParams.get('title'),
				episode: u.searchParams.get('episode'),
				mode: u.searchParams.get('mode'),
				quality: u.searchParams.get('quality')
			};
			opts.onPlay?.(body);
			return r.fulfill({
				status: 200,
				contentType: 'text/event-stream',
				body: `event: done\ndata: ${JSON.stringify({
					session_id: 'test-session',
					upstream_url: 'about:blank',
					referer: '',
					subtitle_url: null,
					episode: Number(body.episode),
					media_kind: 'Hls'
				})}\n\n`
			});
		}
		if (p === '/api/play') {
			const body = JSON.parse(r.request().postData() ?? '{}');
			opts.onPlay?.(body);
			return j({
				session_id: 'test-session',
				upstream_url: 'about:blank',
				referer: '',
				subtitle_url: null,
				episode: body.episode,
				media_kind: 'Hls'
			});
		}

		// Default: empty 200 so unknown probes don't blow up under Xvfb
		// (the test cares about the home-strip surfaces; everything else
		// just needs to not error out).
		return j(null);
	});

	// Bounce through about:blank to force a full SvelteKit remount.
	// goto() to the same URL is a SPA no-op (SvelteKit treats it as
	// client-side navigation, onMount doesn't re-run, the racing
	// initial-load state persists). about:blank drops the runtime
	// entirely; the goto back to the home URL fires a fresh mount,
	// and every /api/* it issues now flows through page.route()
	// because the handler was registered before the bounce.
	const homeUrl = page.url();
	await page.waitForLoadState('networkidle').catch(() => {});
	await page.goto('about:blank');
	await page.goto(homeUrl, { waitUntil: 'domcontentloaded' });
	return { app, page, context };
}

async function waitForStripVisible(page: Page) {
	await expect(page.getByText(/top rated/i).first()).toBeVisible({ timeout: 15_000 });
}

test('Continue card shows last+1 and clicking it plays that episode', async () => {
	let playArgs: Record<string, string | null> | null = null;
	const { app, page } = await launchAppWithContinueStubs({
		history: continueHistory,
		onPlay: (body) => {
			playArgs = body as typeof playArgs;
		}
	});
	try {
		await waitForStripVisible(page);

		const strip = page.getByRole('region', { name: /continue watching/i });
		await expect(strip).toBeVisible({ timeout: 10_000 });

		// Card transitions to its resumable button form via onRowReady.
		const card = strip.getByRole('button').first();
		await expect(card).toBeVisible({ timeout: 10_000 });

		// last_watched=5, cap=12 → pickNextEpisode = 6. Badge surfaces
		// the episode the click would actually play.
		await expect(card).toContainText('6');

		await card.click();
		await expect.poll(() => playArgs?.episode, { timeout: 10_000 }).toBe('6');
		// playStream encodes title + mode in the SSE query string;
		// kitsu_id isn't on the wire (the backend reads it from the
		// reverse cache during resolution), so assert what is.
		expect(playArgs?.title).toBe(continueKitsuMatch.canonical_title);
		expect(playArgs?.mode).toBe('sub');
	} finally {
		await app.close();
	}
});

test('Continue card at the announced cap shows the same episode (replay)', async () => {
	const { app, page } = await launchAppWithContinueStubs({ history: atCapHistory });
	try {
		await waitForStripVisible(page);

		const strip = page.getByRole('region', { name: /continue watching/i });
		const card = strip.getByRole('button').first();
		await expect(card).toBeVisible({ timeout: 10_000 });

		// last_watched=12, cap=12 → pickNextEpisode returns 12 (replay)
		// so the card surfaces ep 12 in its badge — not 13, which the
		// stream wouldn't have anyway.
		await expect(card).toContainText('12');
	} finally {
		await app.close();
	}
});

test('Continue row whose match is unresolvable renders as a /search fallback link', async () => {
	const { app, page } = await launchAppWithContinueStubs({
		history: orphanHistory,
		allmangaKitsuMap: null
	});
	try {
		await waitForStripVisible(page);

		const strip = page.getByRole('region', { name: /continue watching/i });
		const fallback = strip.getByRole('link').first();
		await expect(fallback).toBeVisible({ timeout: 10_000 });
		await expect(fallback).toHaveAttribute('href', /\/search/);
	} finally {
		await app.close();
	}
});

test('Continue card during the availability-probe window is not a /search link', async () => {
	// Codex P2 #3348970892: while the per-row probe is in flight,
	// historyMatches[entry.id] is still undefined and the prior code
	// fell into the /search fallback branch, so a click during the
	// window navigated to search even when the row's Kitsu match was
	// only one IPC away from being usable. The fix renders the row
	// as a non-interactive loading card during that window. Assertion:
	// no /search link inside the Continue strip while the probe is
	// pending.
	const { app, page } = await launchAppWithContinueStubs({
		history: continueHistory,
		// Tight enough that the 500ms assertion below lands inside
		// the probe window AND the button-visibility wait has
		// headroom under the 10s timeout — the goto-replay in
		// launchAppWithContinueStubs fires /api/availability twice
		// (once per render cycle), so a longer delay doubles up
		// under Xvfb.
		availabilityDelayMs: 1_500
	});
	try {
		await waitForStripVisible(page);

		const strip = page.getByRole('region', { name: /continue watching/i });
		await expect(strip).toBeVisible({ timeout: 10_000 });
		// During the probe-pending window the row should not be a
		// search link — that's the regression Codex flagged. Allow a
		// short settle so the renderer has time to mount its loading
		// state, but keep the assertion well inside the probe delay.
		await page.waitForTimeout(500);
		const searchLinks = strip.locator('a[href*="/search"]');
		expect(await searchLinks.count()).toBe(0);

		// After the probe lands the card flips to its button form.
		const card = strip.getByRole('button').first();
		await expect(card).toBeVisible({ timeout: 10_000 });
	} finally {
		await app.close();
	}
});
