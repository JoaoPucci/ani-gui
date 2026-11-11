/**
 * The show page's link session: the quiet offer the page a Continue
 * card's player opened makes — for a guess, or a line the user named a
 * show for — the picker it opens, and the link.
 *
 * The page hands it what the app provides ($lib/history/link-offer.ts)
 * and three effects of its own — a notice, a navigation, a refresh of
 * what it reads from the history (empty on the show page, which hears
 * of every link from the app instead, $lib/history/link-deps.ts)
 * — and renders its state. Arriving on
 * a page decides the offer; a later arrival outdates an earlier one
 * still out. A link opens the chosen show's page without the card's
 * row, refreshing it first when it is the page the link was made on,
 * unless the user left the page while the link was out. Its notice is
 * a plain confirmation: the answer is the viewer's, and the way to
 * change it is "Not this show?", which stays on every line the user
 * named a show for ($lib/history/picked-offer.ts).
 */

import type { ReadOptions } from '$lib/api';
import { linkedNotice, linkFailure } from './link-notices';
import { carryAudio } from './carry-audio';
import { LinkPicker } from './link-picker.svelte';
import { guessedGroup, linkOffer, type LinkDeps, type LinkOffer } from './link-offer';
import { offerAgain } from './picked-offer';

/** A notice to the user. */
export interface LinkNotice {
	kind: 'success' | 'error';
	message: string;
}

export interface LinkSessionDeps extends LinkDeps {
	notify: (notice: LinkNotice) => void;
	/** Go to the show page of `kitsuId`, as opened from the guessed
	 *  card's row `row` when one is given. */
	showPage: (kitsuId: string, row: string | null) => void;
	/** Read again what the page reads from the history. */
	refresh: () => void;
	/** The path the app is on now. */
	here: () => string;
	/** The Sub or Dub the viewer watches the show `shownId` in, as the
	 *  surface plays it — its own choice, else the Settings default —
	 *  or null when none is known yet. */
	audioInForce?: (shownId: string) => 'sub' | 'dub' | null;
}

export class LinkSession extends LinkPicker {
	open = $state(false);
	busy = $state(false);
	/** Whether the latest arrival's offer is decided. */
	settled = $state(false);
	/** The history rows the card stands for, once worked out. */
	group = $state<string[] | null>(null);

	#deps: LinkSessionDeps;
	#arrival = 0;
	#groupAsk = 0;
	#pageId = '';

	/** How many arrivals the page has had: a link answered after a
	 *  later one leaves that one's state alone. */
	protected get arrivals(): number {
		return this.#arrival;
	}

	constructor(deps: LinkSessionDeps) {
		super(deps);
		this.#deps = deps;
	}

	/** Outdate every arrival still out, and every link answered after
	 *  this, as a later arrival does. */
	protected supersede(): void {
		++this.#arrival;
	}

	/** The page `pageId` was opened, from the guessed card's row `row`
	 *  or from anywhere else (null). Whether its offer stands as the
	 *  session's: no when a later arrival overtook it. */
	async arrive(row: string | null, pageId: string): Promise<boolean> {
		const arrival = ++this.#arrival;
		const reads = this.beginReads();
		this.#pageId = pageId;
		this.offer = null;
		this.open = false;
		this.settled = false;
		this.group = null;
		const offer = await this.findOffer(row, pageId, { signal: reads.signal });
		if (arrival !== this.#arrival) return false;
		this.offer = offer;
		this.settled = true;
		return true;
	}

	/** The offer the page `pageId` makes for the card row `row`: a
	 *  guess's, else the way to change again a row the user named a show
	 *  for ($lib/history/picked-offer.ts). */
	protected async findOffer(
		row: string | null,
		pageId: string,
		opts: ReadOptions
	): Promise<LinkOffer | null> {
		const guess = this.asksGuess ? await linkOffer(this.#deps, row, pageId, opts) : null;
		return guess ?? offerAgain(this.#deps, row, pageId, this.inPlayer, opts);
	}

	/** Whether the arrival may ask about a guess: a session no guess
	 *  opened only changes again a show the user named. */
	protected get asksGuess(): boolean {
		return true;
	}

	/** Whether the session is the player's. */
	protected get inPlayer(): boolean {
		return false;
	}

	/** Open the picker on the offer's candidates, naming `episode` —
	 *  the one the surface it opened from offers for the card — when
	 *  given. */
	async start(episode?: number): Promise<void> {
		const offer = this.offer;
		if (!offer) return;
		this.shownEpisode = episode ?? null;
		this.open = true;
		this.group = null;
		// The latest opening's answer stands; a history that cannot be
		// read leaves the card's row alone, as a row whose match cannot
		// be read is left out of the group.
		const ask = ++this.#groupAsk;
		// Read ahead under the arrival's reads: the page going cancels it,
		// and a cancelled read sets no group.
		void this.kept((opts) => guessedGroup(this.#deps, offer, opts))
			.catch(() => [offer.entry.id])
			.then((group) => {
				if (ask === this.#groupAsk && this.offer === offer) this.group = group;
			});
		await this.loadFor(offer);
	}

	/** Close the picker, unless a link is in flight. */
	cancel(): void {
		if (!this.busy) this.open = false;
	}

	/** Link the card's guessed rows to the chosen show; whether it did.
	 *  True means saved, not saved for the visit now on screen: a link
	 *  that lands after a later arrival leaves that visit as it is. */
	async confirm(): Promise<boolean> {
		const offer = this.offer;
		const chosen = this.chosen;
		if (!offer || !chosen || this.busy) return false;
		const from = this.#pageId;
		// Where the user is as the link starts. One who leaves while it is
		// out is not brought back; a surface that moved on to another card
		// meanwhile keeps that card's state, and only reads its history
		// again.
		const at = this.#deps.here();
		const arrival = this.#arrival;
		const onSurface = () => this.#deps.here() === at;
		const stayed = () => arrival === this.#arrival && onSurface();
		this.busy = true;
		this.error = null;
		try {
			const rows = this.group ?? (await guessedGroup(this.#deps, offer));
			await this.#deps.link(offer.entry.id, rows, chosen.ref.id);
			// A picked show with no Sub or Dub of its own takes the one the
			// viewer was watching in ($lib/history/carry-audio.ts).
			carryAudio(this.#deps, offer.current.id, chosen.ref.id);
			this.#deps.notify(linkedNotice(chosen.ref.canonical_title));
			if (stayed()) this.#landed(chosen.ref.id, from);
			else if (onSurface()) this.#deps.refresh();
			return true;
		} catch (e) {
			if (stayed()) this.error = linkFailure(e);
			else this.#deps.notify({ kind: 'error', message: linkFailure(e) });
			return false;
		} finally {
			this.busy = false;
		}
	}

	/** A link to `to` made and answered on the page `from`, the visit
	 *  still on screen: the page is opened again without the card's row,
	 *  since the row is linked and there is nothing left to offer, nor
	 *  room to hold. */
	#landed(to: string, from: string): void {
		this.open = false;
		this.offer = null;
		if (to === from) this.#deps.refresh();
		this.#deps.showPage(to, null);
	}
}
