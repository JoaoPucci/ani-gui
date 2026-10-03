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

describe('executeKitsuGroupDelete — kept positions', () => {
	test("forgets the removed show's kept positions once its rows are gone", async () => {
		const history = [h('aa-1'), h('aa-2'), h('aa-3')];
		const matches = { 'aa-1': m('k-1'), 'aa-2': m('k-1'), 'aa-3': m('k-2') };
		const order: string[] = [];
		const historyDelete = vi.fn(async (id: string) => {
			order.push(`delete:${id}`);
		});
		const forgetPositions = vi.fn((kitsuId: string) => {
			order.push(`forget:${kitsuId}`);
		});

		await executeKitsuGroupDelete('aa-1', { history, matches, historyDelete, forgetPositions });

		expect(order).toEqual(['delete:aa-1', 'delete:aa-2', 'forget:k-1']);
	});

	test('a failed delete forgets nothing', async () => {
		const history = [h('aa-1')];
		const matches = { 'aa-1': m('k-1') };
		const historyDelete = vi.fn().mockRejectedValue(new Error('down'));
		const forgetPositions = vi.fn();

		await expect(
			executeKitsuGroupDelete('aa-1', { history, matches, historyDelete, forgetPositions })
		).rejects.toThrow('down');
		expect(forgetPositions).not.toHaveBeenCalled();
	});
});

describe('executeKitsuGroupDelete — kept positions of a card without its match', () => {
	// Positions are keyed by the Kitsu id the play page had, and a play
	// stamps the row's show id → Kitsu id mapping. A card deleted before
	// its match resolved still has that mapping to forget by.
	test('forgets by the stamped mapping when the match never resolved', async () => {
		const history = [h('aa-1')];
		const historyDelete = vi.fn().mockResolvedValue(undefined);
		const forgetPositions = vi.fn();
		const kitsuIdOf = vi.fn(async (showId: string) => (showId === 'aa-1' ? 'k-9' : null));

		await executeKitsuGroupDelete('aa-1', {
			history,
			matches: { 'aa-1': undefined },
			historyDelete,
			forgetPositions,
			kitsuIdOf
		});

		expect(forgetPositions).toHaveBeenCalledWith('k-9');
	});

	test('asks for no mapping when the match resolved', async () => {
		const kitsuIdOf = vi.fn();
		const forgetPositions = vi.fn();
		await executeKitsuGroupDelete('aa-1', {
			history: [h('aa-1')],
			matches: { 'aa-1': m('k-1') },
			historyDelete: vi.fn().mockResolvedValue(undefined),
			forgetPositions,
			kitsuIdOf
		});
		expect(kitsuIdOf).not.toHaveBeenCalled();
		expect(forgetPositions).toHaveBeenCalledWith('k-1');
	});

	test('a mapping that cannot be read forgets nothing and fails nothing', async () => {
		const forgetPositions = vi.fn();
		const result = await executeKitsuGroupDelete('aa-1', {
			history: [h('aa-1')],
			matches: { 'aa-1': null },
			historyDelete: vi.fn().mockResolvedValue(undefined),
			forgetPositions,
			kitsuIdOf: vi.fn().mockRejectedValue(new Error('down'))
		});
		expect(forgetPositions).not.toHaveBeenCalled();
		expect(result.removedIds).toEqual(['aa-1']);
	});
});

describe('executeKitsuGroupDelete — a show that still has a row', () => {
	// Positions belong to the show, and a surviving row of the same show
	// is still a Continue card whose resume point they are.
	test('an unresolved card forgets nothing while a resolved row of its show remains', async () => {
		const forgetPositions = vi.fn();
		await executeKitsuGroupDelete('aa-1', {
			history: [h('aa-1'), h('bb-1')],
			matches: { 'aa-1': undefined, 'bb-1': m('k-9') },
			historyDelete: vi.fn().mockResolvedValue(undefined),
			forgetPositions,
			kitsuIdOf: async (id) => (id === 'aa-1' ? 'k-9' : null)
		});
		expect(forgetPositions).not.toHaveBeenCalled();
	});

	test('a resolved card forgets nothing while an unresolved row maps to its show', async () => {
		const forgetPositions = vi.fn();
		await executeKitsuGroupDelete('aa-1', {
			history: [h('aa-1'), h('bb-1')],
			matches: { 'aa-1': m('k-9'), 'bb-1': undefined },
			historyDelete: vi.fn().mockResolvedValue(undefined),
			forgetPositions,
			kitsuIdOf: async (id) => (id === 'bb-1' ? 'k-9' : null)
		});
		expect(forgetPositions).not.toHaveBeenCalled();
	});

	test("a remaining row of another show does not keep this show's positions", async () => {
		const forgetPositions = vi.fn();
		await executeKitsuGroupDelete('aa-1', {
			history: [h('aa-1'), h('bb-1'), h('cc-1')],
			matches: { 'aa-1': undefined, 'bb-1': m('k-2'), 'cc-1': undefined },
			historyDelete: vi.fn().mockResolvedValue(undefined),
			forgetPositions,
			kitsuIdOf: async (id) => (id === 'aa-1' ? 'k-9' : id === 'cc-1' ? 'k-3' : null)
		});
		expect(forgetPositions).toHaveBeenCalledWith('k-9');
	});

	test("a remaining row whose show cannot be told keeps the removed show's positions", async () => {
		const forgetPositions = vi.fn();
		await executeKitsuGroupDelete('aa-1', {
			history: [h('aa-1'), h('bb-1')],
			matches: { 'aa-1': m('k-9'), 'bb-1': undefined },
			historyDelete: vi.fn().mockResolvedValue(undefined),
			forgetPositions,
			kitsuIdOf: async (id) => {
				if (id === 'bb-1') throw new Error('down');
				return null;
			}
		});
		expect(forgetPositions).not.toHaveBeenCalled();
	});
});
