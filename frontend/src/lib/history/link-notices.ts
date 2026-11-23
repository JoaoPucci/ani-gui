/**
 * What the link session tells the user — split from
 * $lib/history/link-session.svelte.ts for the per-file complexity bar.
 */

import { m } from '$lib/paraglide/messages';
import { kitsuEntryGone } from './match';
import type { LinkNotice } from './link-session.svelte';

/** The notice a link leaves: a plain confirmation. The answer is the
 *  viewer's, recorded once given; Home's "Not what you watched?",
 *  the player's "Not this show…" or the show page's "Not this show?",
 *  which stay on the line ($lib/history/picked-offer.ts), change it. */
export function linkedNotice(title: string): LinkNotice {
	return { kind: 'success', message: m.detail_link_done_toast({ title }) };
}

/** What the player tells the user (`inPlayer`): failures only, since
 *  its slip says what a pick did, once — unless the player is gone,
 *  and no slip is left to say it. Anywhere else, every notice. */
export function playerNotify(
	notify: (notice: LinkNotice) => void,
	inPlayer: boolean,
	player: { readonly gone: boolean }
): (notice: LinkNotice) => void {
	return (notice) => {
		if (!inPlayer || notice.kind === 'error' || player.gone) notify(notice);
	};
}

/** Why a link failed, as the picker says it. */
export function linkFailure(e: unknown): string {
	return kitsuEntryGone(e) ? m.detail_link_failed_gone() : m.detail_link_failed();
}
