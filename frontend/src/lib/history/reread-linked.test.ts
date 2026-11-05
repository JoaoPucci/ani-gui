// Home's cards read again in place after a link or its undo.

import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import type { HistoryEntry } from '$lib/api';
import { rereadLinked } from './reread-linked';

const row = (id: string, kitsu_id?: string): HistoryEntry => ({
	id,
	title: id,
	ep_no: '1',
	...(kitsu_id ? { kitsu_id } : {})
});

describe('rereadLinked', () => {
	it('matches again the rows whose recorded show changed, in Continue order', async () => {
		const before = [row('a'), row('b'), row('c', '9')];
		const after = [row('a', '5'), row('b'), row('c')];
		const rematch = vi.fn();
		const next = await rereadLinked(before, {
			list: async () => after,
			order: (rows) => [...rows].reverse(),
			rematch
		});
		expect(next.map((h) => h.id)).toEqual(['c', 'b', 'a']);
		expect(rematch.mock.calls.map(([h]) => h.id).sort()).toEqual(['a', 'c']);
	});

	it('matches a row it had not seen', async () => {
		const rematch = vi.fn();
		await rereadLinked([], { list: async () => [row('a')], order: (r) => r, rematch });
		expect(rematch).toHaveBeenCalledWith(row('a'));
	});

	it('matches again exactly the rows whose recorded show is not what it was', async () => {
		const ids = ['a', 'b', 'c', 'd'];
		const kitsu = fc.option(fc.constantFrom('1', '2'), { nil: undefined });
		await fc.assert(
			fc.asyncProperty(
				fc.tuple(...ids.map(() => kitsu)),
				fc.tuple(...ids.map(() => kitsu)),
				async (was, now) => {
					const rematch = vi.fn();
					await rereadLinked(
						ids.map((id, i) => row(id, was[i])),
						{ list: async () => ids.map((id, i) => row(id, now[i])), order: (r) => r, rematch }
					);
					const expected = ids.filter((_, i) => was[i] !== now[i]);
					expect(rematch.mock.calls.map(([h]) => h.id)).toEqual(expected);
				}
			)
		);
	});
});
