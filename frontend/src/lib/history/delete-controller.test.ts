import { describe, expect, test, vi } from 'vitest';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { executeKitsuGroupDelete } from './delete-controller';

function h(id: string): HistoryEntry {
	return {
		id,
		ep_no: '1',
		title: 'Stub',
		watched_at: 1,
		kitsu_id: ''
	} as HistoryEntry;
}
function m(id: string): KitsuAnimeRef {
	return { id, canonical_title: 'Stub' } as KitsuAnimeRef;
}

describe('executeKitsuGroupDelete', () => {
	test('serializes backend deletes — next call starts only after the previous resolves', async () => {
		const callOrder: string[] = [];
		const historyDelete = vi.fn(async (id: string) => {
			callOrder.push(`start:${id}`);
			await new Promise((r) => setTimeout(r, 5));
			callOrder.push(`end:${id}`);
		});
		const history = [h('aa-1'), h('aa-2'), h('aa-3')];
		const matches = { 'aa-1': m('k-1'), 'aa-2': m('k-1'), 'aa-3': m('k-2') };

		await executeKitsuGroupDelete('aa-1', { history, matches, historyDelete });

		// Pin sequential order — aa-1 fully resolves before aa-2 starts.
		// A Promise.all regression would interleave starts (Codex P2
		// #3369156513 root cause).
		const aa1Start = callOrder.indexOf('start:aa-1');
		const aa1End = callOrder.indexOf('end:aa-1');
		const aa2Start = callOrder.indexOf('start:aa-2');
		expect(aa1Start).toBeLessThan(aa1End);
		expect(aa1End).toBeLessThan(aa2Start);
	});

	test('removes every Kitsu-group sibling from history and reports the removed ids', async () => {
		const history = [h('aa-1'), h('aa-2'), h('aa-3')];
		const matches = { 'aa-1': m('k-1'), 'aa-2': m('k-1'), 'aa-3': m('k-2') };
		const historyDelete = vi.fn().mockResolvedValue(undefined);

		const result = await executeKitsuGroupDelete('aa-1', { history, matches, historyDelete });

		expect(result.removedIds.sort()).toEqual(['aa-1', 'aa-2']);
		expect(result.remainingHistory.map((e) => e.id)).toEqual(['aa-3']);
		expect(historyDelete).toHaveBeenCalledTimes(2);
	});

	test('with no resolved match, deletes only the clicked id', async () => {
		const history = [h('aa-1'), h('aa-2')];
		const matches = { 'aa-1': null, 'aa-2': m('k-1') };
		const historyDelete = vi.fn().mockResolvedValue(undefined);

		const result = await executeKitsuGroupDelete('aa-1', { history, matches, historyDelete });

		expect(result.removedIds).toEqual(['aa-1']);
		expect(result.remainingHistory.map((e) => e.id)).toEqual(['aa-2']);
		expect(historyDelete).toHaveBeenCalledTimes(1);
		expect(historyDelete).toHaveBeenCalledWith('aa-1');
	});

	test('singleton group (clicked entry resolves but no siblings share its Kitsu id)', async () => {
		const history = [h('aa-1'), h('aa-2')];
		const matches = { 'aa-1': m('k-1'), 'aa-2': m('k-2') };
		const historyDelete = vi.fn().mockResolvedValue(undefined);

		const result = await executeKitsuGroupDelete('aa-1', { history, matches, historyDelete });

		expect(result.removedIds).toEqual(['aa-1']);
		expect(result.remainingHistory.map((e) => e.id)).toEqual(['aa-2']);
	});
});
