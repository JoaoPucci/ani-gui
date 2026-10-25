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
import { m } from '$lib/paraglide/messages';

/**
 * Outcome of a Save: `noop` when the editor was disabled (so nothing was
 * written), `saved` with the optimistic `live` entry to show until the
 * page re-reads, or `failed` when no clean write landed.
 */
export type SaveResult =
	| { kind: 'noop' }
	| { kind: 'saved'; live: EntryView }
	// At least one tracker accepted the edit but another failed. The edit is
	// the furthest-along value across providers, so the button keeps `live`;
	// the caller warns that not every tracker synced.
	| { kind: 'partial'; live: EntryView; rateLimited?: boolean }
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
		if (written > 0) {
			// Something landed → the edit is the furthest-along value across
			// trackers, so surface it. `failed > 0` means a partial sync.
			const status = effectiveStatus(input.save.status, input.save.progress);
			const live = {
				status,
				progress: effectiveProgress(status, input.save.progress, input.save.total ?? null)
			};
			if (failed === 0) return { kind: 'saved', live };
			return outcome.rateLimited
				? { kind: 'partial', live, rateLimited: true }
				: { kind: 'partial', live };
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
		// `failed === 0` means no present tracker was left behind — whether we
		// deleted the row (`removed > 0`) or it was already absent everywhere
		// (`removed === 0`). Both satisfy "not on any list", so clear the entry.
		if (outcome.failed === 0) return { kind: 'removed' };
		return outcome.rateLimited ? { kind: 'failed', rateLimited: true } : { kind: 'failed' };
	} catch {
		return { kind: 'failed' };
	}
}

/** What the editor does with a write's result: the toast that reports
 *  it, if any, and whether the editor's own state — the button's value,
 *  the pending edit, the popover — takes the result. */
export interface Settled {
	toast: { kind: 'success' | 'error'; message: string } | null;
	local: boolean;
}

const NOTHING: Settled = { toast: null, local: false };

function failedToast(rateLimited: boolean | undefined): Settled['toast'] {
	return {
		kind: 'error',
		message: rateLimited ? m.detail_list_rate_limited() : m.detail_list_save_failed()
	};
}

/** How a Save's result settles. `sameShow` is false when the editor has
 *  moved to another show while the write was out. */
export function settleSave(res: SaveResult, sameShow: boolean): Settled {
	if (!sameShow || res.kind === 'noop') return NOTHING;
	if (res.kind === 'failed') return { toast: failedToast(res.rateLimited), local: false };
	if (res.kind === 'partial') {
		return { toast: { kind: 'error', message: m.detail_list_save_partial() }, local: true };
	}
	return { toast: { kind: 'success', message: m.detail_list_saved() }, local: true };
}

/** How a Remove's result settles — see {@link settleSave}. */
export function settleRemove(res: RemoveResult, sameShow: boolean): Settled {
	if (!sameShow || res.kind === 'noop') return NOTHING;
	if (res.kind === 'failed') return { toast: failedToast(res.rateLimited), local: false };
	return { toast: { kind: 'success', message: m.detail_list_removed() }, local: true };
}
