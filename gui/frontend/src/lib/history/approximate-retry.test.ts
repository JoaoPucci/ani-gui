import { describe, it, expect, vi } from 'vitest';

import {
	createApproximateCollector,
	retryApproximateCaps,
	type ApproximateRow
} from './approximate-retry';
import type { KitsuAnimeRef } from '$lib/api';

function ref(id: string): KitsuAnimeRef {
	return {
		id,
		canonical_title: `Show ${id}`,
		titles: {},
		abbreviated_titles: [],
		slug: `show-${id}`,
		synopsis: null,
		poster_image: null,
		cover_image: null,
		episode_count: 26,
		status: 'finished',
		start_date: '2019-01-01',
		end_date: null,
		subtype: 'TV',
		age_rating: null,
		popularity_rank: null,
		average_rating: null
	};
}

const row = (id: string): ApproximateRow => ({ entryId: id, match: ref(id) });

/** A `wait` that records what it was asked for and returns instantly. */
function fakeClock() {
	const waited: number[] = [];
	return {
		waited,
		wait: async (ms: number) => {
			waited.push(ms);
		}
	};
}

/** Availability answers, in order, keyed by the number of times a row
 *  has been probed. Anything past the end repeats the last entry. */
function answers(...seq: ({ count: number | null; approximate: boolean } | 'reject')[]) {
	const probes: string[] = [];
	const modes: string[] = [];
	const per = new Map<string, number>();
	const fetchAvailability = vi.fn(async (match: KitsuAnimeRef, mode: 'sub' | 'dub') => {
		probes.push(match.id);
		modes.push(mode);
		const n = per.get(match.id) ?? 0;
		per.set(match.id, n + 1);
		const a = seq[Math.min(n, seq.length - 1)];
		if (a === 'reject') throw new Error('probe failed');
		return { episode_count: a.count, episode_count_approximate: a.approximate };
	});
	return { probes, modes, fetchAvailability };
}

const exact = (count: number) => ({ count, approximate: false });
const stillApproximate = (count: number) => ({ count, approximate: true });

describe('retryApproximateCaps', () => {
	it('climbs a fixed ladder rather than reading the gate off the answers', async () => {
		const clock = fakeClock();
		const { fetchAvailability, probes } = answers(stillApproximate(13));

		await retryApproximateCaps([row('a')], {
			fetchAvailability,
			mode: 'sub',
			onRefined: () => {},
			wait: clock.wait,
			backoffMs: [500, 30_000, 60_000]
		});

		// The first step is short because the overwhelmingly common
		// cause is a detail fetch that failed once with the gate
		// admitting throughout — the backend sets the same unconfirmed
		// flag for that as for a refusal, and the breaker only opens on
		// the third consecutive failure. The later steps carry the row
		// past the 60 s breaker cooldown for the case where it really
		// was refused. Neither step claims to know which happened.
		expect(clock.waited).toEqual([500, 30_000, 60_000]);
		expect(probes).toEqual(['a', 'a', 'a']);
	});

	it('uses the shipped ladder, and enough of it to outlast a breaker cooldown', async () => {
		const clock = fakeClock();
		const { fetchAvailability } = answers(stillApproximate(13));

		await retryApproximateCaps([row('a')], {
			fetchAvailability,
			mode: 'sub',
			onRefined: () => {},
			wait: clock.wait
		});

		// Pins the DEFAULT, which is what ships — the cases below hand
		// in their own ladder and would keep passing if this one were
		// flattened. The last step matters most: the backend's
		// BREAKER_COOLDOWN is 60 s, so a ladder that never reaches past
		// it leaves a genuinely gate-refused row without a single
		// attempt the gate would admit.
		expect(clock.waited).toEqual([500, 30_000, 60_000]);
	});

	it('gives every row the same first step regardless of what came before', async () => {
		const clock = fakeClock();
		let n = 0;
		const fetchAvailability = async () => {
			n++;
			return n === 1
				? { episode_count: 13, episode_count_approximate: true }
				: { episode_count: 12, episode_count_approximate: false };
		};

		await retryApproximateCaps([row('a'), row('b'), row('c')], {
			fetchAvailability,
			mode: 'sub',
			onRefined: () => {},
			wait: clock.wait,
			backoffMs: [500, 30_000, 60_000],
			maxAttempts: 1
		});

		// Row a's unconfirmed answer must not push row b onto a longer
		// step, and row b's confirmed one must not shorten row c's:
		// neither answer is evidence about the gate, so a row's wait
		// depends only on its own attempt count.
		expect(clock.waited).toEqual([500, 500, 500]);
	});

	it('repeats the last step once the ladder runs out', async () => {
		const clock = fakeClock();
		const { fetchAvailability } = answers(stillApproximate(13));

		await retryApproximateCaps([row('a')], {
			fetchAvailability,
			mode: 'sub',
			onRefined: () => {},
			wait: clock.wait,
			backoffMs: [500, 30_000],
			maxAttempts: 4
		});

		expect(clock.waited).toEqual([500, 30_000, 30_000, 30_000]);
	});

	it('publishes a cap only once the retry comes back exact', async () => {
		const clock = fakeClock();
		const { fetchAvailability } = answers(exact(12));
		const onRefined = vi.fn();

		await retryApproximateCaps([row('a')], {
			fetchAvailability,
			mode: 'sub',
			onRefined,
			wait: clock.wait
		});

		expect(onRefined).toHaveBeenCalledTimes(1);
		expect(onRefined).toHaveBeenCalledWith('a', 12, false);
	});

	it('does not republish a cap that came back approximate again', async () => {
		const clock = fakeClock();
		const { fetchAvailability } = answers(stillApproximate(13));
		const onRefined = vi.fn();

		await retryApproximateCaps([row('a')], {
			fetchAvailability,
			mode: 'sub',
			onRefined,
			wait: clock.wait,
			maxAttempts: 2
		});

		// Publishing an equally-unconfirmed number would churn the card
		// without making it any more trustworthy. The retry only ever
		// tightens an unconfirmed cap into a confirmed one.
		expect(onRefined).not.toHaveBeenCalled();
	});

	it('probes one row at a time', async () => {
		const clock = fakeClock();
		let inFlight = 0;
		let peak = 0;
		const fetchAvailability = vi.fn(async () => {
			inFlight++;
			peak = Math.max(peak, inFlight);
			await Promise.resolve();
			inFlight--;
			return { episode_count: 12, episode_count_approximate: false };
		});

		await retryApproximateCaps([row('a'), row('b'), row('c')], {
			fetchAvailability,
			mode: 'sub',
			onRefined: () => {},
			wait: clock.wait
		});

		// The breaker admits exactly one half-open trial per cooldown.
		// Firing every approximate row at once spends that single slot
		// on one of them and gets the rest refused all over again.
		expect(peak).toBe(1);
		expect(fetchAvailability).toHaveBeenCalledTimes(3);
	});

	it('gives up on a row after the attempt budget', async () => {
		const clock = fakeClock();
		const { fetchAvailability, probes } = answers(stillApproximate(13));

		await retryApproximateCaps([row('a')], {
			fetchAvailability,
			mode: 'sub',
			onRefined: () => {},
			wait: clock.wait,
			maxAttempts: 3
		});

		// An upstream that stays refused is not going to be talked round
		// by a fourth ask; the click path revalidates anyway.
		expect(probes).toEqual(['a', 'a', 'a']);
	});

	it('keeps retrying after a probe rejects, and publishes nothing', async () => {
		const clock = fakeClock();
		const onRefined = vi.fn();
		const { fetchAvailability, probes } = answers('reject', exact(12));

		await retryApproximateCaps([row('a')], {
			fetchAvailability,
			mode: 'sub',
			onRefined,
			wait: clock.wait,
			maxAttempts: 3
		});

		expect(probes).toEqual(['a', 'a']);
		expect(onRefined).toHaveBeenCalledWith('a', 12, false);
	});

	it('stops as soon as it is cancelled', async () => {
		const clock = fakeClock();
		let cancelled = false;
		const { fetchAvailability, probes } = answers(exact(12));

		await retryApproximateCaps([row('a'), row('b'), row('c')], {
			fetchAvailability: async (m, mode) => {
				cancelled = true;
				return fetchAvailability(m, mode);
			},
			mode: 'sub',
			onRefined: () => {},
			wait: clock.wait,
			cancelled: () => cancelled
		});

		// The home route mounts and unmounts freely; a retry loop that
		// outlives its page keeps probing allmanga for a strip nobody
		// is looking at.
		expect(probes).toEqual(['a']);
	});

	it('skips a row whose cap a click already settled', async () => {
		const clock = fakeClock();
		const { fetchAvailability, probes } = answers(exact(12));

		await retryApproximateCaps([row('a'), row('b')], {
			fetchAvailability,
			mode: 'sub',
			onRefined: () => {},
			wait: clock.wait,
			shouldRetry: (id) => id !== 'a'
		});

		// A click runs its own interactive lookup, which the gate never
		// refuses. That answer is already exact, so re-probing the row
		// spends a scraper slot to learn nothing.
		expect(probes).toEqual(['b']);
	});

	it('probes under the configured mode', async () => {
		const clock = fakeClock();
		const { fetchAvailability, modes } = answers(exact(12));

		await retryApproximateCaps([row('a')], {
			fetchAvailability,
			mode: 'dub',
			onRefined: () => {},
			wait: clock.wait
		});

		// A dub user probed under 'sub' reads a playable count for a
		// track they are not watching — the same mismatch the loader's
		// getMode gate exists to prevent.
		expect(modes).toEqual(['dub']);
	});

	it('does nothing at all when no row came back approximate', async () => {
		const clock = fakeClock();
		const fetchAvailability = vi.fn();

		await retryApproximateCaps([], {
			fetchAvailability,
			mode: 'sub',
			onRefined: () => {},
			wait: clock.wait
		});

		expect(fetchAvailability).not.toHaveBeenCalled();
		expect(clock.waited).toEqual([]);
	});

	it('treats a countless answer as no answer', async () => {
		const clock = fakeClock();
		const onRefined = vi.fn();
		const { fetchAvailability, probes } = answers({ count: null, approximate: false });

		await retryApproximateCaps([row('a')], {
			fetchAvailability,
			mode: 'sub',
			onRefined,
			wait: clock.wait,
			maxAttempts: 2
		});

		// `approximate: false` with no count is not a confirmed cap — it
		// is the absence of one, and publishing it would overwrite the
		// card's match.episode_count fallback with nothing.
		expect(onRefined).not.toHaveBeenCalled();
		expect(probes).toEqual(['a', 'a']);
	});
});

describe('createApproximateCollector', () => {
	it('keeps only the rows whose count arrived unconfirmed', () => {
		const c = createApproximateCollector();
		const a = ref('a');
		const b = ref('b');

		c.record('hist-a', a, true);
		c.record('hist-b', b, false);

		expect(c.rows).toEqual([{ entryId: 'hist-a', match: a }]);
	});

	it('starts empty and stays empty when nothing was refused', () => {
		const c = createApproximateCollector();
		expect(c.rows).toEqual([]);
		c.record('hist-a', ref('a'), false);
		expect(c.rows).toEqual([]);
	});
});
