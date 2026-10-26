import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { editorToggle, settleRemove, settleSave } from './editor-settle';
import {
	runEditorRemove,
	runEditorSave,
	type RemoveResult,
	type SaveResult
} from './editor-actions';

const live = fc.record({
	status: fc.constantFrom('watching' as const, 'paused' as const),
	progress: fc.integer({ min: 0, max: 24 })
});
const saveResult: fc.Arbitrary<SaveResult> = fc.oneof(
	fc.constant({ kind: 'noop' as const }),
	fc.constant({ kind: 'superseded' as const }),
	fc.record({ kind: fc.constant('saved' as const), live }),
	fc.record({ kind: fc.constant('partial' as const), live }),
	fc.record({ kind: fc.constant('failed' as const), rateLimited: fc.boolean() })
);
const removeResult: fc.Arbitrary<RemoveResult> = fc.oneof(
	fc.constantFrom(
		{ kind: 'noop' as const },
		{ kind: 'removed' as const },
		{ kind: 'superseded' as const }
	),
	fc.record({ kind: fc.constant('failed' as const), rateLimited: fc.boolean() })
);

describe('settling a result, by property', () => {
	it('takes a result into the editor only on the same show, and only one that landed', () => {
		fc.assert(
			fc.property(saveResult, fc.boolean(), (res, sameShow) => {
				const settled = settleSave(res, sameShow);
				expect(settled.local).toBe(sameShow && (res.kind === 'saved' || res.kind === 'partial'));
			})
		);
		fc.assert(
			fc.property(removeResult, fc.boolean(), (res, sameShow) => {
				expect(settleRemove(res, sameShow).local).toBe(sameShow && res.kind === 'removed');
			})
		);
	});

	it('reports the same toast on any show, and none for a no-op or a superseded write', () => {
		fc.assert(
			fc.property(saveResult, (res) => {
				expect(settleSave(res, true).toast).toEqual(settleSave(res, false).toast);
				const silent = res.kind === 'noop' || res.kind === 'superseded';
				expect(settleSave(res, true).toast === null).toBe(silent);
			})
		);
		fc.assert(
			fc.property(removeResult, (res) => {
				expect(settleRemove(res, true).toast).toEqual(settleRemove(res, false).toast);
				const silent = res.kind === 'noop' || res.kind === 'superseded';
				expect(settleRemove(res, true).toast === null).toBe(silent);
			})
		);
	});
});

describe('editorToggle, by property', () => {
	it('ignores a disabled editor, closes an open one, and reopens a closed one on its write', () => {
		fc.assert(
			fc.property(fc.boolean(), fc.boolean(), fc.boolean(), (open, busy, disabled) => {
				const act = editorToggle({ open, busy, disabled });
				if (disabled) expect(act).toBe('ignore');
				else if (open) expect(act).toBe('close');
				else expect(act).toBe(busy ? 'reopen' : 'open');
			})
		);
	});
});

describe('runEditorSave and runEditorRemove, by property', () => {
	const tally = fc.record(
		{
			written: fc.integer({ min: 0, max: 3 }),
			failed: fc.integer({ min: 0, max: 3 }),
			rateLimited: fc.boolean(),
			superseded: fc.boolean()
		},
		{ requiredKeys: ['written', 'failed'] }
	);
	const save = { status: 'paused' as const, statusChanged: true, progress: 5 };

	it('is superseded exactly when nothing was written or failed and a write was superseded', async () => {
		await fc.assert(
			fc.asyncProperty(tally, async (outcome) => {
				const res = await runEditorSave(
					{ syncSetEntry: async () => outcome },
					{ kitsuId: '12', disabled: false, save }
				);
				const superseded = outcome.written === 0 && outcome.failed === 0 && !!outcome.superseded;
				expect(res.kind === 'superseded').toBe(superseded);
				if (outcome.written > 0) expect(['saved', 'partial']).toContain(res.kind);
			})
		);
	});

	it('a removal is superseded only when nothing was removed or failed', async () => {
		await fc.assert(
			fc.asyncProperty(
				fc.record(
					{
						removed: fc.integer({ min: 0, max: 3 }),
						failed: fc.integer({ min: 0, max: 3 }),
						superseded: fc.boolean()
					},
					{ requiredKeys: ['removed', 'failed'] }
				),
				async (outcome) => {
					const res = await runEditorRemove(
						{ syncRemoveEntry: async () => outcome },
						{ kitsuId: '12', disabled: false }
					);
					const superseded = outcome.removed === 0 && outcome.failed === 0 && !!outcome.superseded;
					expect(res.kind === 'superseded').toBe(superseded);
					if (!superseded && outcome.failed === 0) expect(res.kind).toBe('removed');
				}
			)
		);
	});
});
