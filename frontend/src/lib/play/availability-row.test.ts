import { describe, expect, it } from 'vitest';
import { audioKey, mergeRow, rowFor, UNKNOWN_ROW } from './availability-row';

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
