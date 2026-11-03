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
 * restarts ($lib/history/guess-slip.ts).
 */

import type { LinkOutcome } from '$lib/api';
import { LinkSession, type LinkNotice, type LinkSessionDeps } from './link-session.svelte';
import type { LinkDeps } from './link-offer';
import { keptNotice } from './link-notices';
import { hideRow, isRowHidden, slipState, type SlipState } from './guess-slip';

export { __resetHiddenRowsForTests, type SlipState } from './guess-slip';

export interface InPlaceLinkDeps extends LinkDeps {
	notify: (notice: LinkNotice) => void;
	/** Read again what the surface reads from the history, after a link
	 *  or an undo. */
	refresh?: () => void;
}

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
	#notify: (notice: LinkNotice) => void;

	constructor(deps: InPlaceLinkDeps) {
		const refresh = deps.refresh ?? noRefresh;
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
		this.#notify = deps.notify;
	}

	/** What the slip shows now. */
	get slip(): SlipState {
		return slipState({
			row: this.#row,
			aside: this.aside,
			linked: this.linked !== null,
			settled: this.settled,
			offered: this.offer !== null
		});
	}

	override async arrive(row: string | null, pageId: string): Promise<void> {
		this.#row = row;
		this.#page = pageId;
		this.aside = isRowHidden(row);
		this.linked = null;
		await super.arrive(row, pageId);
	}

	/** Hide the slip for this row until the app restarts. */
	hide(): void {
		if (this.#row) hideRow(this.#row);
		this.aside = true;
	}

	/** "That's right": link the card to the show it is guessed as. */
	async confirmCurrent(): Promise<void> {
		const offer = this.offer;
		if (!offer || this.busy) return;
		this.choose({ ref: offer.current, current: true });
		await this.confirm();
		// The picker that shows a link's error is closed: say it here.
		if (this.error) this.#notify({ kind: 'error', message: this.error });
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

/** A surface that reads nothing from the history. */
function noRefresh(): void {}
