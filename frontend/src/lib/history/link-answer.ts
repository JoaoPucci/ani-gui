/**
 * What a link session does with a link's answer — split from
 * $lib/history/link-session.svelte.ts for the per-file complexity bar.
 *
 * A saved link carries the viewer's Sub or Dub to the pick
 * ($lib/history/carry-audio.ts), says the show was saved, and, while
 * the viewer stayed on the visit it was made from, lands there: the
 * page is opened again on the linked show without the card's row,
 * since there is nothing left to offer; on the same surface moved on
 * to another card, it only reads the history again. A link a
 * correction asked later overtook (`overtaken`) saved nothing: nothing
 * is said or carried, the picker closes on the visit it was made from,
 * and the surface reads its history again, or on the player its line.
 * A failed link says why: in the picker while the viewer stayed, else
 * in a notice.
 */

import type { KitsuAnimeRef, LinkOutcome } from '$lib/api';
import { carryAudio } from './carry-audio';
import { linkedNotice, linkFailure } from './link-notices';
import type { LinkOffer } from './link-offer';
import type { LinkSessionDeps } from './link-session.svelte';

/** The session state a link's answer changes. */
export interface AnsweredSession {
	open: boolean;
	offer: LinkOffer | null;
	error: string | null;
}

/** A link of `offer`'s card to `chosen`, asked on the page `from`. */
export interface LinkAsked {
	session: AnsweredSession;
	deps: LinkSessionDeps;
	offer: LinkOffer;
	chosen: KitsuAnimeRef;
	from: string;
	/** The viewer is still on the visit the link was made from. */
	stayed: () => boolean;
	/** The viewer is still on the surface, on that visit or another. */
	onSurface: () => boolean;
}

/** Act on a link's answer; whether it saved. */
export function linkAnswered(outcome: LinkOutcome, a: LinkAsked): boolean {
	if (outcome.overtaken === true) {
		if (a.stayed()) a.session.open = false;
		if (a.onSurface()) (a.deps.reread ?? a.deps.refresh)();
		return false;
	}
	carryAudio(a.deps, a.offer.current.id, a.chosen.id);
	a.deps.notify(linkedNotice(a.chosen.canonical_title));
	if (a.stayed()) landed(a);
	else if (a.onSurface()) a.deps.refresh();
	return true;
}

function landed(a: LinkAsked): void {
	a.session.open = false;
	a.session.offer = null;
	if (a.chosen.id === a.from) a.deps.refresh();
	a.deps.showPage(a.chosen.id, null);
}

/** Say why a link failed; it saved nothing. */
export function linkFailed(error: unknown, a: LinkAsked): false {
	if (a.stayed()) a.session.error = linkFailure(error);
	else a.deps.notify({ kind: 'error', message: linkFailure(error) });
	return false;
}
