import { describe, expect, it, vi } from 'vitest';
import type { HistoryEntry } from '$lib/api';
import { STRIP_WARM_MAX, createWarmWatch, stripWarmIds, warmContinueStrip } from './strip-warm';
import type { WarmInFlight, WarmOutcome } from './strip-warm';

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

describe('warmContinueStrip answer', () => {
	it('names the shows it carries, and ends answered or failed', async () => {
		const ok = warmContinueStrip([row('a', '12'), row('b', '21')], {
			warm: () => Promise.resolve()
		});
		expect([...(ok?.ids ?? [])]).toEqual(['12', '21']);
		await expect(ok?.settled).resolves.toBe('answered');
		const down = warmContinueStrip([row('a', '12')], {
			warm: () => Promise.reject(new Error('x'))
		});
		await expect(down?.settled).resolves.toBe('failed');
		expect(warmContinueStrip([row('a')], { warm: () => Promise.resolve() })).toBeNull();
	});
});

describe('createWarmWatch', () => {
	function warmOf(ids: string[]) {
		let end: (o: WarmOutcome) => void = () => {};
		const settled = new Promise<WarmOutcome>((r) => (end = r));
		const warm: WarmInFlight = { ids: new Set(ids), settled };
		return { warm, end };
	}
	const tick = () => new Promise((r) => setTimeout(r, 0));

	it('answers the warm still out for a show it carries', async () => {
		const watch = createWarmWatch();
		const { warm, end } = warmOf(['12', '21']);
		watch.track(warm);
		const pending = watch.pendingFor('12');
		expect(pending).not.toBeNull();
		end('answered');
		await expect(pending).resolves.toBe('answered');
	});

	it('answers none for a show no warm carries', () => {
		const watch = createWarmWatch();
		watch.track(warmOf(['12']).warm);
		expect(watch.pendingFor('99')).toBeNull();
	});

	it('answers none once the warm has ended, answered or failed', async () => {
		for (const outcome of ['answered', 'failed'] as const) {
			const watch = createWarmWatch();
			const { warm, end } = warmOf(['12']);
			watch.track(warm);
			end(outcome);
			await tick();
			expect(watch.pendingFor('12'), outcome).toBeNull();
		}
	});

	it('answers none before any warm, and ignores a strip that sent none', () => {
		const watch = createWarmWatch();
		expect(watch.pendingFor('12')).toBeNull();
		watch.track(null);
		expect(watch.pendingFor('12')).toBeNull();
	});

	it('keeps an earlier warm still out when Home sends another', async () => {
		const watch = createWarmWatch();
		const first = warmOf(['12']);
		const second = warmOf(['21']);
		watch.track(first.warm);
		watch.track(second.warm);
		expect(watch.pendingFor('12')).not.toBeNull();
		expect(watch.pendingFor('21')).not.toBeNull();
		first.end('answered');
		await tick();
		expect(watch.pendingFor('12')).toBeNull();
		expect(watch.pendingFor('21')).not.toBeNull();
	});
});
