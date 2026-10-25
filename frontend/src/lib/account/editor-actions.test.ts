import { describe, expect, it, vi } from 'vitest';
import {
	editorToggle,
	runEditorRemove,
	runEditorSave,
	settleRemove,
	settleSave
} from './editor-actions';
import { m } from '$lib/paraglide/messages';
import type { EditorSave } from './set-entry';

const baseSave: EditorSave = { status: 'watching', statusChanged: false, progress: 6 };

describe('runEditorSave', () => {
	it('does not write when the editor is disabled (stale between render and click)', async () => {
		const syncSetEntry = vi.fn(async () => ({ written: 1, failed: 0 }));
		const res = await runEditorSave(
			{ syncSetEntry },
			{ kitsuId: 'kitsu-12', disabled: true, save: baseSave }
		);
		expect(res).toEqual({ kind: 'noop' });
		expect(syncSetEntry).not.toHaveBeenCalled();
	});

	it('clean save (failed 0, written > 0) → saved with the optimistic live state', async () => {
		const syncSetEntry = vi.fn(async () => ({ written: 2, failed: 0 }));
		const res = await runEditorSave(
			{ syncSetEntry },
			{ kitsuId: 'kitsu-12', disabled: false, save: baseSave }
		);
		expect(res).toEqual({ kind: 'saved', live: { status: 'watching', progress: 6 } });
		expect(syncSetEntry).toHaveBeenCalledWith('kitsu-12', baseSave);
	});

	it('mirrors the promoted status: a started title saved at Planning becomes Watching', async () => {
		const syncSetEntry = vi.fn(async () => ({ written: 1, failed: 0 }));
		const res = await runEditorSave(
			{ syncSetEntry },
			{
				kitsuId: 'k',
				disabled: false,
				save: { status: 'planning', statusChanged: false, progress: 5 }
			}
		);
		expect(res).toEqual({ kind: 'saved', live: { status: 'watching', progress: 5 } });
	});

	it('a partial success (some written, some failed) → partial, keeping the landed value', async () => {
		// At least one tracker accepted the edit, so the new value is the
		// furthest-along across providers — the button must keep it (reverting
		// would hide what MAL/AniList actually stored). The caller still warns
		// that not every tracker synced.
		const syncSetEntry = vi.fn(async () => ({ written: 1, failed: 1 }));
		expect(
			await runEditorSave({ syncSetEntry }, { kitsuId: 'k', disabled: false, save: baseSave })
		).toEqual({ kind: 'partial', live: { status: 'watching', progress: 6 } });
	});

	it('a partial success carries rateLimited when a tracker 429d', async () => {
		const syncSetEntry = vi.fn(async () => ({ written: 1, failed: 1, rateLimited: true }));
		expect(
			await runEditorSave({ syncSetEntry }, { kitsuId: 'k', disabled: false, save: baseSave })
		).toEqual({ kind: 'partial', live: { status: 'watching', progress: 6 }, rateLimited: true });
	});

	it('nothing written (written 0) → failed (e.g. every tracker unmappable)', async () => {
		const syncSetEntry = vi.fn(async () => ({ written: 0, failed: 0 }));
		expect(
			await runEditorSave({ syncSetEntry }, { kitsuId: 'k', disabled: false, save: baseSave })
		).toEqual({ kind: 'failed' });
	});

	it('a thrown sync → failed (never leaves the caller hanging)', async () => {
		const syncSetEntry = vi.fn(async () => {
			throw new Error('network');
		});
		expect(
			await runEditorSave({ syncSetEntry }, { kitsuId: 'k', disabled: false, save: baseSave })
		).toEqual({ kind: 'failed' });
	});
});

describe('runEditorRemove', () => {
	it('does not remove when disabled', async () => {
		const syncRemoveEntry = vi.fn(async () => ({ removed: 1, failed: 0 }));
		expect(await runEditorRemove({ syncRemoveEntry }, { kitsuId: 'k', disabled: true })).toEqual({
			kind: 'noop'
		});
		expect(syncRemoveEntry).not.toHaveBeenCalled();
	});

	it('clean removal (failed 0, removed > 0) → removed', async () => {
		const syncRemoveEntry = vi.fn(async () => ({ removed: 1, failed: 0 }));
		const res = await runEditorRemove(
			{ syncRemoveEntry },
			{ kitsuId: 'kitsu-12', disabled: false }
		);
		expect(res).toEqual({ kind: 'removed' });
		expect(syncRemoveEntry).toHaveBeenCalledWith('kitsu-12');
	});

	it('a present tracker left behind (failed > 0) → failed', async () => {
		const syncRemoveEntry = vi.fn(async () => ({ removed: 1, failed: 1 }));
		expect(await runEditorRemove({ syncRemoveEntry }, { kitsuId: 'k', disabled: false })).toEqual({
			kind: 'failed'
		});
	});

	it('already absent on every tracker (removed 0, failed 0) → removed (postcondition met)', async () => {
		// The row was removed elsewhere; no provider still has it and nothing
		// failed, so "remove" is satisfied — clear the entry rather than treating
		// it as a failure that restores the stale on-list state.
		const syncRemoveEntry = vi.fn(async () => ({ removed: 0, failed: 0 }));
		expect(await runEditorRemove({ syncRemoveEntry }, { kitsuId: 'k', disabled: false })).toEqual({
			kind: 'removed'
		});
	});

	it('a thrown sync → failed', async () => {
		const syncRemoveEntry = vi.fn(async () => {
			throw new Error('boom');
		});
		expect(await runEditorRemove({ syncRemoveEntry }, { kitsuId: 'k', disabled: false })).toEqual({
			kind: 'failed'
		});
	});
});

describe('rate-limit flag passthrough', () => {
	it('runEditorSave carries rateLimited from a failed outcome', async () => {
		const syncSetEntry = vi.fn(async () => ({ written: 0, failed: 1, rateLimited: true }));
		expect(
			await runEditorSave({ syncSetEntry }, { kitsuId: 'k', disabled: false, save: baseSave })
		).toEqual({ kind: 'failed', rateLimited: true });
	});

	it('runEditorRemove carries rateLimited from a failed outcome', async () => {
		const syncRemoveEntry = vi.fn(async () => ({ removed: 0, failed: 1, rateLimited: true }));
		expect(await runEditorRemove({ syncRemoveEntry }, { kitsuId: 'k', disabled: false })).toEqual({
			kind: 'failed',
			rateLimited: true
		});
	});
});

describe('settling a result', () => {
	it('reports a save that ends after the editor moved to another show, and leaves that show alone', () => {
		const live = { status: 'watching' as const, progress: 6 };
		expect(settleSave({ kind: 'saved', live }, false)).toEqual({
			toast: { kind: 'success', message: m.detail_list_saved() },
			local: false
		});
		expect(settleSave({ kind: 'partial', live }, false)).toEqual({
			toast: { kind: 'error', message: m.detail_list_save_partial() },
			local: false
		});
		expect(settleSave({ kind: 'failed', rateLimited: true }, false)).toEqual({
			toast: { kind: 'error', message: m.detail_list_rate_limited() },
			local: false
		});
		expect(settleRemove({ kind: 'removed' }, false)).toEqual({
			toast: { kind: 'success', message: m.detail_list_removed() },
			local: false
		});
	});

	it('takes the result locally on the same show', () => {
		const live = { status: 'watching' as const, progress: 6 };
		expect(settleSave({ kind: 'saved', live }, true).local).toBe(true);
		expect(settleSave({ kind: 'partial', live }, true).local).toBe(true);
		expect(settleSave({ kind: 'failed' }, true)).toEqual({
			toast: { kind: 'error', message: m.detail_list_save_failed() },
			local: false
		});
		expect(settleRemove({ kind: 'removed' }, true).local).toBe(true);
		expect(settleSave({ kind: 'noop' }, true)).toEqual({ toast: null, local: false });
	});

	it('shows the rate-limited message for a save every attempt of which was refused', async () => {
		const res = await runEditorSave(
			{ syncSetEntry: async () => ({ written: 0, failed: 1, rateLimited: true }) },
			{ kitsuId: 'kitsu-12', disabled: false, save: baseSave }
		);
		expect(settleSave(res, true).toast).toEqual({
			kind: 'error',
			message: m.detail_list_rate_limited()
		});
	});

	it('says nothing for a save a newer write superseded, and takes nothing', async () => {
		const res = await runEditorSave(
			{ syncSetEntry: async () => ({ written: 0, failed: 0, superseded: true }) },
			{ kitsuId: 'kitsu-12', disabled: false, save: baseSave }
		);
		expect(res).toEqual({ kind: 'superseded' });
		expect(settleSave(res, true)).toEqual({ toast: null, local: false });
	});
});

describe('editorToggle', () => {
	it('opens a closed editor, and not while it is disabled', () => {
		expect(editorToggle({ open: false, busy: false, disabled: false })).toBe('open');
		expect(editorToggle({ open: false, busy: false, disabled: true })).toBe('ignore');
	});

	it('reopens a closed editor whose write is still out, showing that write rather than a fresh form', () => {
		expect(editorToggle({ open: false, busy: true, disabled: false })).toBe('reopen');
	});

	it('closes an open editor even while its write waits — the write carries on', () => {
		expect(editorToggle({ open: true, busy: true, disabled: false })).toBe('close');
		expect(editorToggle({ open: true, busy: false, disabled: false })).toBe('close');
	});
});

describe('a removal a newer write superseded', () => {
	it('says nothing and takes nothing, where one that removed somewhere still clears', async () => {
		const superseded = await runEditorRemove(
			{ syncRemoveEntry: async () => ({ removed: 0, failed: 0, superseded: true }) },
			{ kitsuId: 'kitsu-12', disabled: false }
		);
		expect(superseded).toEqual({ kind: 'superseded' });
		expect(settleRemove(superseded, true)).toEqual({ toast: null, local: false });
		const cleared = await runEditorRemove(
			{ syncRemoveEntry: async () => ({ removed: 1, failed: 0, superseded: true }) },
			{ kitsuId: 'kitsu-12', disabled: false }
		);
		expect(cleared).toEqual({ kind: 'removed' });
	});
});
