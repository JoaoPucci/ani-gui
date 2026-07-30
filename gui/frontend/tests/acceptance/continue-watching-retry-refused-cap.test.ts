// Acceptance: a Continue card whose availability probe the scraper
// gate refused corrects itself once the breaker recovers.
//
// A refused probe answers with the allmanga search hit's count, which
// counts half-episodes as whole ones and so runs high. The card then
// offers an episode past the end. The backend refuses to serve that
// count back from cache so it self-heals on the next read, and the
// route now re-probes after a breaker cooldown to BE that next read.
//
// Everything about the correction lives on the route — the timer, the
// mode the re-probe carries, the publication path back into the card,
// and the teardown that stops it — so none of it is observable below
// the route.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { m } from '../../src/lib/paraglide/messages';

const SHOW = 'Cowboy Bebop';
const WATCHED = 12;
/** What the refused probe reports: one high, the half-episode error. */
const REFUSED_CAP = 13;
/** What the detail fetch reports once the gate admits again. */
const TRUE_CAP = 12;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	// The retry waits out a breaker cooldown before its first ask, so
	// the scenario drives the clock rather than sitting out a minute.
	vi.useFakeTimers();
	__resetApiBaseForTests(API_BASE);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	vi.useRealTimers();
});

/**
 * Poll under fake timers. `Date.now()` and `setTimeout` are both
 * faked, so the wall-clock version of this would never advance;
 * stepping the clock 10 ms at a time also flushes the microtask queue
 * the loader's promise chain runs on.
 */
async function until(predicate: () => boolean, what: string, steps = 400) {
	for (let i = 0; i < steps; i++) {
		if (predicate()) return;
		await vi.advanceTimersByTimeAsync(10);
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

const carrying = (el: Element | null) => el?.textContent?.includes(SHOW) ?? false;
const resumeButton = () => Array.from(target.querySelectorAll('button')).find(carrying) ?? null;
const offers = (episode: number) =>
	resumeButton()?.textContent?.includes(m.home_resume_episode_label({ episode })) ?? false;

/**
 * Mount home with one history row whose availability probe answers
 * unconfirmed the first time and confirmed every time after. Returns
 * the recorded probe bodies so a scenario can count them and read the
 * mode each carried.
 */
function mountRefusedRow() {
	const probes: { mode?: string }[] = [];
	server.use(
		...homeHandlers({ history: [{ ep_no: String(WATCHED), id: 'allanime-1', title: SHOW }] }, [
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([kitsuRef('1', SHOW, 26)])),
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				probes.push((await request.json()) as { mode?: string });
				return HttpResponse.json(
					probes.length === 1
						? { available: true, episode_count: REFUSED_CAP, episode_count_approximate: true }
						: { available: true, episode_count: TRUE_CAP, episode_count_approximate: false }
				);
			})
		])
	);
	app = mount(HomePage, { target });
	return probes;
}

/** Comfortably past the breaker cooldown the retry waits out. */
const PAST_COOLDOWN_MS = 90_000;

describe('Continue Watching card after a gate-refused probe', () => {
	it('corrects the episode it offers once the re-probe confirms the cap', async () => {
		const probes = mountRefusedRow();

		// The unconfirmed cap is one high, so the card offers an
		// episode past the end of the show — the actual defect.
		await until(() => offers(REFUSED_CAP), `the card to offer episode ${REFUSED_CAP}`);
		expect(probes).toHaveLength(1);

		await vi.advanceTimersByTimeAsync(PAST_COOLDOWN_MS);

		await until(
			() => offers(TRUE_CAP),
			`the card to correct itself to episode ${TRUE_CAP} after the re-probe`
		);
		expect(offers(REFUSED_CAP)).toBe(false);
		expect(probes.length).toBeGreaterThan(1);
	});

	it('re-probes under the configured mode', async () => {
		const probes = mountRefusedRow();
		await until(() => offers(REFUSED_CAP), 'the card to render its unconfirmed cap');
		await vi.advanceTimersByTimeAsync(PAST_COOLDOWN_MS);
		await until(() => probes.length > 1, 'the re-probe to be answered');

		// A retry that lost the mode would read a playable count for a
		// track the user is not watching, and the correction would be
		// as wrong as what it replaced.
		expect(probes.every((p) => p.mode === 'sub')).toBe(true);
	});

	it('stops re-probing once the route is torn down', async () => {
		const probes = mountRefusedRow();
		await until(() => offers(REFUSED_CAP), 'the card to render its unconfirmed cap');

		// Leave before the cooldown elapses, as a user who lands on
		// home and immediately navigates on does.
		unmount(app!);
		app = null;
		await vi.advanceTimersByTimeAsync(PAST_COOLDOWN_MS * 4);

		// The loop outlives the first pass by design; without a
		// teardown it would keep asking allmanga about a strip nobody
		// is looking at.
		expect(probes).toHaveLength(1);
	});

	it('survives a run of refusals and still corrects the card', async () => {
		// The route-level shape of the thing the helper cases pin: a
		// refusal is not an answer, so it must not spend the row's
		// budget — and it must still back off, or the retry hammers the
		// very pacer that is refusing it. Three refusals here is more
		// than the answer budget, so with refusals counting the row
		// would be dropped before the exact answer arrives.
		const probes: { mode?: string }[] = [];
		let refusalsLeft = 3;
		server.use(
			...homeHandlers({ history: [{ ep_no: String(WATCHED), id: 'allanime-1', title: SHOW }] }, [
				http.post(`${API_BASE}/api/kitsu/search`, () =>
					HttpResponse.json([kitsuRef('1', SHOW, 26)])
				),
				http.post(`${API_BASE}/api/availability`, async ({ request }) => {
					probes.push((await request.json()) as { mode?: string });
					if (refusalsLeft > 0) {
						refusalsLeft--;
						return HttpResponse.json({
							available: true,
							episode_count: REFUSED_CAP,
							episode_count_approximate: true,
							gate_refused: true
						});
					}
					return HttpResponse.json({
						available: true,
						episode_count: TRUE_CAP,
						episode_count_approximate: false
					});
				})
			])
		);
		app = mount(HomePage, { target });

		await until(() => offers(REFUSED_CAP), `the card to offer episode ${REFUSED_CAP}`);

		// Generous enough to clear the whole ladder several times over,
		// which is also what proves the retry is climbing it rather
		// than spinning at the first rung.
		await vi.advanceTimersByTimeAsync(PAST_COOLDOWN_MS * 8);

		await until(
			() => offers(TRUE_CAP),
			`the card to correct itself to episode ${TRUE_CAP} after the refusals cleared`
		);
		expect(refusalsLeft).toBe(0);
	});
});
