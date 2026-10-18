import { describe, expect, it } from 'vitest';
import { audioKey, mergeRow, rowFor, UNKNOWN_ROW } from './availability-row';
import { deadEnd } from './reload-offer';

const SUB = audioKey('42', 'sub');

describe('audioKey — the show and audio an answer is about', () => {
	it('names both, and nothing before the show is known', () => {
		expect(audioKey('42', 'sub')).not.toBe(audioKey('42', 'dub'));
		expect(audioKey('42', 'sub')).not.toBe(audioKey('43', 'sub'));
		expect(audioKey(null, 'sub')).toBeNull();
		expect(audioKey(undefined, 'dub')).toBeNull();
		expect(audioKey('', 'dub')).toBeNull();
	});
});

describe('mergeRow — a patch writes only the fields it carries', () => {
	it('reads as unknown before any answer', () => {
		expect(rowFor(null, SUB)).toEqual(UNKNOWN_ROW);
	});

	it('writes a verdict, its refusal and the count', () => {
		const row = mergeRow(null, SUB, {
			available: false,
			refused: true,
			count: null,
			extraEpisodes: []
		});
		expect(rowFor(row, SUB)).toMatchObject({
			listed: false,
			refused: true,
			count: null,
			extraEpisodes: []
		});
	});

	it('keeps what a patch leaves out', () => {
		let row = mergeRow(null, SUB, { available: true, count: 12, extraEpisodes: ['5.5'] });
		row = mergeRow(row, SUB, { count: 13 });
		expect(rowFor(row, SUB)).toMatchObject({
			listed: true,
			refused: false,
			count: 13,
			extraEpisodes: ['5.5']
		});
		// An empty patch — an answer owned elsewhere — changes nothing.
		expect(rowFor(mergeRow(row, SUB, {}), SUB)).toEqual(rowFor(row, SUB));
	});

	it('a refusal travels with its verdict, and a later verdict clears it', () => {
		let row = mergeRow(null, SUB, { available: false, refused: true });
		row = mergeRow(row, SUB, { available: false });
		expect(rowFor(row, SUB).refused).toBe(false);
	});

	it('writes nothing before the show is known', () => {
		const row = mergeRow(null, SUB, { available: true });
		expect(mergeRow(row, null, { available: false })).toBe(row);
	});
});

describe('rowFor — an answer is about the show and audio it was asked for', () => {
	const DUB = audioKey('42', 'dub');
	const OTHER_SHOW = audioKey('43', 'sub');
	const delistedInSub = mergeRow(null, SUB, { available: false, count: null, extraEpisodes: [] });
	const countedInSub = mergeRow(null, SUB, { available: true, count: 8, extraEpisodes: ['5.5'] });

	it('a sub answer says nothing about dub while the dub answer is not in', () => {
		// Pending, failed or unsettled, the dub question wrote nothing;
		// the page knows what it knows before any answer.
		expect(rowFor(delistedInSub, DUB)).toEqual(UNKNOWN_ROW);
		expect(rowFor(countedInSub, DUB)).toEqual(UNKNOWN_ROW);
		// Which refuses nothing ($lib/play/reload-offer).
		expect(
			deadEnd({ listed: rowFor(delistedInSub, DUB).listed, unaired: false, beyondPlayable: false })
		).toBeNull();
	});

	it('a show is not judged by another show’s answer', () => {
		expect(rowFor(delistedInSub, OTHER_SHOW)).toEqual(UNKNOWN_ROW);
	});

	it('a failed or unsettled re-ask in the same audio keeps that audio’s verdict', () => {
		// Neither writes anything, so the verdict stands.
		expect(rowFor(delistedInSub, SUB).listed).toBe(false);
		expect(rowFor(mergeRow(delistedInSub, SUB, {}), SUB).listed).toBe(false);
	});

	it('the first answer in a new audio starts from nothing known', () => {
		// A patch that carries only a count — the verdict owned by a
		// re-ask elsewhere — does not inherit the other audio's verdict.
		const row = mergeRow(delistedInSub, DUB, { count: 6 });
		expect(rowFor(row, DUB)).toMatchObject({
			listed: null,
			refused: false,
			count: 6,
			extraEpisodes: []
		});
		const refusedInSub = mergeRow(null, SUB, { available: false, refused: true });
		expect(rowFor(mergeRow(refusedInSub, DUB, { count: 6 }), DUB).refused).toBe(false);
	});

	it('a settled answer in the new audio is that audio’s verdict', () => {
		const listed = mergeRow(delistedInSub, DUB, { available: true, count: 12, extraEpisodes: [] });
		expect(rowFor(listed, DUB)).toMatchObject({ listed: true, count: 12 });
		const delisted = mergeRow(countedInSub, DUB, { available: false, count: null });
		expect(rowFor(delisted, DUB)).toMatchObject({ listed: false, count: null });
	});
});
