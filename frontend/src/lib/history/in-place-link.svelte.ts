/**
 * The link session of a surface that stays where it is: the player of
 * a guessed session and Home's Continue cards.
 *
 * The show page's session ($lib/history/link-session.svelte.ts) leaves
 * for the linked show's page after a link and comes back after an
 * undo. These do not. Home reads the card again in place, and its
 * notice holds an Undo, which makes the same offer again, so the guess
 * comes back.
 *
 * The player (`inPlayer`) asks "Is this the show you meant?". Yes
 * records the show playing on the card's row, which stops being a
 * guess for good, and says nothing — also where the play landed on
 * another listing than the card's: the user is believed, as a pick of
 * that show in the picker is. A pick of
 * another show is recorded the same way while the episode goes on
 * playing, and the slip alone says so, with the way to switch to it
 * (`linked`): the player gives no notice, so no Undo. The offer stays
 * after a pick as the ⋯ entry's way to change the show again, asking
 * nothing, and so does the session a switch opens for the row
 * ($lib/history/guess-slip.ts), which says once what plays now.
 *
 * Hide sets the player's slip aside for the card's row until the app
 * restarts.
 */

import type { KitsuAnimeRef, LinkOutcome } from '$lib/api';
import { LinkSession, type LinkNotice, type LinkSessionDeps } from './link-session.svelte';
import type { LinkDeps, LinkOffer } from './link-offer';
import { offerAgain } from './picked-offer';
import { playerNotify } from './link-notices';
import {
	hideRow,
	isRowHidden,
	notePick,
	slipState,
	takeSwitch,
	type SlipState
} from './guess-slip';

export { __resetHiddenRowsForTests, type SlipState } from './guess-slip';

export interface InPlaceLinkDeps extends LinkDeps {
	notify: (notice: LinkNotice) => void;
	/** Read again what the surface reads from the history, after a link
	 *  or an undo. */
	refresh?: () => void;
	/** The player's session: a pick is said in its slip, not a notice,
	 *  and its offer stays to change the show again. */
	inPlayer?: boolean;
}

export class InPlaceLinkSession extends LinkSession {
	/** Whether the viewer hid this row's slip. */
	aside = $state(false);
	/** The show picked for the card from here, while what plays is
	 *  still the one before; null otherwise. */
	linked = $state<{ id: string; title: string; show: KitsuAnimeRef } | null>(null);
	/** The show a switch to a pick opened this session for, which the
	 *  slip names once; null otherwise. */
	switchedTitle = $state<string | null>(null);

	#row = $state<string | null>(null);
	#page = '';
	#refresh: () => void;
	#notify: (notice: LinkNotice) => void;
	#inPlayer: boolean;
	#linkDeps: LinkDeps;

	constructor(deps: InPlaceLinkDeps) {
		const refresh = deps.refresh ?? noRefresh;
		const inPlayer = deps.inPlayer === true;
		super({
			...deps,
			// The player says what a pick did in its slip, once: no
			// notice, and so no Undo. Failures are still said.
			notify: playerNotify(deps.notify, inPlayer),
			refresh,
			// The surface is not left for the linked show's page, so an
			// undo never navigates either.
			showPage: () => {},
			here: () => ''
		} satisfies LinkSessionDeps);
		this.#refresh = refresh;
		this.#notify = deps.notify;
		this.#inPlayer = inPlayer;
		this.#linkDeps = deps;
	}

	/** What the slip shows now. */
	get slip(): SlipState {
		return slipState({
			row: this.#row,
			aside: this.aside,
			linked: this.linked !== null,
			switched: this.switchedTitle !== null,
			settled: this.settled,
			offered: this.offer !== null,
			asking: this.offer?.saved === undefined
		});
	}

	override async arrive(row: string | null, pageId: string): Promise<void> {
		this.#row = row;
		this.#page = pageId;
		this.aside = isRowHidden(row);
		this.linked = null;
		this.switchedTitle = this.#inPlayer ? takeSwitch(row) : null;
		await super.arrive(row, pageId);
	}

	/** A guessed row's offer; on the player, else the way to change the
	 *  show again of a row the viewer picked a show for there. */
	protected override async findOffer(
		row: string | null,
		pageId: string
	): Promise<LinkOffer | null> {
		return (
			(await super.findOffer(row, pageId)) ??
			offerAgain(this.#linkDeps, row, pageId, this.#inPlayer)
		);
	}

	/** Hide the slip for this row until the app restarts. */
	hide(): void {
		if (this.#row) hideRow(this.#row);
		this.aside = true;
	}

	/** "Yes" to "Is this the show you meant?": record the show playing
	 *  on the card's line, wherever the play landed — the user is
	 *  believed, and a pick of the same show in the picker writes the
	 *  same. */
	async answerYes(): Promise<void> {
		if (!this.offer) return;
		await this.confirmCurrent();
	}

	/** Link the card to the show it is guessed as. */
	async confirmCurrent(): Promise<void> {
		const offer = this.offer;
		if (!offer || this.busy) return;
		this.choose({ ref: offer.current, current: true });
		await this.confirm();
		// The picker that shows a link's error is closed: say it here.
		if (this.error) this.#notify({ kind: 'error', message: this.error });
	}

	/** Save the pick; whether it was saved, so a switch follows only a
	 *  pick that was. */
	override async confirm(): Promise<boolean> {
		const chosen = this.chosen;
		const offered = this.offer;
		if (!(await super.confirm()) || !offered || !chosen) return false;
		// Another show: what plays is still the one before, and the slip
		// says so.
		this.linked = chosen.current
			? null
			: { id: chosen.ref.id, title: chosen.ref.canonical_title, show: chosen.ref };
		if (this.#inPlayer) {
			// The ⋯ entry stays, to change the show again; the show playing
			// is still the one the picker names as playing.
			notePick(offered.entry.id);
			this.offer = { entry: offered.entry, current: offered.current, saved: chosen.ref.id };
		}
		if (chosen.ref.id !== this.#page) this.#refresh();
		return true;
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
