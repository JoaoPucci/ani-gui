/**
 * What the link session tells the user, and its undo — split from
 * $lib/history/link-session.svelte.ts for the per-file complexity bar.
 */

import { m } from '$lib/paraglide/messages';
import type { LinkOutcome } from '$lib/api';
import { kitsuEntryGone } from './match';
import { undoLink } from './link-offer';
import type { LinkNotice, LinkSessionDeps } from './link-session.svelte';

/** The notice a link leaves, with its Undo. */
export function linkedNotice(title: string, onUndo: () => void): LinkNotice {
	return {
		kind: 'success',
		message: m.detail_link_done_toast({ title }),
		actionLabel: m.detail_link_undo(),
		onAction: onUndo
	};
}

/** Why a link failed, as the picker says it. */
export function linkFailure(e: unknown): string {
	return kitsuEntryGone(e) ? m.detail_link_failed_gone() : m.detail_link_failed();
}

/** Undo `outcome`, made on the page `from` for the card row `row`.
 *  The page goes back to `from` only while the user is still on the
 *  page the link left them on; anywhere else, they stay. */
export async function undoAndReturn(
	deps: LinkSessionDeps,
	outcome: LinkOutcome,
	from: string,
	row: string
): Promise<void> {
	const stayed = deps.here() === `/anime/${outcome.kitsu_id}`;
	try {
		const all = await undoLink(deps, outcome);
		deps.notify(
			all
				? { kind: 'info', message: m.detail_link_undone_toast() }
				: { kind: 'error', message: m.detail_link_undo_kept_toast() }
		);
	} catch {
		deps.notify({ kind: 'error', message: m.detail_link_failed() });
		return;
	}
	if (!stayed) return;
	deps.refresh();
	deps.showPage(from, row);
}
