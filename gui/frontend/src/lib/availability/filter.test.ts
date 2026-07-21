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

import { filterAvailable, filterAvailableCacheOnly, filterAvailableStrict } from './filter';

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

describe('filterAvailable (lazy / fire-and-forget warm)', () => {
	beforeEach(() => {
		apiMock.availabilityBatch.mockReset();
		apiMock.availabilityWarm.mockReset();
		apiMock.checkAvailability.mockReset();
	});
	afterEach(() => vi.useRealTimers());

	it('returns empty list unchanged without hitting the API', async () => {
		const out = await filterAvailable([], 'sub');
		expect(out).toEqual([]);
		expect(apiMock.availabilityBatch).not.toHaveBeenCalled();
	});

	it('drops cards the cache marks unavailable, keeps cached-true and uncached', async () => {
		const items = [ref('a'), ref('b', { status: 'finished' }), ref('c')];
		apiMock.availabilityBatch.mockResolvedValueOnce({
			cached: { a: true, b: false /* c uncached */ }
		});
		apiMock.availabilityWarm.mockResolvedValueOnce(undefined);
		const out = await filterAvailable(items, 'sub');
		// b drops; a (true) and c (uncached, unknown) survive — the
		// home strip's "render now, prune later" UX requirement.
		expect(out.map((r) => r.id)).toEqual(['a', 'c']);
	});

	it('keeps unaired and airing shows visible even when unavailable', async () => {
		// Upcoming seasons routinely exist on Kitsu before allmanga
		// catalogs them (and airing shows can lag). Hiding them blocks
		// planning; only a FINISHED show missing from the catalog is
		// confidently gone. Play surfaces stay gated separately.
		const items = [
			ref('up', { status: 'unreleased' }),
			ref('air', { status: 'current' }),
			ref('tba', { status: 'tba' }),
			ref('gone', { status: 'finished' })
		];
		apiMock.availabilityBatch.mockResolvedValueOnce({
			cached: { up: false, air: false, tba: false, gone: false }
		});
		apiMock.availabilityWarm.mockResolvedValueOnce(undefined);
		const out = await filterAvailable(items, 'sub');
		expect(out.map((i) => i.id)).toEqual(['up', 'air', 'tba']);
	});

	it('warms only the uncached items and forwards mode + alt titles', async () => {
		const items = [ref('a', { episode_count: 12, status: 'finished' }), ref('b')];
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: { a: true } });
		apiMock.availabilityWarm.mockResolvedValueOnce(undefined);
		await filterAvailable(items, 'dub');
		expect(apiMock.availabilityWarm).toHaveBeenCalledTimes(1);
		const warmArg = apiMock.availabilityWarm.mock.calls[0][0];
		expect(warmArg).toHaveLength(1);
		expect(warmArg[0]).toMatchObject({
			title: 'Title b',
			mode: 'dub',
			alt_titles: ['alt-b'],
			kitsu_id: 'b'
		});
	});

	it('forwards the Kitsu start year to availability warm so the backend picker can use it', async () => {
		// Without year, the backend writes an availability:v4 row that
		// was decided without the year discriminator — leaving the
		// same wrong-show decision cached for the home-strip warm
		// payload. Pin that the warm call forwards year so list-view
		// availability matches what the detail page would resolve.
		const items = [ref('wing', { start_date: '1995-04-07', episode_count: 49 })];
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.availabilityWarm.mockResolvedValueOnce(undefined);
		await filterAvailable(items, 'sub');
		expect(apiMock.availabilityWarm.mock.calls[0][0][0]).toMatchObject({
			kitsu_id: 'wing',
			year: 1995
		});
	});

	it('skips the warm call when nothing is uncached', async () => {
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: { a: true, b: false } });
		await filterAvailable([ref('a'), ref('b')], 'sub');
		expect(apiMock.availabilityWarm).not.toHaveBeenCalled();
	});

	it('falls back to rendering all items when the batch call throws', async () => {
		// Network failure shouldn't blank the home page — the lazy
		// click path will surface real errors when the user actually
		// picks a show.
		apiMock.availabilityBatch.mockRejectedValueOnce(new Error('offline'));
		const items = [ref('a'), ref('b')];
		const out = await filterAvailable(items, 'sub');
		expect(out).toEqual(items);
		expect(apiMock.availabilityWarm).not.toHaveBeenCalled();
	});

	it('ignores warm-call rejections (fire-and-forget contract)', async () => {
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.availabilityWarm.mockRejectedValueOnce(new Error('warm failed'));
		// The function awaits the batch call, then kicks off warm
		// without await. The rejection must not propagate.
		await expect(filterAvailable([ref('a')], 'sub')).resolves.toBeDefined();
	});
});

describe('filterAvailableCacheOnly (high-frequency surfaces)', () => {
	beforeEach(() => {
		apiMock.availabilityBatch.mockReset();
		apiMock.availabilityWarm.mockReset();
		apiMock.checkAvailability.mockReset();
	});

	it('returns empty list unchanged without hitting the API', async () => {
		const out = await filterAvailableCacheOnly([], 'sub');
		expect(out).toEqual([]);
		expect(apiMock.availabilityBatch).not.toHaveBeenCalled();
		expect(apiMock.availabilityWarm).not.toHaveBeenCalled();
	});

	it('drops cards the cache marks unavailable, keeps cached-true and uncached', async () => {
		const items = [ref('a'), ref('b', { status: 'finished' }), ref('c')];
		apiMock.availabilityBatch.mockResolvedValueOnce({
			cached: { a: true, b: false /* c uncached */ }
		});
		const out = await filterAvailableCacheOnly(items, 'sub');
		expect(out.map((r) => r.id)).toEqual(['a', 'c']);
	});

	it('keeps unaired and airing shows visible even when unavailable', async () => {
		apiMock.availabilityBatch.mockResolvedValueOnce({
			cached: { up: false, gone: false }
		});
		const out = await filterAvailableCacheOnly(
			[ref('up', { status: 'unreleased' }), ref('gone', { status: 'finished' })],
			'sub'
		);
		expect(out.map((r) => r.id)).toEqual(['up']);
	});

	it('NEVER calls availabilityWarm — high-frequency surfaces stay cache-only', async () => {
		// The reason this variant exists. The topbar live-search fires
		// once per settled keystroke; warming uncached items on each
		// query enqueues redundant upstream probes for overlapping
		// hits. The cache-only variant reads the cache and stops.
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: { a: true } });
		await filterAvailableCacheOnly([ref('a'), ref('b')], 'sub');
		expect(apiMock.availabilityWarm).not.toHaveBeenCalled();
	});

	it('falls back to rendering all items when the batch call throws', async () => {
		apiMock.availabilityBatch.mockRejectedValueOnce(new Error('offline'));
		const items = [ref('a'), ref('b')];
		const out = await filterAvailableCacheOnly(items, 'sub');
		expect(out).toEqual(items);
		expect(apiMock.availabilityWarm).not.toHaveBeenCalled();
	});
});

describe('filterAvailableStrict (search / inline probe)', () => {
	beforeEach(() => {
		apiMock.availabilityBatch.mockReset();
		apiMock.availabilityWarm.mockReset();
		apiMock.checkAvailability.mockReset();
	});

	it('marks inline probes as background so the scraper gate paces them', async () => {
		// Rail fills are opportunistic; the backend gate paces
		// background probes and skips them while its breaker is open,
		// so a cold cache can't rate-limit the IP before the user's
		// first click.
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.checkAvailability.mockResolvedValue({ available: true });
		await filterAvailableStrict([ref('a')], 'sub');
		expect(apiMock.checkAvailability).toHaveBeenCalledWith(
			expect.objectContaining({ background: true })
		);
	});

	it('inline-probes uncached items and applies their results', async () => {
		// b is cached false → drops. a is cached true → kept. c is
		// uncached → probed inline → kept (probe says available).
		// d is uncached → probed inline → dropped (probe says not).
		apiMock.availabilityBatch.mockResolvedValueOnce({
			cached: { a: true, b: false }
		});
		apiMock.checkAvailability.mockImplementation(async (args) =>
			args.kitsu_id === 'c' ? { available: true } : { available: false }
		);
		const out = await filterAvailableStrict(
			[ref('a'), ref('b', { status: 'finished' }), ref('c'), ref('d', { status: 'finished' })],
			'sub',
			2
		);
		expect(out.map((r) => r.id)).toEqual(['a', 'c']);
		// Two probes, one per uncached id.
		expect(apiMock.checkAvailability).toHaveBeenCalledTimes(2);
	});

	it('keeps unaired and airing shows visible even when the probe says unavailable', async () => {
		// The strict path probes inline; an upcoming season allmanga
		// hasn't catalogued yet still renders so the user can open and
		// plan it.
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.checkAvailability.mockResolvedValue({ available: false });
		const out = await filterAvailableStrict(
			[ref('up', { status: 'unreleased' }), ref('gone', { status: 'finished' })],
			'sub'
		);
		expect(out.map((r) => r.id)).toEqual(['up']);
	});

	it('forwards the Kitsu start year to inline probes', async () => {
		// Same symmetry rule as the lazy warm path: the strict probe
		// must hand the backend picker the year so list-view cards
		// resolve to the same allmanga show as the detail page.
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.checkAvailability.mockResolvedValue({ available: true });
		await filterAvailableStrict([ref('wing', { start_date: '1995-04-07' })], 'sub');
		expect(apiMock.checkAvailability.mock.calls[0][0]).toMatchObject({
			kitsu_id: 'wing',
			year: 1995
		});
	});

	it('keeps an item when its inline probe throws (defer to lazy path)', async () => {
		apiMock.availabilityBatch.mockResolvedValueOnce({ cached: {} });
		apiMock.checkAvailability.mockRejectedValue(new Error('upstream 503'));
		const out = await filterAvailableStrict([ref('a')], 'sub');
		expect(out.map((r) => r.id)).toEqual(['a']);
	});

	it('returns items unchanged when the batch call itself throws', async () => {
		apiMock.availabilityBatch.mockRejectedValueOnce(new Error('offline'));
		const items = [ref('a'), ref('b')];
		const out = await filterAvailableStrict(items, 'sub');
		expect(out).toEqual(items);
	});

	it('returns empty list unchanged without hitting the API', async () => {
		const out = await filterAvailableStrict([], 'sub');
		expect(out).toEqual([]);
		expect(apiMock.availabilityBatch).not.toHaveBeenCalled();
	});
});
