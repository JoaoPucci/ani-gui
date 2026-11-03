/**
 * The show page's link session: the quiet offer a guessed Continue
 * card's page makes, the picker it opens, the link and its undo.
 *
 * The page hands it what the app provides ($lib/history/link-offer.ts)
 * and three effects of its own — a notice, a navigation, a refresh of
 * what it reads from the history — and renders its state. Arriving on
 * a page decides the offer; a later arrival outdates an earlier one
 * still out. A link opens the chosen show's page without the card's
 * row, refreshing it first when it is the page the link was made on.
 * An undo gives the rows back and, while the user is still on the page
 * the link left them on, returns to the page it was made from, which
 * then offers it again ($lib/history/link-notices.ts).
 */

import type { LinkOutcome } from '$lib/api';
import { linkedNotice, linkFailure, undoAndReturn } from './link-notices';
import { LinkPicker } from './link-picker.svelte';
import { guessedGroup, linkOffer, type LinkDeps } from './link-offer';

/** A notice to the user, with an optional action. */
export interface LinkNotice {
	kind: 'success' | 'info' | 'error';
	message: string;
	actionLabel?: string;
	onAction?: () => void;
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

	constructor(deps: LinkSessionDeps) {
		super(deps);
		this.#deps = deps;
	}

	/** The page `pageId` was opened, from the guessed card's row `row`
	 *  or from anywhere else (null). */
	async arrive(row: string | null, pageId: string): Promise<void> {
		const arrival = ++this.#arrival;
		this.#pageId = pageId;
		this.offer = null;
		this.open = false;
		this.settled = false;
		const offer = await linkOffer(this.#deps, row, pageId);
		if (arrival !== this.#arrival) return;
		this.offer = offer;
		this.settled = true;
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
		void guessedGroup(this.#deps, offer)
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

	/** Link the card's guessed rows to the chosen show. */
	async confirm(): Promise<void> {
		const offer = this.offer;
		const chosen = this.chosen;
		if (!offer || !chosen || this.busy) return;
		const from = this.#pageId;
		this.busy = true;
		this.error = null;
		try {
			const rows = this.group ?? (await guessedGroup(this.#deps, offer));
			const outcome = await this.#deps.link(rows, chosen.ref.id);
			this.open = false;
			this.offer = null;
			this.#deps.notify(
				linkedNotice(
					chosen.ref.canonical_title,
					() => void this.undo(outcome, from, offer.entry.id)
				)
			);
			// The page is opened again without the card's row: the row is
			// linked, so there is nothing left to offer, nor room to hold.
			if (chosen.ref.id === from) this.#deps.refresh();
			this.#deps.showPage(chosen.ref.id, null);
		} catch (e) {
			this.error = linkFailure(e);
		} finally {
			this.busy = false;
		}
	}

	/** Undo `outcome`, made on the page `from` for the card row `row`
	 *  ($lib/history/link-notices.ts). */
	undo(outcome: LinkOutcome, from: string, row: string): Promise<void> {
		return undoAndReturn(this.#deps, outcome, from, row);
	}
}
