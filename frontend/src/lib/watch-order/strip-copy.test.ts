import { describe, expect, it } from 'vitest';
import { columnCountText, columnKeyText, emptyColumnText } from './strip-copy';
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
			['released_after', false, 'Released after'],
			['previous', false, 'Previous'],
			['next', false, 'Next'],
			['watch_before', false, 'Watch before'],
			['watch_after', false, 'Watch after'],
			['before', false, 'Before'],
			['after', false, 'After']
		];
		for (const [word, plural, text] of cases) {
			expect(columnKeyText({ word, plural })).toBe(text);
		}
	});

	it('says a join or a fork under the key, and nothing for one entry', () => {
		expect(columnCountText('run', 'before', 2)).toBe('Comes after these 2');
		expect(columnCountText('run', 'after', 2)).toBe('2 ways to continue');
		expect(columnCountText('run', 'after', 1)).toBeNull();
		expect(columnCountText('run', 'before', 0)).toBeNull();
	});

	it("says nothing under a side story's parents: two parents are not a join", () => {
		expect(columnCountText('side', 'before', 2)).toBeNull();
	});

	it('says why a column is empty', () => {
		expect(emptyColumnText('first')).toBe('Start of the run');
		expect(emptyColumnText('last')).toBe('End of the run');
		expect(emptyColumnText('side_end')).toBe('Nothing links on from here');
	});
});
