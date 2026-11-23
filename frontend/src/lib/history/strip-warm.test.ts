import { describe, expect, it, vi } from 'vitest';
import type { HistoryEntry } from '$lib/api';
import { STRIP_WARM_MAX, stripWarmIds, warmContinueStrip } from './strip-warm';

function row(id: string, kitsu?: string): HistoryEntry {
	return { id, ep_no: '1', title: id, ...(kitsu === undefined ? {} : { kitsu_id: kitsu }) };
}

describe('stripWarmIds', () => {
	it('takes the Kitsu id each row recorded, in the strip order, once each', () => {
		const history = [row('a', '12'), row('b'), row('c', '21'), row('d', '12'), row('e', '')];
		expect(stripWarmIds(history)).toEqual(['12', '21']);
	});

	it('takes no more than a page of ids', () => {
		const history = Array.from({ length: STRIP_WARM_MAX + 5 }, (_, i) => row(`r${i}`, `${i + 1}`));
		const ids = stripWarmIds(history);
		expect(ids).toHaveLength(STRIP_WARM_MAX);
		expect(ids[0]).toBe('1');
	});
});

describe('warmContinueStrip', () => {
	it('asks the backend once for the whole strip, with the page signal', () => {
		const warm = vi.fn(() => Promise.resolve());
		const signal = new AbortController().signal;
		warmContinueStrip([row('a', '12'), row('b', '21')], { warm, signal });
		expect(warm).toHaveBeenCalledTimes(1);
		expect(warm).toHaveBeenCalledWith(['12', '21'], { signal });
	});

	it('asks nothing for a strip with no recorded ids', () => {
		const warm = vi.fn(() => Promise.resolve());
		warmContinueStrip([row('a')], { warm });
		expect(warm).not.toHaveBeenCalled();
	});

	it('swallows a failed warm: the cards fill on their own', async () => {
		const warm = vi.fn(() => Promise.reject(new Error('down')));
		const unhandled = vi.fn();
		process.on('unhandledRejection', unhandled);
		warmContinueStrip([row('a', '12')], { warm });
		await new Promise((r) => setTimeout(r, 0));
		process.off('unhandledRejection', unhandled);
		expect(unhandled).not.toHaveBeenCalled();
	});
});
