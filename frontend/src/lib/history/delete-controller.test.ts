import { describe, expect, test, vi } from 'vitest';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { executeKitsuGroupDelete } from './delete-controller';
import {
	clearRowPositions,
	readPosition,
	savePosition,
	type PositionStorage
} from '$lib/play/watch-position';

function memory(): PositionStorage {
	const data = new Map<string, string>();
	return { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) };
}

function h(id: string): HistoryEntry {
	return {
		id,
		ep_no: '1',
		title: 'Stub',
		watched_at: 1,
		kitsu_id: ''
	} as HistoryEntry;
}
/** A delete, and the forgetting of positions it leaves running. */
async function deleteAndForget(...args: Parameters<typeof executeKitsuGroupDelete>) {
	const result = await executeKitsuGroupDelete(...args);
	await result.forgetting;
	return result;
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

		await deleteAndForget('aa-1', { history, matches, historyDelete });

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

		const result = await deleteAndForget('aa-1', { history, matches, historyDelete });

		expect(result.removedIds.sort()).toEqual(['aa-1', 'aa-2']);
		expect(result.remainingHistory.map((e) => e.id)).toEqual(['aa-3']);
		expect(historyDelete).toHaveBeenCalledTimes(2);
	});

	test('with no resolved match, deletes only the clicked id', async () => {
		const history = [h('aa-1'), h('aa-2')];
		const matches = { 'aa-1': null, 'aa-2': m('k-1') };
		const historyDelete = vi.fn().mockResolvedValue(undefined);

		const result = await deleteAndForget('aa-1', { history, matches, historyDelete });

		expect(result.removedIds).toEqual(['aa-1']);
		expect(result.remainingHistory.map((e) => e.id)).toEqual(['aa-2']);
		expect(historyDelete).toHaveBeenCalledTimes(1);
		expect(historyDelete).toHaveBeenCalledWith('aa-1');
	});

	test('singleton group (clicked entry resolves but no siblings share its Kitsu id)', async () => {
		const history = [h('aa-1'), h('aa-2')];
		const matches = { 'aa-1': m('k-1'), 'aa-2': m('k-2') };
		const historyDelete = vi.fn().mockResolvedValue(undefined);

		const result = await deleteAndForget('aa-1', { history, matches, historyDelete });

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

		await deleteAndForget('aa-1', { history, matches, historyDelete, forgetPositions });

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

		await deleteAndForget('aa-1', {
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
		await deleteAndForget('aa-1', {
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
		const result = await deleteAndForget('aa-1', {
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
		await deleteAndForget('aa-1', {
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
		await deleteAndForget('aa-1', {
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
		await deleteAndForget('aa-1', {
			history: [h('aa-1'), h('bb-1'), h('cc-1')],
			matches: { 'aa-1': undefined, 'bb-1': m('k-2'), 'cc-1': undefined },
			historyDelete: vi.fn().mockResolvedValue(undefined),
			forgetPositions,
			kitsuIdOf: async (id) => (id === 'aa-1' ? 'k-9' : id === 'cc-1' ? 'k-3' : null)
		});
		expect(forgetPositions).toHaveBeenCalledWith('k-9');
	});

	test("a remaining unresolved row with no stamped mapping keeps the removed show's positions", async () => {
		// No mapping is not a different show: the row may well be this
		// one's, so its show cannot be told.
		const forgetPositions = vi.fn();
		await deleteAndForget('aa-1', {
			history: [h('aa-1'), h('bb-1')],
			matches: { 'aa-1': m('k-9'), 'bb-1': undefined },
			historyDelete: vi.fn().mockResolvedValue(undefined),
			forgetPositions,
			kitsuIdOf: async () => null
		});
		expect(forgetPositions).not.toHaveBeenCalled();
	});

	test("a remaining row whose show cannot be told keeps the removed show's positions", async () => {
		const forgetPositions = vi.fn();
		await deleteAndForget('aa-1', {
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

describe('executeKitsuGroupDelete — the show a row records', () => {
	// A row records the Kitsu id of the show played, and that id names
	// the row's show unless Kitsu answers it gone; then the row is the
	// entry it is shown as, else its stamped mapping. Positions are
	// keyed by the id the play page had, so a removed row's recorded id
	// is one of them, whatever became of the entry since.
	function rec(id: string, kitsuId: string): HistoryEntry {
		return { ...h(id), kitsu_id: kitsuId };
	}
	const deleteOk = () => vi.fn().mockResolvedValue(undefined);

	test('reads a removed row’s mapping before the delete takes it', async () => {
		// Removing a row deletes its show id → Kitsu id mapping with it,
		// so a read after the delete finds nothing.
		const mappings: Record<string, string> = { 'aa-1': 'k-9' };
		const forgetPositions = vi.fn();
		await deleteAndForget('aa-1', {
			history: [h('aa-1')],
			matches: { 'aa-1': undefined },
			historyDelete: vi.fn(async (id: string) => {
				delete mappings[id];
			}),
			forgetPositions,
			kitsuIdOf: async (id) => mappings[id] ?? null
		});
		expect(forgetPositions).toHaveBeenCalledWith('k-9');
	});

	test('forgets a removed row’s recorded id though its card never resolved', async () => {
		const forgetPositions = vi.fn();
		await deleteAndForget('aa-1', {
			history: [rec('aa-1', 'k-5')],
			matches: { 'aa-1': undefined },
			historyDelete: deleteOk(),
			forgetPositions,
			kitsuIdOf: async () => null
		});
		expect(forgetPositions).toHaveBeenCalledWith('k-5');
	});

	test('forgets a gone recorded id and the entry the card was shown as', async () => {
		const forgetPositions = vi.fn();
		await deleteAndForget('aa-1', {
			history: [rec('aa-1', 'k-gone')],
			matches: { 'aa-1': m('k-6') },
			historyDelete: deleteOk(),
			forgetPositions
		});
		expect(forgetPositions.mock.calls.map(([k]) => k).sort()).toEqual(['k-6', 'k-gone']);
	});

	test('a remaining row is told by its recorded id, not a stale mapping', async () => {
		const forgetPositions = vi.fn();
		await deleteAndForget('aa-1', {
			history: [h('aa-1'), rec('bb-1', 'k-8')],
			matches: { 'aa-1': m('k-7'), 'bb-1': undefined },
			historyDelete: deleteOk(),
			forgetPositions,
			kitsuIdOf: async (id) => (id === 'bb-1' ? 'k-7' : null),
			recordedGone: async () => false
		});
		expect(forgetPositions).toHaveBeenCalledWith('k-7');
	});

	test('a remaining row with a recorded id needs no mapping to be told', async () => {
		const forgetPositions = vi.fn();
		await deleteAndForget('aa-1', {
			history: [h('aa-1'), rec('bb-1', 'k-8')],
			matches: { 'aa-1': m('k-7'), 'bb-1': undefined },
			historyDelete: deleteOk(),
			forgetPositions,
			kitsuIdOf: async () => null,
			recordedGone: async () => false
		});
		expect(forgetPositions).toHaveBeenCalledWith('k-7');
	});

	test('a remaining row of the same show keeps its positions by the id it records', async () => {
		const forgetPositions = vi.fn();
		await deleteAndForget('aa-1', {
			history: [rec('aa-1', 'k-1'), rec('bb-1', 'k-1')],
			matches: { 'aa-1': m('k-1'), 'bb-1': undefined },
			historyDelete: deleteOk(),
			forgetPositions,
			kitsuIdOf: async () => null,
			recordedGone: async () => false
		});
		expect(forgetPositions).not.toHaveBeenCalled();
	});

	test('a remaining row whose recorded entry is gone is told by its mapping', async () => {
		const forgetPositions = vi.fn();
		await deleteAndForget('aa-1', {
			history: [h('aa-1'), rec('bb-1', 'k-gone')],
			matches: { 'aa-1': m('k-7'), 'bb-1': undefined },
			historyDelete: deleteOk(),
			forgetPositions,
			kitsuIdOf: async (id) => (id === 'bb-1' ? 'k-7' : null),
			recordedGone: async (k) => k === 'k-gone'
		});
		expect(forgetPositions).not.toHaveBeenCalled();
	});

	test('a remaining row shown as another entry is that entry, without asking Kitsu', async () => {
		// The home page goes past a recorded id only when Kitsu answers
		// it gone, so a match other than the recorded id is the entry
		// the card shows.
		const recordedGone = vi.fn(async () => false);
		const forgetPositions = vi.fn();
		await deleteAndForget('aa-1', {
			history: [h('aa-1'), rec('bb-1', 'k-gone')],
			matches: { 'aa-1': undefined, 'bb-1': m('k-7') },
			historyDelete: deleteOk(),
			forgetPositions,
			kitsuIdOf: async (id) => (id === 'aa-1' ? 'k-7' : null),
			recordedGone
		});
		expect(forgetPositions).not.toHaveBeenCalled();
		expect(recordedGone).not.toHaveBeenCalled();
	});

	test('a remaining row whose recorded id cannot be judged keeps the removed show’s positions', async () => {
		const forgetPositions = vi.fn();
		await deleteAndForget('aa-1', {
			history: [h('aa-1'), rec('bb-1', 'k-8')],
			matches: { 'aa-1': m('k-7'), 'bb-1': undefined },
			historyDelete: deleteOk(),
			forgetPositions,
			kitsuIdOf: async (id) => (id === 'bb-1' ? 'k-8' : null),
			recordedGone: vi.fn().mockRejectedValue(new Error('down'))
		});
		expect(forgetPositions).not.toHaveBeenCalled();
	});
});

describe('executeKitsuGroupDelete — a slow Kitsu does not hold the removal', () => {
	// Telling an unresolved remaining row's show can take a Kitsu read,
	// and the card's removal must not wait on it: the delete settles
	// with the remaining history, and the positions follow.
	const never = () => new Promise<boolean>(() => {});
	const tick = () => new Promise((r) => setTimeout(r, 20));

	test('settles with the remaining history while a Kitsu read hangs', async () => {
		const forgetPositions = vi.fn();
		const outcome = await Promise.race([
			executeKitsuGroupDelete('aa-1', {
				history: [h('aa-1'), { ...h('bb-1'), kitsu_id: 'k-8' }],
				matches: { 'aa-1': m('k-7'), 'bb-1': undefined },
				historyDelete: vi.fn().mockResolvedValue(undefined),
				forgetPositions,
				recordedGone: never
			}),
			tick().then(() => 'held' as const)
		]);
		expect(outcome).not.toBe('held');
		if (outcome !== 'held') expect(outcome.remainingHistory.map((e) => e.id)).toEqual(['bb-1']);
		expect(forgetPositions).not.toHaveBeenCalled();
	});

	test('asks about every remaining row at once', async () => {
		const asked: string[] = [];
		const answers: Array<(gone: boolean) => void> = [];
		const forgetPositions = vi.fn();
		const result = await Promise.race([
			executeKitsuGroupDelete('aa-1', {
				history: [h('aa-1'), { ...h('bb-1'), kitsu_id: 'k-8' }, { ...h('cc-1'), kitsu_id: 'k-9' }],
				matches: { 'aa-1': m('k-7'), 'bb-1': undefined, 'cc-1': undefined },
				historyDelete: vi.fn().mockResolvedValue(undefined),
				forgetPositions,
				recordedGone: (k) => {
					asked.push(k);
					return new Promise<boolean>((r) => answers.push(r));
				}
			}),
			tick().then(() => null)
		]);
		await tick();
		expect(asked.sort()).toEqual(['k-8', 'k-9']);
		for (const answer of answers) answer(false);
		await result?.forgetting;
		expect(forgetPositions).toHaveBeenCalledWith('k-7');
	});
});

describe('executeKitsuGroupDelete — forgetting that fails', () => {
	test('a forget that throws leaves the removal settled and fails nothing', async () => {
		const result = await executeKitsuGroupDelete('aa-1', {
			history: [h('aa-1')],
			matches: { 'aa-1': m('k-1') },
			historyDelete: vi.fn().mockResolvedValue(undefined),
			forgetPositions: () => {
				throw new Error('storage refused');
			}
		});
		await expect(result.forgetting).resolves.toBeUndefined();
		expect(result.removedIds).toEqual(['aa-1']);
	});
});

// A session a Continue card opened keeps its positions for the card's
// row. The card's match may have been a guess a later load corrected,
// so the shows the card names now need not include the one those
// positions are under; the row still reaches them.
describe('executeKitsuGroupDelete — positions written for the removed rows', () => {
	test("forgets every removed row's positions once the rows are gone", async () => {
		const history = [h('aa-1'), h('aa-2'), h('aa-3')];
		const matches = { 'aa-1': m('k-1'), 'aa-2': m('k-1'), 'aa-3': m('k-2') };
		const order: string[] = [];
		const historyDelete = vi.fn(async (id: string) => {
			order.push(`delete:${id}`);
		});
		const forgetRowPositions = vi.fn((rows: string[]) => {
			order.push(`forget-rows:${rows.join(',')}`);
		});

		await deleteAndForget('aa-1', { history, matches, historyDelete, forgetRowPositions });

		expect(order).toEqual(['delete:aa-1', 'delete:aa-2', 'forget-rows:aa-1,aa-2']);
	});

	test("forgets none of them while a remaining row's show cannot be told", async () => {
		// Untold, the remaining row may be a card of the show a removed
		// row's position is under — its play may have landed on that
		// row — so every position stays, as the show rule keeps them.
		const history = [h('aa-1'), h('aa-2')];
		const matches = { 'aa-1': m('k-1') };
		const forgetPositions = vi.fn();
		const forgetRowPositions = vi.fn();

		await deleteAndForget('aa-1', {
			history,
			matches,
			historyDelete: vi.fn().mockResolvedValue(undefined),
			forgetPositions,
			forgetRowPositions
		});

		expect(forgetPositions).not.toHaveBeenCalled();
		expect(forgetRowPositions).not.toHaveBeenCalled();
	});

	test('a guess later corrected leaves no position behind', async () => {
		// The card was a guess of k-1 when it played, and is k-2 now; no
		// remaining row is a card of k-1.
		const s = memory();
		savePosition('k-1', 1, 600, 1420, s, 'aa-1');
		const history = [h('aa-1'), h('aa-2')];
		const matches = { 'aa-1': m('k-2'), 'aa-2': m('k-3') };

		await deleteAndForget('aa-1', {
			history,
			matches,
			historyDelete: vi.fn().mockResolvedValue(undefined),
			forgetRowPositions: (rows, keep) => clearRowPositions(rows, s, keep)
		});

		expect(readPosition('k-1', 1, s)).toBeNull();
	});

	test('keeps a position whose show a remaining card is', async () => {
		// The guessed card's play landed on another provider show, whose
		// row is a card of the show played: its position is that card's.
		const s = memory();
		savePosition('k-1', 1, 600, 1420, s, 'aa-1');
		const history = [h('aa-1'), h('aa-9')];
		const matches = { 'aa-1': m('k-2'), 'aa-9': m('k-1') };

		await deleteAndForget('aa-1', {
			history,
			matches,
			historyDelete: vi.fn().mockResolvedValue(undefined),
			forgetRowPositions: (rows, keep) => clearRowPositions(rows, s, keep)
		});

		expect(readPosition('k-1', 1, s)).toBe(600);
	});

	test('a failed delete forgets nothing', async () => {
		const forgetRowPositions = vi.fn();

		await expect(
			executeKitsuGroupDelete('aa-1', {
				history: [h('aa-1')],
				matches: { 'aa-1': m('k-1') },
				historyDelete: vi.fn().mockRejectedValue(new Error('down')),
				forgetRowPositions
			})
		).rejects.toThrow('down');
		expect(forgetRowPositions).not.toHaveBeenCalled();
	});
});
