import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { KitsuAnimeRef } from '$lib/api';

const apiMock = vi.hoisted(() => ({
	availabilityBatch: vi.fn(),
	availabilityWarm: vi.fn(),
	checkAvailability: vi.fn(),
	altTitlesFromKitsu: vi.fn((ref: { id: string } | null | undefined) =>
		ref ? [`alt-${ref.id}`] : []
	),
	yearFromKitsuRef: vi.fn((ref: { start_date: string | null } | null | undefined) =>
		ref?.start_date ? Number(ref.start_date.slice(0, 4)) : null
	)
}));
vi.mock('$lib/api', () => apiMock);

import { filterAvailableProgressive } from './progressive';

function ref(id: string, overrides: Partial<KitsuAnimeRef> = {}): KitsuAnimeRef {
	return {
		id,
		canonical_title: `Title ${id}`,
		slug: null,
		synopsis: null,
		start_date: null,
		end_date: null,
		episode_count: null,
		average_rating: null,
		subtype: null,
		status: null,
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null,
		...overrides
	};
}

describe('filterAvailableProgressive (search / render-then-prune)', () => {
	beforeEach(() => {
		apiMock.availabilityBatch.mockReset();
		apiMock.availabilityWarm.mockReset();
		apiMock.checkAvailability.mockReset();
	});
	afterEach(() => vi.useRealTimers());

	/** A checkAvailability mock whose per-item promises resolve only
	 *  when the test says so — models slow upstream probes. */
	function hangingProbes() {
		const resolvers = new Map<string, (r: { available: boolean }) => void>();
		apiMock.checkAvailability.mockImplementation(
			(req: { kitsu_id: string }) =>
				new Promise<{ available: boolean }>((res) => resolvers.set(req.kitsu_id, res))
		);
		return resolvers;
	}

	it('emits the kept list at the grace deadline while probes still hang', async () => {
		vi.useFakeTimers();
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		hangingProbes();
		const emit = vi.fn();
		const items = [ref('a'), ref('b', { status: 'finished' }), ref('c')];
		const done = filterAvailableProgressive(items, 'sub', emit, 2000);
		await vi.advanceTimersByTimeAsync(1999);
		expect(emit).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(1);
		// Unknown verdicts keep their cards — the page renders instead
		// of holding nineteen ready cards hostage to one slow probe.
		expect(emit).toHaveBeenCalledTimes(1);
		expect(emit.mock.calls[0][0].map((r: KitsuAnimeRef) => r.id)).toEqual(['a', 'b', 'c']);
		void done;
	});

	it('prunes a finished show when its late probe reports unavailable', async () => {
		vi.useFakeTimers();
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		const resolvers = hangingProbes();
		const emit = vi.fn();
		const items = [ref('a'), ref('gone', { status: 'finished' })];
		const done = filterAvailableProgressive(items, 'sub', emit, 2000);
		await vi.advanceTimersByTimeAsync(2000);
		expect(emit).toHaveBeenCalledTimes(1);
		resolvers.get('gone')!({ available: false });
		await vi.advanceTimersByTimeAsync(0);
		expect(emit).toHaveBeenCalledTimes(2);
		expect(emit.mock.calls[1][0].map((r: KitsuAnimeRef) => r.id)).toEqual(['a']);
		resolvers.get('a')!({ available: true });
		await vi.advanceTimersByTimeAsync(0);
		await done;
	});

	it('does not re-emit for late verdicts that keep the card', async () => {
		vi.useFakeTimers();
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		const resolvers = hangingProbes();
		const emit = vi.fn();
		const done = filterAvailableProgressive([ref('a'), ref('b')], 'sub', emit, 2000);
		await vi.advanceTimersByTimeAsync(2000);
		expect(emit).toHaveBeenCalledTimes(1);
		resolvers.get('a')!({ available: true });
		await vi.advanceTimersByTimeAsync(0);
		// available:true changes nothing on screen — no churn.
		expect(emit).toHaveBeenCalledTimes(1);
		resolvers.get('b')!({ available: true });
		await vi.advanceTimersByTimeAsync(0);
		await done;
	});

	it('emits once without waiting when every probe settles before the grace', async () => {
		vi.useFakeTimers();
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: { a: true } });
		apiMock.checkAvailability.mockResolvedValue({ available: false });
		const emit = vi.fn();
		const items = [ref('a'), ref('gone', { status: 'finished' })];
		await filterAvailableProgressive(items, 'sub', emit, 2000);
		// The promise resolved with zero timer advancement — warm
		// caches keep the exact single-render behavior of the strict
		// variant, flicker-free.
		expect(emit).toHaveBeenCalledTimes(1);
		expect(emit.mock.calls[0][0].map((r: KitsuAnimeRef) => r.id)).toEqual(['a']);
	});

	it('emits once immediately when everything is already cached', async () => {
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: { a: true, b: false } });
		const emit = vi.fn();
		await filterAvailableProgressive([ref('a'), ref('b', { status: 'finished' })], 'sub', emit);
		expect(emit).toHaveBeenCalledTimes(1);
		expect(emit.mock.calls[0][0].map((r: KitsuAnimeRef) => r.id)).toEqual(['a']);
		expect(apiMock.checkAvailability).not.toHaveBeenCalled();
	});

	it('keeps an item whose probe throws (defer to lazy path)', async () => {
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.checkAvailability.mockRejectedValue(new Error('upstream 520'));
		const emit = vi.fn();
		await filterAvailableProgressive([ref('a', { status: 'finished' })], 'sub', emit);
		expect(emit.mock.calls.at(-1)![0].map((r: KitsuAnimeRef) => r.id)).toEqual(['a']);
	});

	it('emits all items when the batch call itself throws', async () => {
		apiMock.availabilityBatch.mockRejectedValueOnce(new Error('offline'));
		const emit = vi.fn();
		const items = [ref('a'), ref('b')];
		await filterAvailableProgressive(items, 'sub', emit);
		expect(emit).toHaveBeenCalledTimes(1);
		expect(emit.mock.calls[0][0]).toEqual(items);
	});

	it('emits an empty list without hitting the API', async () => {
		const emit = vi.fn();
		await filterAvailableProgressive([], 'sub', emit);
		expect(emit).toHaveBeenCalledWith([]);
		expect(apiMock.availabilityBatch).not.toHaveBeenCalled();
	});

	it('keeps inline probes interactive — the search user is waiting on them', async () => {
		// Routing these through the gate's paced background slots
		// turns a cold ~20-hit search into a ~20-second wait (two
		// admits per hit at 500 ms each). The user is actively
		// waiting, so probes ride the interactive lane like a click.
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.checkAvailability.mockResolvedValue({ available: true });
		await filterAvailableProgressive([ref('a')], 'sub', vi.fn());
		expect(apiMock.checkAvailability).not.toHaveBeenCalledWith(
			expect.objectContaining({ background: true })
		);
	});

	it('probes only uncached items and applies their verdicts', async () => {
		// b is cached false → drops. a is cached true → kept. c is
		// uncached → probed → kept. d is uncached → probed → dropped.
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: { a: true, b: false } });
		apiMock.checkAvailability.mockImplementation(async (args: { kitsu_id: string }) =>
			args.kitsu_id === 'c' ? { available: true } : { available: false }
		);
		const emit = vi.fn();
		await filterAvailableProgressive(
			[ref('a'), ref('b', { status: 'finished' }), ref('c'), ref('d', { status: 'finished' })],
			'sub',
			emit,
			2000,
			2
		);
		expect(emit.mock.calls.at(-1)![0].map((r: KitsuAnimeRef) => r.id)).toEqual(['a', 'c']);
		expect(apiMock.checkAvailability).toHaveBeenCalledTimes(2);
	});

	it('keeps unaired shows visible even when the probe says unavailable', async () => {
		// An upcoming season allmanga hasn't catalogued yet still
		// renders so the user can open and plan it.
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.checkAvailability.mockResolvedValue({ available: false });
		const emit = vi.fn();
		await filterAvailableProgressive(
			[ref('up', { status: 'unreleased' }), ref('gone', { status: 'finished' })],
			'sub',
			emit
		);
		expect(emit.mock.calls.at(-1)![0].map((r: KitsuAnimeRef) => r.id)).toEqual(['up']);
	});

	it('forwards the Kitsu start year to inline probes', async () => {
		// Same symmetry rule as the lazy warm path: the probe must
		// hand the backend picker the year so list-view cards resolve
		// to the same allmanga show as the detail page.
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.checkAvailability.mockResolvedValue({ available: true });
		await filterAvailableProgressive([ref('wing', { start_date: '1995-04-07' })], 'sub', vi.fn());
		expect(apiMock.checkAvailability.mock.calls[0][0]).toMatchObject({
			kitsu_id: 'wing',
			year: 1995
		});
	});
});
