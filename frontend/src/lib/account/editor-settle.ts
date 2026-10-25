// How the list editor settles a write's result — the toast that reports
// it and whether the editor's own state takes it — and what a click on
// its trigger does. Split from `editor-actions.ts` so each file stays
// inside the CRAP gate's per-file bar; re-exported from there.

import type { RemoveResult, SaveResult } from './editor-actions';
import { m } from '$lib/paraglide/messages';

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

/** How a Save's result settles. The result is always reported — a save
 *  that waited out a rate limit can end after the user moved on — but
 *  the editor takes it only when it is still on the show (`sameShow`).
 *  A superseded save says and takes nothing. */
export function settleSave(res: SaveResult, sameShow: boolean): Settled {
	if (res.kind === 'noop' || res.kind === 'superseded') return NOTHING;
	if (res.kind === 'failed') return { toast: failedToast(res.rateLimited), local: false };
	if (res.kind === 'partial') {
		return { toast: { kind: 'error', message: m.detail_list_save_partial() }, local: sameShow };
	}
	return { toast: { kind: 'success', message: m.detail_list_saved() }, local: sameShow };
}

/** How a Remove's result settles — see {@link settleSave}. */
export function settleRemove(res: RemoveResult, sameShow: boolean): Settled {
	if (res.kind === 'noop' || res.kind === 'superseded') return NOTHING;
	if (res.kind === 'failed') return { toast: failedToast(res.rateLimited), local: false };
	return { toast: { kind: 'success', message: m.detail_list_removed() }, local: sameShow };
}

/** What a click on the editor's trigger does. A disabled editor does
 *  nothing — its live entry is still loading or its read failed, so the
 *  form would seed Planning/0 and a Save could overwrite a real entry.
 *  An open one closes, even while its write waits out a rate limit: the
 *  write carries on and its toast reports how it ended. A closed one
 *  opens, unless a write is still out — it would seed a form the
 *  write's result is about to move. */
export function editorToggle(state: {
	open: boolean;
	busy: boolean;
	disabled: boolean;
}): 'open' | 'close' | 'ignore' {
	if (state.disabled) return 'ignore';
	if (state.open) return 'close';
	return state.busy ? 'ignore' : 'open';
}
