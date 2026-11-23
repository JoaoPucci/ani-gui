// The pick a player mounted again finds its line keeps.

import { describe, expect, it } from 'vitest';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { keptOnArrival, KeptPicks } from './kept-pick';

const ref = (id: string) => ({ id, canonical_title: `Show ${id}` }) as KitsuAnimeRef;
const LINE: HistoryEntry = { id: 'hianime:jojo-2000-5182', title: 'JoJo', ep_no: '2' };

// Whether the line keeps a pick is the backend's verdict
// (commands/link_verdict.rs), found with the arrival's offer
// ($lib/history/picked-offer.ts): the player takes it only where it
// applies.
describe('keptOnArrival', () => {
	const at = { inPlayer: true, row: LINE.id };
	const found = { offer: null, unread: false, kept: ref('4013') };

	it('is the pick the line keeps, on the player, for an arrival that made no offer', () => {
		expect(keptOnArrival(at, found)?.id).toBe('4013');
		expect(keptOnArrival(at, { ...found, kept: null })).toBeNull();
	});

	it('is taken beside the offer to change it again that the player makes for it', () => {
		const offer = { entry: LINE, current: ref('7158'), saved: '4013' };
		expect(keptOnArrival(at, { ...found, offer })?.id).toBe('4013');
	});

	it('is none on Home, with no row, or after a failed read', () => {
		expect(keptOnArrival({ ...at, inPlayer: false }, found)).toBeNull();
		expect(keptOnArrival({ ...at, row: null }, found)).toBeNull();
		expect(keptOnArrival(at, { ...found, unread: true })).toBeNull();
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

	it("is not cleared or replaced by an earlier arrival's read landing late", () => {
		const kept = new KeptPicks();
		kept.note(3, ref('4013'));
		kept.note(2, null);
		kept.note(2, ref('9999'));
		expect(kept.take(3)?.id).toBe('4013');
	});

	it('holds nothing for an arrival that found none', () => {
		const kept = new KeptPicks();
		kept.note(1, ref('4013'));
		kept.note(2, null);
		expect(kept.take(1)).toBeNull();
	});
});
