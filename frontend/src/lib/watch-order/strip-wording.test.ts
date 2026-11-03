import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { columnKey, STRIP_WORDINGS } from './strip-wording';

describe('columnKey', () => {
	it('names the link by its Kitsu role in link order', () => {
		expect(columnKey('role', 'linked', 'run', 'before', 1)).toEqual({
			word: 'prequel',
			plural: false
		});
		// Gintama: The Final's two prequels; Fate/Zero 2nd Season's two sequels.
		expect(columnKey('role', 'linked', 'run', 'before', 2)).toEqual({
			word: 'prequel',
			plural: true
		});
		expect(columnKey('role', 'linked', 'run', 'after', 2)).toEqual({
			word: 'sequel',
			plural: true
		});
	});

	it('says "released before/after" in release order, where the neighbours are not linked', () => {
		expect(columnKey('role', 'released', 'run', 'before', 1).word).toBe('released_before');
		expect(columnKey('role', 'released', 'run', 'after', 1).word).toBe('released_after');
	});

	it("names a side story's parents as parent stories, in either order", () => {
		// Fate/Zero: Onegai! Einzbern Soudanshitsu has two.
		expect(columnKey('role', 'linked', 'side', 'before', 2)).toEqual({
			word: 'parent',
			plural: true
		});
		expect(columnKey('role', 'released', 'side', 'before', 1).word).toBe('parent');
		expect(columnKey('role', 'linked', 'side', 'after', 0)).toEqual({
			word: 'sequel',
			plural: false
		});
	});

	it('gives the other wordings their two words', () => {
		expect(columnKey('previous', 'linked', 'run', 'before', 1).word).toBe('previous');
		expect(columnKey('previous', 'linked', 'run', 'after', 1).word).toBe('next');
		expect(columnKey('watch', 'linked', 'run', 'before', 1).word).toBe('watch_before');
		expect(columnKey('watch', 'linked', 'run', 'after', 1).word).toBe('watch_after');
		expect(columnKey('plain', 'linked', 'run', 'before', 1).word).toBe('before');
		expect(columnKey('plain', 'linked', 'run', 'after', 1).word).toBe('after');
	});

	it('pluralises only relation roles, and the other wordings ignore order, kind and count', () => {
		const order = fc.constantFrom('linked' as const, 'released' as const);
		const kind = fc.constantFrom('run' as const, 'side' as const);
		const side = fc.constantFrom('before' as const, 'after' as const);
		fc.assert(
			fc.property(
				fc.constantFrom(...STRIP_WORDINGS),
				order,
				order,
				kind,
				kind,
				side,
				fc.nat(5),
				fc.nat(5),
				(w, o1, o2, k1, k2, s, c1, c2) => {
					const a = columnKey(w, o1, k1, s, c1);
					if (a.plural) expect(w === 'role' && c1 > 1).toBe(true);
					if (w !== 'role') expect(columnKey(w, o2, k2, s, c2)).toEqual(a);
				}
			)
		);
	});
});
