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
import {
	_electron as electron,
	expect,
	test,
	type ElectronApplication,
	type Page
} from '@playwright/test';
import { withColdLaunchRetry } from '../lib/cold-launch.cjs';
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
	/** Override the episode's canonical title. A long one wraps at the
	 *  card width, which is the case the card-height assertion needs
	 *  and a short title cannot exercise. Explicit `null` means Kitsu
	 *  knows the episode but carries no title for it — the branch that
	 *  falls back to "Episode N", or to nothing at all on a movie. */
	episodeTitle?: string | null;
	/** Never answer the Kitsu detail lookup, so the row's match stays
	 *  `undefined` — exactly the condition the loading branch renders
	 *  on. The placeholder is then a stable state rather than a
	 *  transient one. Hanging the availability probe instead does NOT
	 *  work: the row resolves without waiting for it. */
	matchHang?: boolean;
	/** Serve the show as a movie. Combined with `episodeTitle: null`
	 *  this is the only variant whose body has no episode line. */
	singleVideo?: boolean;
	/** Hook fired whenever the renderer posts to /api/play — lets
	 *  the test assert the click handler ran with the right episode. */
	onPlay?: (body: unknown) => void;
	/** Per-request hook for /api/play/stream. Return 'hang' to leave
	 *  that request pending forever (only the renderer's own abort
	 *  releases it) — the takeover test uses this to pin a background
	 *  prefetch in its in-flight state. Anything else falls through
	 *  to the normal instant `done` fulfillment.
	 *
	 *  Must be wired through here rather than a per-test
	 *  `page.route()`: registering ANY additional route after this
	 *  harness's handler stops EventSource (SSE) requests from being
	 *  intercepted at all in this Electron/Playwright combination —
	 *  fetches keep routing, the SSEs silently hit the real backend. */
	onPlayStream?: (url: URL) => 'hang' | undefined;
	/** Make the play stream answer with the backend's typed
	 *  rate-limit envelope (allanime's in-band "Too many requests,
	 *  please try again in N seconds" → kind: 'rate_limited' +
	 *  retry_after_secs) instead of a done event. */
	playRateLimited?: boolean;
}

async function launchAppWithContinueStubsOnce(
	opts: StubOptions,
	onLaunch: (app: ElectronApplication) => void
) {
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
	onLaunch(app);
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
		if (p.startsWith('/api/kitsu/anime/')) {
			// Never fulfilled: the request stays pending for the life of
			// the app, so the row never leaves its loading state.
			if (opts.matchHang) return;
			return j(
				opts.singleVideo
					? { ...continueKitsuMatch, subtype: 'movie', episode_count: 1 }
					: continueKitsuMatch
			);
		}
		if (p.startsWith('/api/kitsu/episodes/'))
			return j([
				opts.episodeTitle === undefined
					? continueKitsuEpisode6
					: { ...continueKitsuEpisode6, canonical_title: opts.episodeTitle }
			]);
		if (p.startsWith('/api/title-match')) return j(null);
		if (p === '/api/kitsu/search') return j([]);

		if (p === '/api/availability') {
			if (opts.availabilityDelayMs) {
				await new Promise((resolve) => setTimeout(resolve, opts.availabilityDelayMs));
			}
			return j({
				available: true,
				episode_count: opts.singleVideo ? 1 : continueKitsuMatch.episode_count,
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
			if (opts.onPlayStream?.(u) === 'hang') return;
			if (opts.playRateLimited) {
				// Same envelope ani_error_to_sse_payload serializes for
				// AniError::RateLimited — the renderer's SSE error
				// listener rejects with this parsed object.
				return r.fulfill({
					status: 200,
					contentType: 'text/event-stream',
					body: `event: error\ndata: ${JSON.stringify({
						kind: 'rate_limited',
						key: 'error.network.rate_limited',
						retry_after_secs: 9
					})}\n\n`
				});
			}
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

async function launchAppWithContinueStubs(opts: StubOptions) {
	// A cold launch can lose its first window underneath the bounce —
	// the closed-target flake — before any assertion runs. The harness
	// relaunches once, closing the dead app in between; every other
	// failure propagates untouched. See lib/cold-launch.cjs.
	let pending: ElectronApplication | null = null;
	return withColdLaunchRetry(
		async () => {
			const launched = await launchAppWithContinueStubsOnce(opts, (app) => {
				pending = app;
			});
			pending = null;
			return launched;
		},
		{
			cleanup: async () => {
				const dead = pending;
				pending = null;
				if (dead) await dead.close();
			}
		}
	);
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

test('Rate-limited play surfaces the busy-source copy with the advertised wait', async () => {
	// The backend types allanime's in-band throttle answer ("Too many
	// requests, please try again in N seconds") and forwards
	// retry_after_secs; the user-visible contract is that a click
	// during the window names the wait instead of the generic
	// "couldn't start" shrug.
	const { app, page } = await launchAppWithContinueStubs({
		history: continueHistory,
		playRateLimited: true
	});
	try {
		await waitForStripVisible(page);

		const strip = page.getByRole('region', { name: /continue watching/i });
		const card = strip.getByRole('button').first();
		await expect(card).toBeVisible({ timeout: 10_000 });
		// Wait for the resumable form (episode badge) — before
		// onRowReady lands the tile renders as a non-interactive
		// loading card and a click would wait on actionability forever.
		await expect(card).toContainText('6', { timeout: 10_000 });
		await card.click();

		await expect(page.getByText(/busy right now/i)).toBeVisible({ timeout: 10_000 });
		await expect(page.getByText(/about 9 seconds/i)).toBeVisible();
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

test('Episode click takes over a started background prefetch instead of waiting behind it', async () => {
	// The play-cache's click bypass: a detail-page mount warms visible
	// episodes in the background (prefetch=1 on the wire); a click on
	// one of those episodes must NOT wait behind that request — it
	// aborts the shared background fire and issues its own interactive
	// replacement. Acceptance shape: the episode-6 background stream
	// hangs open, the click still resolves promptly via a fresh
	// request without the prefetch flag, and episode 6 sees exactly
	// two stream requests — the takeover pair, no duplicate traffic.
	const streamUrls: string[] = [];
	const ep6 = (prefetch: boolean) =>
		streamUrls
			.map((s) => new URL(s))
			.filter(
				(u) =>
					u.searchParams.get('episode') === '6' &&
					(u.searchParams.get('prefetch') === '1') === prefetch
			).length;
	const { app, page } = await launchAppWithContinueStubs({
		history: continueHistory,
		onPlayStream: (u) => {
			streamUrls.push(u.toString());
			// Pin the episode-6 warm in flight — the takeover target.
			// Only the renderer's own abort releases it; if the click
			// waited behind it, the test would time out instead of
			// navigating. Other episodes' warms resolve instantly.
			if (u.searchParams.get('episode') === '6' && u.searchParams.get('prefetch') === '1') {
				return 'hang';
			}
			return undefined;
		}
	});
	try {
		await waitForStripVisible(page);

		// Into the detail page via the first poster card (top-rated
		// strip — the trending fixtures all feed the hero rotation, so
		// they never render as strip cards).
		const card = page.locator('a.poster-card').first();
		await expect(card).toBeVisible({ timeout: 10_000 });
		await card.click();

		// The mount-time warm fans out over the visible tiles once
		// airing + availability settle. Waiting for the episode-6
		// prefetch on the wire ALSO guarantees its fire has started —
		// the precondition for the click bypass (a queued-not-started
		// entry takes the promote path instead).
		await expect.poll(() => ep6(true), { timeout: 15_000 }).toBe(1);

		// Click the episode the hung warm is holding.
		const tile = page.locator('li[data-ep-num="6"] button.ep-tile');
		await expect(tile).toBeVisible({ timeout: 10_000 });
		await tile.click();

		// Takeover: one fresh interactive request replaces the hung
		// background one — same episode, no prefetch flag, exactly one.
		await expect.poll(() => ep6(false), { timeout: 10_000 }).toBe(1);
		expect(ep6(true)).toBe(1);

		// ...and it, not the hung warm, delivers the session — the app
		// reaches the player page instead of spinning on the overlay.
		await page.waitForURL(/\/play\//, { timeout: 10_000 });
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

/** Height of the single Continue card this stub config produces.
 *  One app launch per call: each state being compared is STABLE
 *  under its own config, which is what makes the comparison
 *  trustworthy. Catching them as a live transition inside one launch
 *  means racing the probe, and that race is not symmetric — once the
 *  app is warm the probe wins, the placeholder is never on screen,
 *  and the measurement silently becomes resolved-vs-resolved. */
async function continueCardHeight(opts: StubOptions, selector: string): Promise<number> {
	const { app, page } = await launchAppWithContinueStubs(opts);
	try {
		await waitForStripVisible(page);
		const strip = page.getByRole('region', { name: /continue watching/i });
		await expect(strip).toBeVisible({ timeout: 10_000 });
		// Class locators, not roles: the strip also holds the rail's
		// kebab and each card's delete chip, so `getByRole('button')`
		// picks up a 33px chip and compares it against a card.
		const card = strip.locator(selector).first();
		await expect(card).toBeVisible({ timeout: 10_000 });
		return (await card.boundingBox())?.height ?? 0;
	} finally {
		await app.close();
	}
}

test('every Continue card is the same height, whatever its probe returns', async () => {
	// The rail resolves row by row, so a card that changes size when
	// its probe lands makes the row settle in visible steps.
	//
	// Stated as "all four shapes are one height" rather than "before
	// equals after". Same claim, but measurable without timing, and it
	// also catches a rail that is ragged with nothing loading at all.
	// The four are every shape the card has: the placeholder, and the
	// three terminal bodies — a title that wraps to the clamp, a title
	// that fits on one line, and a movie with no episode line.
	//
	// Measured rather than counted: the acceptance tier can compare
	// structure but has no layout engine, and the failure here is
	// purely a height. Four Electron launches, hence the timeout.
	test.setTimeout(180_000);
	const LONG = 'The Long Awaited Reunion Beneath the Sakura Tree at the Edge of the World';

	const placeholder = await continueCardHeight(
		{ history: continueHistory, matchHang: true },
		'.resume-card-loading'
	);
	expect(placeholder).toBeGreaterThan(0);

	const wrappingTitle = await continueCardHeight(
		{ history: continueHistory, episodeTitle: LONG },
		'button.resume-card'
	);
	const shortTitle = await continueCardHeight(
		{ history: continueHistory, episodeTitle: 'Six' },
		'button.resume-card'
	);
	// A movie whose episode carries no canonical title renders neither
	// the episode title nor the "Episode N" fallback.
	const movieNoTitle = await continueCardHeight(
		{ history: continueHistory, singleVideo: true, episodeTitle: null },
		'button.resume-card'
	);

	const heights = [placeholder, wrappingTitle, shortTitle, movieNoTitle];
	// Sub-pixel rounding is fine; a line of text is not.
	expect(
		Math.max(...heights) - Math.min(...heights),
		`placeholder=${placeholder} wrapping=${wrappingTitle} short=${shortTitle} movie=${movieNoTitle}`
	).toBeLessThan(2);
});
