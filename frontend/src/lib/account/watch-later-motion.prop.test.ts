import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { railDelta } from './watch-later-motion';

const row = fc.uniqueArray(fc.constantFrom('a', 'b', 'c', 'd', 'e', 'f', 'g'), { maxLength: 7 });

describe('railDelta, by property', () => {
	it('splits two renders into what arrived, what left and what moved, and nothing else', () => {
		fc.assert(
			fc.property(row, row, (before, after) => {
				const { added, removed, shifted } = railDelta(before, after);
				expect(added).toEqual(after.filter((id) => !before.includes(id)));
				expect(removed).toEqual(before.filter((id) => !after.includes(id)));
				// A card in both moved exactly when its place changed, listed in
				// the order the new render shows it.
				expect(shifted).toEqual(
					after.filter((id) => before.includes(id) && before.indexOf(id) !== after.indexOf(id))
				);
			})
		);
	});

	it('sees no change between a render and itself', () => {
		fc.assert(
			fc.property(row, (ids) => {
				expect(railDelta(ids, ids)).toEqual({ added: [], removed: [], shifted: [] });
			})
		);
	});
});
