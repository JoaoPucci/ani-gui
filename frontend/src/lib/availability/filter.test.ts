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

import { filterAvailable, filterAvailableCacheOnly } from './filter';

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
		// Upcoming seasons routinely exist on Kitsu before the provider
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
