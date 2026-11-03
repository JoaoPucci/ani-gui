/**
 * The link session of a surface that stays where it is: the player of
 * a guessed session and Home's Continue cards.
 *
 * The show page's session ($lib/history/link-session.svelte.ts) leaves
 * for the linked show's page after a link and comes back after an
 * undo. These do not. The player goes on playing the guessed show's
 * episode, and says so after a link to another show, with a way to it;
 * Home reads the card again in place. An undo makes the same offer
 * again, so the guess comes back.
 *
 * "That's right" links the card to the show it is guessed as: the
 * guess is recorded on the row and stops being a guess for good. Its
 * notice says the show was kept, not linked.
 *
 * Hide sets the player's slip aside for the card's row until the app
 * restarts: a module-level set, which lives as long as the renderer.
 */

import type { LinkOutcome } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import { LinkSession, type LinkNotice, type LinkSessionDeps } from './link-session.svelte';
import type { LinkDeps } from './link-offer';

export interface InPlaceLinkDeps extends LinkDeps {
	notify: (notice: LinkNotice) => void;
	/** Read again what the surface reads from the history, after a link
	 *  or an undo. */
	refresh?: () => void;
}

/** What the player's slip shows: nothing, its held room while the
 *  offer is decided, the guess, or the note after a link elsewhere. */
export type SlipState = 'none' | 'holding' | 'guess' | 'linked';

/** Rows whose slip the viewer hid, for this run of the app. Read
 *  through `aside`, which carries what the page shows. */
// eslint-disable-next-line svelte/prefer-svelte-reactivity
const hiddenRows = new Set<string>();

export class InPlaceLinkSession extends LinkSession {
	/** Whether the viewer hid this row's slip. */
	aside = $state(false);
	/** The show the card was linked to from here, while what plays is
	 *  still the guessed show; null otherwise. */
	linked = $state<{ id: string; title: string } | null>(null);

	#row = $state<string | null>(null);
	#page = '';
	#refresh: () => void;
	#keeping: { title: string | null };

	constructor(deps: InPlaceLinkDeps) {
		const refresh = deps.refresh ?? (() => {});
		const keeping = { title: null as string | null };
		super({
			...deps,
			// A guess confirmed is kept, not changed: its notice says so.
			notify: (notice) => deps.notify(keptNotice(notice, keeping.title)),
			refresh,
			// The surface is not left for the linked show's page, so an
			// undo never navigates either.
			showPage: () => {},
			here: () => ''
		} satisfies LinkSessionDeps);
		this.#refresh = refresh;
		this.#keeping = keeping;
	}

	/** What the slip shows now. */
	get slip(): SlipState {
		if (this.#row === null || this.aside) return 'none';
		if (this.linked) return 'linked';
		if (!this.settled) return 'holding';
		return this.offer ? 'guess' : 'none';
	}

	override async arrive(row: string | null, pageId: string): Promise<void> {
		this.#row = row;
		this.#page = pageId;
		this.aside = row !== null && hiddenRows.has(row);
		this.linked = null;
		await super.arrive(row, pageId);
	}

	/** Hide the slip for this row until the app restarts. */
	hide(): void {
		if (this.#row) hiddenRows.add(this.#row);
		this.aside = true;
	}

	/** "That's right": link the card to the show it is guessed as. */
	async confirmCurrent(): Promise<void> {
		const offer = this.offer;
		if (!offer || this.busy) return;
		this.choose({ ref: offer.current, current: true });
		await this.confirm();
	}

	override async confirm(): Promise<void> {
		const chosen = this.chosen;
		const offered = this.offer;
		this.#keeping.title = chosen?.current ? chosen.ref.canonical_title : null;
		try {
			await super.confirm();
		} finally {
			this.#keeping.title = null;
		}
		if (!offered || !chosen || this.offer || this.error) return;
		// Another show: what plays is still the guess's, and the slip
		// says so.
		this.linked = chosen.current ? null : { id: chosen.ref.id, title: chosen.ref.canonical_title };
		if (chosen.ref.id !== this.#page) this.#refresh();
	}

	override async undo(outcome: LinkOutcome, from: string, row: string): Promise<void> {
		// The note about the link goes now; the slip holds its room
		// until the offer is decided again.
		this.linked = null;
		this.settled = false;
		await super.undo(outcome, from, row);
		this.#refresh();
		await this.arrive(this.#row, this.#page);
	}
}

/** The notice a link leaves, reworded for a guess kept as it was. */
function keptNotice(notice: LinkNotice, keptTitle: string | null): LinkNotice {
	if (keptTitle === null || notice.kind !== 'success') return notice;
	return { ...notice, message: m.play_link_kept_toast({ title: keptTitle }) };
}

/** Test seam: forget every hidden row, as a restart does. */
export function __resetHiddenRowsForTests(): void {
	hiddenRows.clear();
}
