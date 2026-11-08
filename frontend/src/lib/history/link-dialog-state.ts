/**
 * The change-show picker's decisions (LinkShowDialog.svelte), apart
 * from the component so they are tested on their own. The component
 * observes its props and the document and moves focus; this says where.
 *
 * - Opening: what had focus is remembered once per opening, never the
 *   page itself ($lib/history/focus-return.ts).
 * - Picking a show: focus goes to Link, and the pick is remembered.
 * - Back (no show picked again): focus goes to the row picked last, or
 *   the search field when nothing was.
 * - Closing: focus goes back to the opener while it is in the document,
 *   else to the surface's fallback; the opener and the pick are
 *   forgotten for the next opening.
 */

import { m } from '$lib/paraglide/messages';
import { subtypeLabel } from '$lib/search/subtype-label';
import type { LinkCandidate } from './link-candidates';
import { focusBack, openerOf } from './focus-return';

/** Where focus goes inside the open picker. */
export type DialogFocusTarget = { to: 'confirm' } | { to: 'row'; id: string } | { to: 'search' };

export interface DialogFocus {
	/** The picker is open, with `active` focused. Only an opening counts. */
	opened: (active: Element | null, body: Element | null) => void;
	/** The picker closed: where focus goes, or null when it was not open
	 *  or there is nowhere to go. */
	closed: (fallback?: () => HTMLElement | null) => HTMLElement | null;
	/** Where focus goes now, with `chosen` the id of the show picked. */
	focusFor: (chosen: string | null) => DialogFocusTarget;
}

export function createDialogFocus(): DialogFocus {
	let shown = false;
	let opener: HTMLElement | null = null;
	let lastPicked: string | null = null;
	return {
		opened: (active, body) => {
			if (shown) return;
			shown = true;
			opener = openerOf(active, body);
		},
		closed: (fallback) => {
			if (!shown) return null;
			shown = false;
			const back = focusBack(opener, fallback);
			opener = null;
			lastPicked = null;
			return back;
		},
		focusFor: (chosen) => {
			if (chosen !== null) {
				lastPicked = chosen;
				return { to: 'confirm' };
			}
			return lastPicked !== null ? { to: 'row', id: lastPicked } : { to: 'search' };
		}
	};
}

/** A candidate row's facts, in order: year, format, episode count. */
export function candidateMeta(c: LinkCandidate): { text: string; num: boolean }[] {
	const fields: { text: string; num: boolean }[] = [];
	const year = c.ref.start_date?.slice(0, 4);
	if (year) fields.push({ text: year, num: true });
	fields.push({ text: subtypeLabel(c.ref.subtype), num: false });
	const count = c.ref.episode_count;
	if (count !== null) {
		fields.push({ text: `${count} ${m.search_card_eps_suffix({ count })}`, num: true });
	}
	return fields;
}

/** What the list says to assistive tech, from one live region. */
export function listStatus(candidates: readonly LinkCandidate[] | null, failed: boolean): string {
	if (candidates === null) return m.detail_link_candidates_loading();
	if (failed) return m.detail_link_candidates_failed();
	return candidates.length === 0 ? m.detail_link_candidates_empty() : '';
}
