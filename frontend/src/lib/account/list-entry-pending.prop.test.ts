import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { pendingAfterSave, seedForOpen, statusWriteIntended } from './list-entry-pending';
import type { ListStatus } from './types';
import { effectiveProgress } from './list-entry-edit';

const status = fc.constantFrom<ListStatus>(
	'planning',
	'watching',
	'completed',
	'paused',
	'dropped',
	'rewatching'
);
const ids = fc.constantFrom('A', 'B');
const pending = fc.option(
	fc.record({ kitsuId: ids, intendedStatus: status, intendedProgress: fc.nat(24) }),
	{ nil: null }
);
const outcome = fc.constantFrom(
	'noop' as const,
	'saved' as const,
	'partial' as const,
	'failed' as const
);

describe('pendingAfterSave, by property', () => {
	it('clears on a clean save, records a status partial, and otherwise keeps what it had', () => {
		fc.assert(
			fc.property(
				pending,
				ids,
				outcome,
				fc.boolean(),
				status,
				fc.nat(24),
				(prev, id, o, changed, s, p) => {
					const next = pendingAfterSave(prev, id, o, changed, s, p);
					if (o === 'saved') expect(next).toBeNull();
					else if (o === 'partial' && changed)
						expect(next).toEqual({ kitsuId: id, intendedStatus: s, intendedProgress: p });
					else expect(next).toBe(prev);
				}
			)
		);
	});
});

describe('seedForOpen, by property', () => {
	const view = fc.record({
		onList: fc.boolean(),
		status: fc.option(status, { nil: null }),
		progress: fc.nat(24),
		total: fc.option(fc.integer({ min: 1, max: 24 }), { nil: null })
	});

	it("seeds the status from the show's own pending retry, and the seed status from the view", () => {
		fc.assert(
			fc.property(view, pending, ids, (v, p, id) => {
				const seed = seedForOpen(v, p, id);
				const mine = p !== null && p.kitsuId === id;
				expect(seed.status).toBe(mine ? p.intendedStatus : (v.status ?? 'planning'));
				expect(seed.seededStatus).toBe(v.status ?? 'planning');
				const base = v.status ?? 'planning';
				const raw = mine ? p.intendedProgress : effectiveProgress(base, v.progress, v.total);
				expect(seed.progress).toBe(effectiveProgress(seed.status, raw, v.total));
			})
		);
	});
});

describe('statusWriteIntended, by property', () => {
	it('is a status write while a retry is pending, or when the pick moved off the seed', () => {
		fc.assert(
			fc.property(fc.boolean(), status, status, (active, picked, seeded) => {
				expect(statusWriteIntended(active, picked, seeded)).toBe(active || picked !== seeded);
			})
		);
	});
});
