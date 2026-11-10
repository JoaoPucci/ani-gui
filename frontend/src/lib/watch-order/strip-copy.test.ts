import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { FIXTURE_RUNS } from './fixtures/runs';
import {
	captionText,
	columnCountText,
	columnKeyText,
	emptyColumnText,
	stackPartsText
} from './strip-copy';
import { franchiseSummary, type FranchiseSummary } from './strip-view';
import type { ColumnWord } from './strip-wording';

describe('the strip copy', () => {
	it('words each column key, in the plural only where a role has one', () => {
		const cases: [ColumnWord, boolean, string][] = [
			['prequel', false, 'Prequel'],
			['prequel', true, 'Prequels'],
			['sequel', true, 'Sequels'],
			['parent', false, 'Parent story'],
			['parent', true, 'Parent stories'],
			['released_before', false, 'Released before'],
			['released_after', false, 'Released after']
		];
		for (const [word, plural, text] of cases) {
			expect(columnKeyText({ word, plural })).toBe(text);
		}
	});

	it('says a join or a fork under the key, and nothing for one entry', () => {
		expect(columnCountText('run', 'before', 2, true)).toBe('Comes after these 2');
		expect(columnCountText('run', 'after', 2, true)).toBe('2 ways to continue');
		expect(columnCountText('run', 'after', 1, true)).toBeNull();
		expect(columnCountText('run', 'before', 0, true)).toBeNull();
	});

	it('says no join or fork count for a franchise read in part: the count may be short', () => {
		expect(columnCountText('run', 'before', 2, false)).toBeNull();
		expect(columnCountText('run', 'after', 3, false)).toBeNull();
	});

	it("says nothing under a side story's parents: two parents are not a join", () => {
		expect(columnCountText('side', 'before', 2, true)).toBeNull();
	});

	it('says why a column is empty', () => {
		expect(emptyColumnText('first')).toBe('The story starts here');
		expect(emptyColumnText('last')).toBe('Nothing after this yet');
		expect(emptyColumnText('side_end')).toBe('Nothing after this yet');
	});

	it("reads the strip's caption", () => {
		expect(captionText({ kind: 'position', position: 4, total: 5 })).toBe('04 of 05');
		// Read aloud in the strip's heading, a bare number says nothing.
		expect(captionText({ kind: 'numeral', position: 7 })).toBe('Position 07');
		// A franchise read in part: the heading reads "Watch order" alone.
		expect(captionText({ kind: 'partial' })).toBe('');
		expect(captionText({ kind: 'side' })).toBe('Side story');
		// A loop is said as what the viewer sees: the others it links with.
		expect(captionText({ kind: 'cycle', count: 10 })).toBe('Linked with 9 others, order unclear');
		expect(captionText({ kind: 'cycle', count: 2 })).toBe('Linked with 1 other, order unclear');
		expect(captionText({ kind: 'branch' })).toBe('One of the ways to continue');
	});
});

describe('stackPartsText', () => {
	it("lists JoJo's parts: the main story, films and versions, side stories, recaps", () => {
		const jojo = FIXTURE_RUNS.find((r) => r.key === 'jojo')!;
		expect(stackPartsText(franchiseSummary(jojo))).toEqual([
			'9 in the main story',
			'4 films and versions',
			'1 side story',
			'1 recap'
		]);
	});

	it('leaves out a part the franchise has none of, never the main story', () => {
		const summary: FranchiseSummary = {
			total: 3,
			main: 3,
			films: 0,
			sides: 0,
			recaps: 0,
			posters: []
		};
		expect(stackPartsText(summary)).toEqual(['3 in the main story']);
	});

	it('has one part for the main story and one for each other kind the franchise holds', () => {
		const count = fc.integer({ min: 0, max: 30 });
		fc.assert(
			fc.property(
				fc.integer({ min: 1, max: 30 }),
				count,
				count,
				count,
				(main, films, sides, recaps) => {
					const parts = stackPartsText({
						total: main + films + sides + recaps,
						main,
						films,
						sides,
						recaps,
						posters: []
					});
					expect(parts[0]).toBe(`${main} in the main story`);
					expect(parts).toHaveLength(1 + [films, sides, recaps].filter((n) => n > 0).length);
				}
			)
		);
	});
});
