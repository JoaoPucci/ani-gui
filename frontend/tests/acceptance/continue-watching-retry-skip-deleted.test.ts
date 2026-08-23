// Acceptance: deleting a Continue card stops the cap retry asking
// about it.
//
// The retry loop outlives the pass that started it — it waits out a
// breaker cooldown before its first ask and climbs a ladder from
// there — so the strip can change underneath it. Deleting a card is
// the change cancellation cannot see: the route stays mounted, so the
// teardown flag never trips, and the queued row goes on spending
// scraper-gate slots on a card that is gone. Worse, a confirmed answer
// for it still reaches `rowReady`, which fetches Kitsu episodes for an
// entry no longer in `historyById` — a map only rebuilt on first load.
//
// `rowWorthRetrying` decides this and is unit-covered. What is not is
// the route feeding it: it has to read LIVE history rather than the
// snapshot the first pass closed over, and nothing below the route can
// observe which of the two it got. Reverting the page to a cap-only
// predicate leaves every other scenario green.

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

/** The card the user deletes mid-retry. */
const DROPPED = 'Cowboy Bebop';
/** The card left alone — the control, so a run where the retry simply
 *  stopped for everyone cannot pass as a run where it skipped one. */
const KEPT = 'Trigun';
const WATCHED = 12;

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

const byLabel = (label: string) =>
	Array.from(target.querySelectorAll('button')).find(
		(b) => b.getAttribute('aria-label') === label
	) ?? null;

const byText = (text: string) =>
	Array.from(target.querySelectorAll('button')).find((b) => b.textContent?.includes(text)) ?? null;

/**
 * Mount home with two history rows whose probes always answer
 * unconfirmed, so both stay in the retry queue for the whole run and
 * the only thing that can take one out of it is the delete.
 */
function mountTwoUnconfirmedRows() {
	/** Every availability probe's title, in order. */
	const probes: string[] = [];
	const deleted: string[] = [];
	server.use(
		...homeHandlers(
			{
				history: [
					{ ep_no: String(WATCHED), id: 'allanime-dropped', title: DROPPED },
					{ ep_no: String(WATCHED), id: 'allanime-kept', title: KEPT }
				]
			},
			[
				http.post(`${API_BASE}/api/kitsu/search`, async ({ request }) => {
					const { query } = (await request.json()) as { query: string };
					return HttpResponse.json(
						query === DROPPED ? [kitsuRef('1', DROPPED, 26)] : [kitsuRef('2', KEPT, 26)]
					);
				}),
				http.post(`${API_BASE}/api/availability`, async ({ request }) => {
					const { title } = (await request.json()) as { title: string };
					probes.push(title);
					// Never confirmed: a row leaves the queue only by
					// running out of attempts or by being skipped, and the
					// window below is well inside its budget.
					return HttpResponse.json({
						available: true,
						episode_count: 27,
						episode_count_approximate: true
					});
				}),
				http.delete(`${API_BASE}/api/history/:id`, ({ params }) => {
					deleted.push(String(params.id));
					return new HttpResponse(null, { status: 204 });
				})
			]
		)
	);
	app = mount(HomePage, { target });
	return { probes, deleted };
}

/** Comfortably past several rungs of the retry's backoff ladder. */
const PAST_COOLDOWN_MS = 90_000;

describe('Continue Watching cap retry after a card is deleted', () => {
	it('stops asking about the deleted row and goes on asking about the rest', async () => {
		const { probes, deleted } = mountTwoUnconfirmedRows();

		// Both rows probed at least once by the first pass, so both are
		// in the retry queue when the delete lands.
		await until(
			() => probes.includes(DROPPED) && probes.includes(KEPT),
			'both rows to be probed by the first pass'
		);

		// Delete one card, the way a user does: the chip on the card,
		// then the confirm the modal asks for.
		const chip = byLabel(m.home_delete_card_aria_label({ title: DROPPED }));
		expect(chip, 'the delete chip should be offered on the card').not.toBeNull();
		chip!.click();
		await until(
			() => byText(m.home_delete_confirm_button()) !== null,
			'the delete confirmation to open'
		);
		byText(m.home_delete_confirm_button())!.click();
		await until(() => deleted.includes('allanime-dropped'), 'the delete to reach the backend');

		// Everything before this point is setup; only what the retry
		// does AFTER the row is gone is the claim.
		probes.length = 0;
		await vi.advanceTimersByTimeAsync(PAST_COOLDOWN_MS);

		expect(probes, 'the deleted row must not be asked about again').not.toContain(DROPPED);
		// The other half of the claim, in the same run: the retry is
		// still going. Without this the scenario passes on a run where
		// it stopped for both rows, which is a different bug wearing
		// the same result.
		expect(probes, 'the surviving row must still be retried').toContain(KEPT);
	});
});
