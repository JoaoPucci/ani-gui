// The pick a player mounted again finds its line keeps.

import { describe, expect, it } from 'vitest';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import type { LinkDeps } from './link-offer';
import { keptOnArrival, keptPick, KeptPicks } from './kept-pick';

const ref = (id: string) => ({ id, canonical_title: `Show ${id}` }) as KitsuAnimeRef;
const LINE: HistoryEntry = { id: 'hianime:jojo-2000-5182', title: 'JoJo', ep_no: '2' };

function deps(rows: HistoryEntry[], fail = false): LinkDeps {
	return {
		history: async () => {
			if (fail) throw new Error('busy');
			return rows;
		},
		resolve: async (e) => ({ match: e.kitsu_id ? ref(e.kitsu_id) : null, trusted: true }),
		search: async () => [],
		link: async () => ({ kitsu_id: '', linked: [] }),
		group: async (_c, r) => r
	};
}

describe('keptPick', () => {
	it('is the show the line was named for, when another show plays', async () => {
		const d = deps([{ ...LINE, kitsu_id: '4013', named: true }]);
		expect((await keptPick(d, LINE.id, '7158'))?.id).toBe('4013');
	});

	it('is none while the show named is the one playing, for a line no one named, or gone', async () => {
		expect(
			await keptPick(deps([{ ...LINE, kitsu_id: '4013', named: true }]), LINE.id, '4013')
		).toBeNull();
		expect(await keptPick(deps([{ ...LINE, kitsu_id: '4013' }]), LINE.id, '7158')).toBeNull();
		expect(await keptPick(deps([]), LINE.id, '7158')).toBeNull();
	});

	it('is none when the history cannot be read', async () => {
		expect(await keptPick(deps([], true), LINE.id, '7158')).toBeNull();
	});
});

describe('keptOnArrival', () => {
	const d = deps([{ ...LINE, kitsu_id: '4013', named: true }]);
	const at = { inPlayer: true, row: LINE.id, pageId: '7158' };
	const none = { offer: null, unread: false };

	it('reads it on the player, for an arrival that made no offer', async () => {
		expect((await keptOnArrival(d, at, none))?.id).toBe('4013');
	});

	it('reads nothing on Home, with no row, past an offer, or after a failed read', async () => {
		expect(await keptOnArrival(d, { ...at, inPlayer: false }, none)).toBeNull();
		expect(await keptOnArrival(d, { ...at, row: null }, none)).toBeNull();
		const offer = { entry: LINE, current: ref('7158') };
		expect(await keptOnArrival(d, at, { offer, unread: false })).toBeNull();
		expect(await keptOnArrival(d, at, { offer: null, unread: true })).toBeNull();
	});
});

describe('KeptPicks', () => {
	it('hands a pick only to the arrival that read it, once', () => {
		const kept = new KeptPicks();
		kept.note(2, ref('4013'));
		expect(kept.take(3)).toBeNull();
		kept.note(2, ref('4013'));
		expect(kept.take(2)).toEqual({ id: '4013', title: 'Show 4013', show: ref('4013') });
		expect(kept.take(2)).toBeNull();
	});

	it('holds nothing for an arrival that found none', () => {
		const kept = new KeptPicks();
		kept.note(1, ref('4013'));
		kept.note(2, null);
		expect(kept.take(1)).toBeNull();
	});
});
