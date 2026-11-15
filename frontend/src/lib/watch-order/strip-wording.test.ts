import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { relationKey } from './strip-wording';

describe('relationKey', () => {
	it('names the link by its Kitsu role in link order', () => {
		expect(relationKey('linked', 'run', 'before', 1)).toEqual({ word: 'prequel', plural: false });
		// Gintama: The Final's two prequels; Fate/Zero 2nd Season's two sequels.
		expect(relationKey('linked', 'run', 'before', 2)).toEqual({ word: 'prequel', plural: true });
		expect(relationKey('linked', 'run', 'after', 2)).toEqual({ word: 'sequel', plural: true });
	});

	it('says "released before/after" in release order, where the neighbours are not linked', () => {
		expect(relationKey('released', 'run', 'before', 1).word).toBe('released_before');
		expect(relationKey('released', 'run', 'after', 1).word).toBe('released_after');
	});

	it("names a side story's parents as parent stories, in either order", () => {
		// Fate/Zero: Onegai! Einzbern Soudanshitsu has two.
		expect(relationKey('linked', 'side', 'before', 2)).toEqual({ word: 'parent', plural: true });
		expect(relationKey('released', 'side', 'before', 1).word).toBe('parent');
		expect(relationKey('linked', 'side', 'after', 0)).toEqual({ word: 'sequel', plural: false });
	});

	it('is plural exactly when the column holds more than one entry', () => {
		const order = fc.constantFrom('linked' as const, 'released' as const);
		const kind = fc.constantFrom('run' as const, 'side' as const);
		const side = fc.constantFrom('before' as const, 'after' as const);
		fc.assert(
			fc.property(order, kind, side, fc.nat(5), (o, k, s, c) => {
				expect(relationKey(o, k, s, c).plural).toBe(c > 1);
			})
		);
	});

	it('words a column by its side alone, whatever the count', () => {
		const order = fc.constantFrom('linked' as const, 'released' as const);
		const kind = fc.constantFrom('run' as const, 'side' as const);
		const side = fc.constantFrom('before' as const, 'after' as const);
		fc.assert(
			fc.property(order, kind, side, fc.nat(5), fc.nat(5), (o, k, s, c1, c2) => {
				expect(relationKey(o, k, s, c1).word).toBe(relationKey(o, k, s, c2).word);
			})
		);
	});
});
