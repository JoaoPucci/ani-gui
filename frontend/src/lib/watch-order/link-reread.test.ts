import { describe, expect, it, vi } from 'vitest';
import { followLink } from './link-reread';
import { latestRead } from './latest-read';
import type { WatchOrderData, WatchOrderSources } from './loader';
import type { FranchiseData } from './types';

const FRANCHISE = {
	anchor: '7',
	entries: [],
	links: [],
	versions: [],
	films: [],
	sides: [],
	recaps: [],
	parents: {},
	complete: true
} as unknown as FranchiseData;
const settle = () => new Promise((r) => setTimeout(r, 0));

function harness(named: Record<string, { kitsu_id: string; ep_no: string } | null>) {
	const shown: (WatchOrderData | null)[] = [];
	const reads = latestRead<WatchOrderData>((d) => shown.push(d));
	const sources: WatchOrderSources = {
		franchise: vi.fn(async () => FRANCHISE),
		named: vi.fn(async () => named)
	};
	return { shown, reads, sources };
}

describe('followLink', () => {
	it('reads the rows again against the franchise shown, keeping it shown meanwhile', async () => {
		const { shown, reads, sources } = harness({ '7': { kitsu_id: '7', ep_no: '3' } });
		followLink(reads, '7', { franchise: FRANCHISE, rows: [] }, sources);
		expect(shown).toEqual([]);
		await settle();
		expect(sources.franchise).not.toHaveBeenCalled();
		expect(shown.at(-1)).toEqual({ franchise: FRANCHISE, rows: [{ kitsu_id: '7', ep_no: '3' }] });
	});

	it('reads the show afresh when nothing is shown yet', async () => {
		const { shown, reads, sources } = harness({});
		followLink(reads, '7', null, sources);
		expect(shown).toEqual([null]);
		await settle();
		expect(sources.franchise).toHaveBeenCalledOnce();
		expect(shown.at(-1)).toEqual({ franchise: FRANCHISE, rows: [] });
	});

	it('reads nothing for a page with no show', () => {
		const { shown, reads, sources } = harness({});
		followLink(reads, '', null, sources);
		expect(shown).toEqual([]);
		expect(sources.franchise).not.toHaveBeenCalled();
	});
});
