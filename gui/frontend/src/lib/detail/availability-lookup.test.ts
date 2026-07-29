import { describe, it, expect } from 'vitest';
import { startAvailabilityLookup, type AvailabilityLookupDeps } from './availability-lookup';
import type { AvailabilityPatch } from './availability-writeback';
import type { AvailabilityResponse } from '$lib/api';

const SUBJECT = {
	title: 'Ongoing Show',
	altTitles: ['Ongoing Show JP'],
	episodeCount: 12,
	year: 2019,
	kitsuId: '42',
	status: 'current'
};

const answer = (over: Partial<AvailabilityResponse> = {}): AvailabilityResponse => ({
	available: true,
	episode_count: 4,
	extra_episodes: [],
	episode_count_approximate: false,
	...over
});

/** A lookup whose response the test releases by hand. */
function harness(respond?: () => Promise<AvailabilityResponse>) {
	const applied: AvailabilityPatch[] = [];
	const resolved: boolean[] = [];
	const sent: unknown[] = [];
	const deps: AvailabilityLookupDeps = {
		check: (args) => {
			sent.push(args);
			return respond ? respond() : Promise.resolve(answer());
		},
		// Passes the answer through, so the assertions are about the
		// sequence rather than about the writeback's own rules.
		begin: () => (a) => ({
			available: a.available,
			count: a.count,
			extraEpisodes: a.extraEpisodes
		}),
		apply: (p) => applied.push(p),
		setResolved: (r) => resolved.push(r)
	};
	return { deps, applied, resolved, sent };
}

const settled = () => new Promise((r) => setTimeout(r, 0));

describe('startAvailabilityLookup', () => {
	it('asks about the show in the mode it was given', async () => {
		const h = harness();
		startAvailabilityLookup(SUBJECT, 'dub', h.deps);
		await settled();

		expect(h.sent).toEqual([
			{
				title: 'Ongoing Show',
				mode: 'dub',
				alt_titles: ['Ongoing Show JP'],
				episode_count: 12,
				year: 2019,
				kitsu_id: '42',
				status: 'current'
			}
		]);
	});

	it('marks the question open before asking and answered once it lands', async () => {
		const h = harness();
		startAvailabilityLookup(SUBJECT, 'sub', h.deps);
		expect(h.resolved).toEqual([false]);

		await settled();
		expect(h.resolved).toEqual([false, true]);
	});

	it('writes what the writeback allows', async () => {
		const h = harness(() => Promise.resolve(answer({ episode_count: 9, extra_episodes: ['9.5'] })));
		startAvailabilityLookup(SUBJECT, 'sub', h.deps);
		await settled();

		expect(h.applied).toEqual([{ available: true, count: 9, extraEpisodes: ['9.5'] }]);
	});

	it('establishes nothing when the lookup fails, but stops saying it is open', async () => {
		// A failed lookup is not a verdict — the page keeps whatever it
		// had, and the click handler surfaces the error if the user goes
		// on to ask for something. But the question is over.
		const h = harness(() => Promise.reject(new Error('offline')));
		startAvailabilityLookup(SUBJECT, 'sub', h.deps);
		await settled();

		expect(h.applied).toEqual([]);
		expect(h.resolved).toEqual([false, true]);
	});

	it('says nothing at all once cancelled', async () => {
		// The page it was asked for has moved on — a different show, a
		// different mode, or gone. Reporting the question answered would
		// be as wrong as writing the answer, because the flag belongs to
		// whichever lookup is current now.
		const h = harness();
		const cancel = startAvailabilityLookup(SUBJECT, 'sub', h.deps);
		cancel();
		await settled();

		expect(h.applied).toEqual([]);
		expect(h.resolved).toEqual([false]);
	});

	it('cancels a failed lookup just as quietly', async () => {
		const h = harness(() => Promise.reject(new Error('offline')));
		const cancel = startAvailabilityLookup(SUBJECT, 'sub', h.deps);
		cancel();
		await settled();

		expect(h.resolved).toEqual([false]);
	});

	it('opens its writeback ticket before the request goes out', async () => {
		// The ticket records what was true when the question was asked;
		// opening it after the answer is back would compare the row
		// against itself and never notice a re-ask.
		const order: string[] = [];
		const h = harness();
		const deps: AvailabilityLookupDeps = {
			...h.deps,
			begin: () => {
				order.push('begin');
				return (a) => ({ available: a.available });
			},
			check: (args) => {
				order.push('check');
				return h.deps.check(args);
			}
		};
		startAvailabilityLookup(SUBJECT, 'sub', deps);
		await settled();

		expect(order).toEqual(['begin', 'check']);
	});

	it('carries an absent episode count and status through untouched', async () => {
		const h = harness();
		startAvailabilityLookup({ title: 'Bare', altTitles: [], kitsuId: '7' }, 'sub', h.deps);
		await settled();

		expect(h.sent).toEqual([
			{
				title: 'Bare',
				mode: 'sub',
				alt_titles: [],
				episode_count: undefined,
				year: undefined,
				kitsu_id: '7',
				status: undefined
			}
		]);
	});

	it('does not send a background or cache-skipping flag', async () => {
		// This is the interactive page-load lookup: the scraper gate
		// must not refuse it, and it reads through the cache — the
		// re-ask is the one that skips.
		const h = harness();
		startAvailabilityLookup(SUBJECT, 'sub', h.deps);
		await settled();

		const sent = h.sent[0] as Record<string, unknown>;
		expect(sent.background).toBeUndefined();
		expect(sent.bypass_cache).toBeUndefined();
	});

	it('runs the two lookups of a mode flip independently', async () => {
		const h = harness();
		const cancelFirst = startAvailabilityLookup(SUBJECT, 'sub', h.deps);
		cancelFirst();
		startAvailabilityLookup(SUBJECT, 'dub', h.deps);
		await settled();

		expect(h.sent.map((s) => (s as { mode: string }).mode)).toEqual(['sub', 'dub']);
		// Only the live one wrote, and only it reported the question
		// answered — twice open, once answered.
		expect(h.applied).toHaveLength(1);
		expect(h.resolved).toEqual([false, false, true]);
	});
});

describe('startAvailabilityLookup — cancellation is idempotent', () => {
	it('tolerates being cancelled more than once', async () => {
		const h = harness();
		const cancel = startAvailabilityLookup(SUBJECT, 'sub', h.deps);
		cancel();
		cancel();
		await settled();

		expect(h.resolved).toEqual([false]);
	});
});
