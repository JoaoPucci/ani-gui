// The detail-page list editor's save/remove state machine, lifted out of
// ListEntryEditor.svelte so the behaviour most likely to regress — the
// disabled gate, the multi-tracker outcome interpretation, the optimistic
// post-save state, and the success/failure decision — is unit-tested
// rather than buried in an untestable `.svelte` file (AGENTS.md §2). The
// sync fan-out is injected so these stay pure; the component is left as
// glue that flips `busy`, applies the returned `live`, and picks a toast.

import type { EntryView } from './types';
import type { EditorSave, RemoveOutcome, SetOutcome } from './set-entry';
import { effectiveProgress, effectiveStatus } from './list-entry-edit';

/**
 * Outcome of a Save: `noop` when the editor was disabled (so nothing was
 * written), `saved` with the optimistic `live` entry to show until the
 * page re-reads, or `failed` when no clean write landed.
 */
export type SaveResult =
	| { kind: 'noop' }
	| { kind: 'saved'; live: EntryView }
	| { kind: 'failed'; rateLimited?: boolean };

/** Outcome of a Remove. `removed` clears the entry; see {@link SaveResult}.
 *  `failed` carries `rateLimited` so the editor can show a retry-specific
 *  toast when a tracker 429'd. */
export type RemoveResult = { kind: 'noop' | 'removed' } | { kind: 'failed'; rateLimited?: boolean };

/**
 * Run the editor's Save against every connected tracker (via the injected
 * `syncSetEntry` fan-out) and decide the resulting UI state.
 *
 * - `disabled` short-circuits to `noop` — belt to the disabled-gated button
 *   and the auto-close effect, so a Save can't fire if the editor went
 *   stale (navigation/loading) between render and click.
 * - A clean save requires every tracker the edit reached to have accepted
 *   it (`failed === 0`) AND at least one write to have landed
 *   (`written > 0`); on any partial failure we report `failed` rather than
 *   showing the new state as if it saved everywhere while a tracker stayed
 *   stale. The optimistic `live` mirrors the status actually written — a
 *   started title saved at Planning is promoted to Watching by the fan-out
 *   ({@link effectiveStatus}), so the button must reflect Watching.
 * - A thrown sync collapses to `failed`.
 */
export async function runEditorSave(
	deps: { syncSetEntry: (kitsuId: string, save: EditorSave) => Promise<SetOutcome> },
	input: { kitsuId: string; disabled: boolean; save: EditorSave }
): Promise<SaveResult> {
	if (input.disabled) return { kind: 'noop' };
	try {
		const outcome = await deps.syncSetEntry(input.kitsuId, input.save);
		const { written, failed } = outcome;
		if (failed === 0 && written > 0) {
			const status = effectiveStatus(input.save.status, input.save.progress);
			return {
				kind: 'saved',
				live: {
					status,
					progress: effectiveProgress(status, input.save.progress, input.save.total ?? null)
				}
			};
		}
		return outcome.rateLimited ? { kind: 'failed', rateLimited: true } : { kind: 'failed' };
	} catch {
		return { kind: 'failed' };
	}
}

/**
 * Run the editor's Remove against every connected tracker (via the injected
 * `syncRemoveEntry` fan-out). A clean removal requires every tracker that
 * had the row to have removed it (`failed === 0`) AND at least one to have
 * actually been removed (`removed > 0`); otherwise the title still lives on
 * a tracker, so we report `failed` and keep the entry visible rather than
 * pretending it's gone everywhere. `disabled` → `noop`; a throw → `failed`.
 */
export async function runEditorRemove(
	deps: { syncRemoveEntry: (kitsuId: string) => Promise<RemoveOutcome> },
	input: { kitsuId: string; disabled: boolean }
): Promise<RemoveResult> {
	if (input.disabled) return { kind: 'noop' };
	try {
		const outcome = await deps.syncRemoveEntry(input.kitsuId);
		const { removed, failed } = outcome;
		if (failed === 0 && removed > 0) return { kind: 'removed' };
		return outcome.rateLimited ? { kind: 'failed', rateLimited: true } : { kind: 'failed' };
	} catch {
		return { kind: 'failed' };
	}
}
