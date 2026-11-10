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
 *
 * An answer can land after the surface moved on. A link that lands
 * after another card arrived is saved for its own card and leaves the
 * newer one's state alone; an Undo from Home's notice gives back the
 * card the link was made on and offers it again only while that card
 * is still the one on screen, else it reads the cards again and leaves
 * the newer card's offer and pick as they are. Once the surface is
 * left (`leave`), a late link or Undo is still saved and still said,
 * and reads nothing and switches to nothing for the page that is gone.
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
	#surface: { gone: boolean };
	#refresh: () => void;
	#notify: (notice: LinkNotice) => void;
	#inPlayer: boolean;
	#linkDeps: LinkDeps;

	constructor(deps: InPlaceLinkDeps) {
		const refresh = deps.refresh ?? noRefresh;
		const inPlayer = deps.inPlayer === true;
		const surface = { gone: false };
		super({
			...deps,
			// The player says what a pick did in its slip, once: no
			// notice, and so no Undo. Failures are still said.
			notify: playerNotify(deps.notify, inPlayer),
			refresh,
			// The surface is not left for the linked show's page, so an
			// undo never navigates either. It is somewhere else only once
			// it is gone: a link answered then reads nothing again.
			showPage: () => {},
			here: () => (surface.gone ? 'gone' : '')
		} satisfies LinkSessionDeps);
		this.#surface = surface;
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

	override arrive(row: string | null, pageId: string): Promise<boolean> {
		this.#row = row;
		this.#page = pageId;
		this.aside = isRowHidden(row);
		this.linked = null;
		this.switchedTitle = this.#inPlayer ? takeSwitch(row) : null;
		return super.arrive(row, pageId);
	}

	/** The surface is gone: the page was left. Whatever is still out
	 *  answers to nothing on it, and leaves no pick to switch to. */
	leave(): void {
		this.#surface.gone = true;
		this.linked = null;
		this.supersede();
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
		const arrival = this.arrivals;
		if (!(await super.confirm()) || !offered || !chosen) return false;
		// The surface moved on to another card meanwhile: the pick is
		// saved for its own card, whose sessions keep the way to change it
		// again, and the new card's session is left as it is.
		if (arrival !== this.arrivals) {
			if (this.#inPlayer) notePick(offered.entry.id);
			return true;
		}
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

	/** Undo `outcome`, made on the page `from` for the card row `row`.
	 *  The rows are given back wherever the user is; what follows is
	 *  for the card the link was made on, while it is the one on screen. */
	override async undo(outcome: LinkOutcome, from: string, row: string): Promise<void> {
		const arrival = this.arrivals;
		const onCard = () => !this.#surface.gone && this.#row === row;
		if (onCard()) {
			// The note about the link goes now; the slip holds its room
			// until the offer is decided again.
			this.linked = null;
			this.settled = false;
		}
		// Says what the undo did, or that it failed, wherever the user is.
		await super.undo(outcome, from, row);
		if (this.#surface.gone) return;
		// The history is the truth, whatever the undo answered.
		this.#refresh();
		// Another card arrived meanwhile: its offer and pick are its own.
		if (arrival === this.arrivals && onCard()) await this.arrive(row, this.#page);
	}
}

/** A surface that reads nothing from the history. */
function noRefresh(): void {}
