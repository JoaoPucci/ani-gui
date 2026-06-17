import { describe, expect, it, vi } from 'vitest';
import { runEditorRemove, runEditorSave } from './editor-actions';
import type { EditorSave } from './set-entry';

const baseSave: EditorSave = { status: 'watching', seededStatus: 'watching', progress: 6 };

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
				save: { status: 'planning', seededStatus: 'planning', progress: 5 }
			}
		);
		expect(res).toEqual({ kind: 'saved', live: { status: 'watching', progress: 5 } });
	});

	it('a partial failure (failed > 0) → failed, no optimistic state', async () => {
		const syncSetEntry = vi.fn(async () => ({ written: 1, failed: 1 }));
		expect(
			await runEditorSave({ syncSetEntry }, { kitsuId: 'k', disabled: false, save: baseSave })
		).toEqual({ kind: 'failed' });
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

	it('nothing removed (removed 0) → failed (no tracker had the row)', async () => {
		const syncRemoveEntry = vi.fn(async () => ({ removed: 0, failed: 0 }));
		expect(await runEditorRemove({ syncRemoveEntry }, { kitsuId: 'k', disabled: false })).toEqual({
			kind: 'failed'
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
