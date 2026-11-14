import { describe, expect, it } from 'vitest';
import { staggerSlots } from './stagger';

import { kitsuRefAnswer } from '$lib/testing/wire-answers';
const list = (...ids: string[]) => ids.map((id) => kitsuRefAnswer({ id }));

describe('staggerSlots', () => {
	it('numbers a first batch from zero', () => {
		expect([...staggerSlots(new Map(), list('a', 'b', 'c'))]).toEqual([
			['a', 0],
			['b', 1],
			['c', 2]
		]);
	});

	it('an appended page staggers from its own first card', () => {
		const first = staggerSlots(new Map(), list('a', 'b'));
		const next = staggerSlots(first, list('a', 'b', 'c', 'd'));
		expect(next.get('a')).toBe(0);
		expect(next.get('b')).toBe(1);
		expect(next.get('c')).toBe(0);
		expect(next.get('d')).toBe(1);
	});

	it('drops cards no longer listed, so a returning one rises again', () => {
		const first = staggerSlots(new Map(), list('a', 'b', 'c'));
		const filtered = staggerSlots(first, list('c'));
		expect([...filtered]).toEqual([['c', 2]]);
		expect(staggerSlots(filtered, list('a', 'c')).get('a')).toBe(0);
	});
});
