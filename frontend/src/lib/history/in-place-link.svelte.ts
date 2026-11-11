/**
 * The link session of a surface that stays where it is: the player of
 * a card's line and Home's Continue cards — for a guess, or a line the
 * user named a show for.
 *
 * The show page's session ($lib/history/link-session.svelte.ts) leaves
 * for the linked show's page after a link. These do not: Home reads
 * the card again in place, and says the show was saved in a plain
 * notice.
 *
 * The player (`inPlayer`) asks "Is this the show you meant?". Yes
 * records the show playing on the card's row, which stops being a
 * guess for good, and says nothing — also where the play landed on
 * another listing than the card's: the user is believed, as a pick of
 * that show in the picker is. A pick of
 * another show is recorded the same way while the episode goes on
 * playing, and the slip alone says so, with the way to switch to it
 * (`linked`): the player gives no notice while it is on screen (a pick
 * saved once it is gone is said in one). The offer stays
 * after a pick as the ⋯ entry's way to change the show again, asking
 * nothing, and so does the session a switch opens for the row
 * ($lib/history/guess-slip.ts), which says once what plays now.
 *
 * Hide sets the player's slip aside for the card's row until the app
 * restarts.
 *
 * An answer can land after the surface moved on. A link that lands
 * after another card arrived is saved for its own card and leaves the
 * newer one's state alone. Once the surface is left (`leave`), a late
 * link or pick is still saved and still said — a pick on the player
 * too, in a notice, since its slip is gone — and reads nothing and
 * switches to nothing for the page that is gone. Whatever Home or show
 * page is on screen hears of it from the app ($lib/history/link-deps.ts).
 */

import type { KitsuAnimeRef } from '$lib/api';
import { LinkSession, type LinkNotice, type LinkSessionDeps } from './link-session.svelte';
import type { LinkDeps } from './link-offer';
import { playerNotify } from './link-notices';
import { surface, type Surface } from './surface';
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
	/** Read again what the surface reads from the history, after a link. */
	refresh?: () => void;
	/** The player's session: a pick is said in its slip, not a notice,
	 *  and its offer stays to change the show again. */
	inPlayer?: boolean;
	/** The Sub or Dub the viewer watches the shown show in
	 *  ($lib/history/carry-audio.ts). */
	audioInForce?: LinkSessionDeps['audioInForce'];
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
	#guess = $state(true);
	#page = '';
	#surface: Surface;
	#refresh: () => void;
	#notify: (notice: LinkNotice) => void;
	#inPlayer: boolean;

	constructor(deps: InPlaceLinkDeps) {
		const refresh = deps.refresh ?? noRefresh;
		const inPlayer = deps.inPlayer === true;
		const place = surface();
		super({
			...deps,
			// The player says what a pick did in its slip, once: no
			// notice. Failures are still said, and so is a pick saved once
			// the player is gone, with its slip.
			notify: playerNotify(deps.notify, inPlayer, place),
			refresh,
			// The surface is not left for the linked show's page. It is
			// somewhere else only once it is gone: a link answered then
			// reads nothing again.
			showPage: () => {},
			here: place.here
		} satisfies LinkSessionDeps);
		this.#surface = place;
		this.#refresh = refresh;
		this.#notify = deps.notify;
		this.#inPlayer = inPlayer;
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
			asking: this.offer?.saved === undefined,
			asks: this.#guess
		});
	}

	/** The surface shows `pageId` for the card row `row`; on the player,
	 *  `guess` says whether a guess opened the session, else it only
	 *  offers to change again a show the user named. */
	override arrive(row: string | null, pageId: string, guess = true): Promise<boolean> {
		this.#guess = guess;
		this.#row = row;
		this.#page = pageId;
		this.aside = isRowHidden(row);
		this.linked = null;
		this.switchedTitle = this.#inPlayer ? takeSwitch(row) : null;
		return super.arrive(row, pageId);
	}

	/** Whether the latest arrival's offer stands: null when it could not
	 *  be read, so a check of the card may be asked again. */
	get stands(): boolean | null {
		if (this.offer) return true;
		return this.unread ? null : false;
	}

	/** The surface is gone: the page was left. Whatever is still out
	 *  answers to nothing on it, and leaves no pick to switch to. */
	leave(): void {
		this.#surface.leave();
		this.endReads();
		this.linked = null;
		this.supersede();
	}

	protected override get inPlayer(): boolean {
		return this.#inPlayer;
	}

	protected override get asksGuess(): boolean {
		return this.#guess;
	}

	/** The player of a session a guessed card opened carries the card's
	 *  verdict; Home judges each card as it reads it. */
	protected override get carriesGuess(): boolean {
		return this.#inPlayer && this.#guess;
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
}

/** A surface that reads nothing from the history. */
function noRefresh(): void {}
