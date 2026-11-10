import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import { loadWatchOrder, type WatchOrderSources } from './loader';
import type { HistoryRow } from './progress';
import type { FranchiseData, WatchEntry } from './types';

const entry = (id: string): WatchEntry => ({
	id,
	title: `Entry ${id}`,
	subtype: 'TV',
	start: '2020-01-01',
	episodes: 12,
	status: 'finished',
	poster: null,
	cover: null
});

const FRANCHISE: FranchiseData = {
	anchor: '7',
	entries: [entry('7'), entry('8')],
	links: [['7', '8']],
	versions: [entry('20')],
	films: [entry('30')],
	sides: [entry('40')],
	recaps: [entry('50')],
	parents: {},
	complete: true
};
const NAMED: Record<string, HistoryRow | null> = { '7': { kitsu_id: '7', ep_no: '3' }, '8': null };
const ROWS: HistoryRow[] = [{ kitsu_id: '7', ep_no: '3' }];

const sources = (franchise: boolean, history: boolean): WatchOrderSources => ({
	franchise: () =>
		franchise ? Promise.resolve(FRANCHISE) : Promise.reject(new Error('kitsu unreachable')),
	named: () => (history ? Promise.resolve(NAMED) : Promise.reject(new Error('no backend')))
});

describe('loadWatchOrder', () => {
	it('hands on the franchise and the rows its entries resume from', async () => {
		expect(await loadWatchOrder('7', sources(true, true))).toEqual({
			franchise: FRANCHISE,
			rows: ROWS
		});
	});

	it('asks for the franchise of the entry the page shows', async () => {
		const franchise = vi.fn((id: string) => {
			void id;
			return Promise.resolve(FRANCHISE);
		});
		await loadWatchOrder('42', { franchise, named: () => Promise.resolve({}) });
		expect(franchise).toHaveBeenCalledTimes(1);
		expect(franchise.mock.calls[0][0]).toBe('42');
	});

	it('asks for the row of every entry the franchise names, once each', async () => {
		const named = vi.fn((ids: readonly string[]) => {
			void ids;
			return Promise.resolve({});
		});
		await loadWatchOrder('7', { franchise: () => Promise.resolve(FRANCHISE), named });
		expect(named).toHaveBeenCalledTimes(1);
		expect([...named.mock.calls[0][0]].sort()).toEqual(['20', '30', '40', '50', '7', '8']);
	});

	it('counts a row toward the entry that named it, whatever id the row recorded', async () => {
		// The row recorded entry 4, which Kitsu has since deleted; the
		// backend names it for 8, its replacement (history_claim::names).
		const got = await loadWatchOrder('7', {
			franchise: () => Promise.resolve(FRANCHISE),
			named: () => Promise.resolve({ '8': { kitsu_id: '4', ep_no: '5' } })
		});
		expect(got.rows).toEqual([{ kitsu_id: '8', ep_no: '5' }]);
	});

	it('reads a franchise that cannot be read as none, and asks for no rows', async () => {
		const named = vi.fn(() => Promise.resolve(NAMED));
		expect(
			await loadWatchOrder('7', {
				franchise: () => Promise.reject(new Error('kitsu unreachable')),
				named
			})
		).toEqual({ franchise: null, rows: [] });
		expect(named).not.toHaveBeenCalled();
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
				expect(got.rows).toEqual(f && h ? ROWS : []);
			})
		);
	});

	it('gives each named entry its row and nothing else', async () => {
		const ids = ['7', '8', '20', '30', '40', '50'];
		const answer = fc.dictionary(
			fc.constantFrom(...ids),
			fc.option(
				fc.record(
					{
						kitsu_id: fc.constantFrom('4', '7', '99'),
						ep_no: fc.integer({ min: 1, max: 30 }).map(String)
					},
					{ requiredKeys: ['ep_no'] }
				),
				{ nil: null }
			)
		);
		await fc.assert(
			fc.asyncProperty(answer, async (named) => {
				const got = await loadWatchOrder('7', {
					franchise: () => Promise.resolve(FRANCHISE),
					named: () => Promise.resolve(named)
				});
				const want = Object.entries(named).flatMap(([id, r]) =>
					r ? [{ kitsu_id: id, ep_no: r.ep_no }] : []
				);
				expect(got.rows).toEqual(want);
			})
		);
	});
});
