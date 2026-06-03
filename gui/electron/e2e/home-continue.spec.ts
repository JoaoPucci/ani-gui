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

	// Generic infra stubs — every test needs these.
	await context.route('**/api/app-info', (r) =>
		r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(appInfo) })
	);
	await context.route('**/api/settings', (r) =>
		r.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify(defaultSettings)
		})
	);
	await context.route('**/api/kitsu/trending-anilist', (r) =>
		r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(trending) })
	);
	await context.route('**/api/kitsu/top-rated', (r) =>
		r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(topRated) })
	);
	// Block image fetches; placeholder takes over.
	await context.route('**/api/image*', (r) => r.fulfill({ status: 503 }));

	// History endpoints — per-test fixture.
	await context.route('**/api/history', (r) =>
		r.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify(opts.history)
		})
	);
	const watchedAt: Record<string, number> = {};
	for (const h of opts.history) watchedAt[h.id] = 1_700_000_000;
	await context.route('**/api/watched-at', (r) =>
		r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(watchedAt) })
	);

	// resolveKitsuMatch step 0: allmanga-kitsu-map short-circuit. The
	// loader hits this for every history entry; only the "Continue
	// Test Show" entry resolves; the orphan id deliberately returns
	// null so the loader falls through to title-match + kitsuSearch.
	await context.route('**/api/allmanga-kitsu-map/**', (r) => {
		const url = r.request().url();
		if (opts.allmangaKitsuMap !== null && url.includes(continueHistory[0].id)) {
			return r.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(opts.allmangaKitsuMap ?? continueKitsuMatch.id)
			});
		}
		return r.fulfill({ status: 200, contentType: 'application/json', body: 'null' });
	});

	// Kitsu detail used by step 0 once allmanga-kitsu-map resolves.
	await context.route('**/api/kitsu/anime/**', (r) =>
		r.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify(continueKitsuMatch)
		})
	);
	// Episode metadata fetched inside onRowReady for the decoration
	// (thumbnail + title). Returning the episode keeps the strip's
	// resume-title element rendered.
	await context.route('**/api/kitsu/episodes/**', (r) =>
		r.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify([continueKitsuEpisode6])
		})
	);

	// Orphan path fallback stubs — title-match + search both return
	// nothing, leaving the row with a null match.
	await context.route('**/api/title-match*', (r) =>
		r.fulfill({ status: 200, contentType: 'application/json', body: 'null' })
	);
	await context.route('**/api/kitsu/search', (r) =>
		r.fulfill({ status: 200, contentType: 'application/json', body: '[]' })
	);

	// Per-row availability probe — the cap signal for pickNextEpisode.
	// Returns the same count as the match's announced episode_count so
	// the at-cap test's pickNextEpisode returns last (replay) and the
	// last+1 test returns ep+1.
	await context.route('**/api/availability', async (r) => {
		if (opts.availabilityDelayMs) {
			await new Promise((resolve) => setTimeout(resolve, opts.availabilityDelayMs));
		}
		return r.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({
				available: true,
				episode_count: continueKitsuMatch.episode_count,
				extra_episodes: []
			})
		});
	});

	// Click handler fires /api/play/stream as an EventSource (SSE) —
	// playStream's preferred path when window.EventSource exists,
	// which it always does in Chromium. The test hook records the
	// resolved query params and we fulfill with a one-shot `done`
	// event carrying a synthetic session so the renderer's overlay
	// resolves cleanly. The trailing route still answers POST
	// /api/play for the EventSource-undefined fallback path.
	await context.route('**/api/play/stream*', async (r) => {
		const url = new URL(r.request().url());
		const body: Record<string, string | null> = {
			title: url.searchParams.get('title'),
			episode: url.searchParams.get('episode'),
			mode: url.searchParams.get('mode'),
			quality: url.searchParams.get('quality')
		};
		opts.onPlay?.(body);
		const session = {
			session_id: 'test-session',
			upstream_url: 'about:blank',
			referer: '',
			subtitle_url: null,
			episode: Number(body.episode),
			media_kind: 'Hls'
		};
		return r.fulfill({
			status: 200,
			contentType: 'text/event-stream',
			body: `event: done\ndata: ${JSON.stringify(session)}\n\n`
		});
	});
	await context.route('**/api/play', async (r) => {
		const body = JSON.parse(r.request().postData() ?? '{}');
		opts.onPlay?.(body);
		return r.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({
				session_id: 'test-session',
				upstream_url: 'about:blank',
				referer: '',
				subtitle_url: null,
				episode: body.episode,
				media_kind: 'Hls'
			})
		});
	});

	const page = await app.firstWindow();
	// `_electron.launch()` already creates the BrowserWindow as part of
	// `app.whenReady()`, so by the time `firstWindow()` resolves the
	// renderer has often already fired its onMount `fetch()` batch
	// (history / settings / trending / top-rated). On warm CI runners
	// those initial requests can land BEFORE the awaits above finish
	// registering routes, so the renderer sees real Kitsu data and the
	// Continue strip stays hidden (history is empty).
	//
	// Wait for any in-flight network to settle before replaying the
	// load: the initial mount fires several /api/* requests; reloading
	// mid-flight surfaces as `page.reload: net::ERR_ABORTED`. Then
	// goto() the same URL to replay the whole boot against the now-
	// registered route table. We use goto() rather than reload() so
	// the navigation goes through Playwright's standard retry path
	// (reload's abort-handling on the `app://` protocol is less
	// forgiving on Xvfb).
	await page.waitForLoadState('networkidle').catch(() => {});
	await page.goto(page.url(), { waitUntil: 'domcontentloaded' });
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
		availabilityDelayMs: 3_000
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
