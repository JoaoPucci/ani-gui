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
import type { KitsuAnimeRef } from '$lib/api';
import { isSingleVideo } from '$lib/detail/play-label';
import { subtypeLabel } from '$lib/search/subtype-label';
import type { LinkCandidate } from '$lib/api';
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

/** Whether `show` is a single-video work, whose episode the player never
 *  names ($lib/detail/play-label). */
function single(show: KitsuAnimeRef | null): boolean {
	return show !== null && isSingleVideo(show.subtype, show.episode_count, show.status);
}

/** The picker's first line: the title the history saved and the
 *  episode the surface offers. On the player (`playing`) that is the
 *  episode playing, and none for a single-video work, as the player
 *  names none; Home's card and the show page name the episode they
 *  offer whatever the work. `current` is the show the card shows now,
 *  null while it is not known. */
export function listBody(o: {
	playing: boolean;
	episode: string;
	savedTitle: string;
	current: KitsuAnimeRef | null;
}): string {
	const title = o.savedTitle;
	if (!o.playing) return m.detail_link_dialog_body({ episode: o.episode, title });
	return single(o.current)
		? m.play_link_dialog_body_single({ title })
		: m.play_link_dialog_body({ episode: o.episode, title });
}

/** Where a switch to `to` picks up: the episode and the time, or the
 *  time alone for a single-video work. */
export function switchHint(o: { episode: string; time: string; to: KitsuAnimeRef }): string {
	return single(o.to)
		? m.play_link_switch_hint_single({ time: o.time })
		: m.play_link_switch_hint({ episode: o.episode, time: o.time });
}
