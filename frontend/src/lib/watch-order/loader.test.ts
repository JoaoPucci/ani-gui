import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import { loadWatchOrder, type WatchOrderSources } from './loader';
import type { HistoryRow } from './progress';
import type { FranchiseData } from './types';

const FRANCHISE: FranchiseData = {
	anchor: '7',
	entries: [],
	links: [],
	versions: [],
	films: [],
	sides: [],
	recaps: [],
	parents: {},
	complete: true
};
const ROWS: HistoryRow[] = [{ kitsu_id: '7', ep_no: '3' }];

const sources = (franchise: boolean, history: boolean): WatchOrderSources => ({
	franchise: () =>
		franchise ? Promise.resolve(FRANCHISE) : Promise.reject(new Error('kitsu unreachable')),
	history: () => (history ? Promise.resolve(ROWS) : Promise.reject(new Error('no backend')))
});

describe('loadWatchOrder', () => {
	it('hands on the franchise and the history rows', async () => {
		expect(await loadWatchOrder('7', sources(true, true))).toEqual({
			franchise: FRANCHISE,
			rows: ROWS
		});
	});

	it('asks for the franchise of the entry the page shows', async () => {
		const franchise = vi.fn(() => Promise.resolve(FRANCHISE));
		await loadWatchOrder('42', { franchise, history: () => Promise.resolve([]) });
		expect(franchise).toHaveBeenCalledWith('42');
	});

	it('reads a franchise that cannot be read as none: no strip, and no error', async () => {
		expect(await loadWatchOrder('7', sources(false, true))).toEqual({
			franchise: null,
			rows: ROWS
		});
	});

	it('reads a history that cannot be read as empty: a strip without progress', async () => {
		expect(await loadWatchOrder('7', sources(true, false))).toEqual({
			franchise: FRANCHISE,
			rows: []
		});
	});

	it('never rejects, whichever read fails', async () => {
		await fc.assert(
			fc.asyncProperty(fc.boolean(), fc.boolean(), async (f, h) => {
				const got = await loadWatchOrder('7', sources(f, h));
				expect(got.franchise).toBe(f ? FRANCHISE : null);
				expect(got.rows).toEqual(h ? ROWS : []);
			})
		);
	});
});
