/**
 * The show page's link session: the quiet offer a guessed Continue
 * card's page makes, the picker it opens, the link and its undo.
 *
 * The page hands it what the app provides ($lib/history/link-offer.ts)
 * and three effects of its own — a notice, a navigation, a refresh of
 * what it reads from the history — and renders its state. Arriving on
 * a page decides the offer; a later arrival outdates an earlier one
 * still out. Linking to the page's own show refreshes the page; to
 * another show, it goes there. An undo gives the rows back and returns
 * to the page the link was made from, which then offers it again.
 */

import { m } from '$lib/paraglide/messages';
import type { LinkOutcome } from '$lib/api';
import { kitsuEntryGone } from './match';
import { resolveHistoryEntry } from './resolve';
import { linkWarnings, type LinkCandidate, type LinkWarning } from './link-candidates';
import {
	confirmLink,
	linkOffer,
	loadCandidates,
	searchCandidates,
	undoLink,
	type LinkDeps,
	type LinkOffer
} from './link-offer';

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

export class LinkSession {
	offer = $state<LinkOffer | null>(null);
	open = $state(false);
	candidates = $state<LinkCandidate[] | null>(null);
	failed = $state(false);
	chosen = $state<LinkCandidate | null>(null);
	busy = $state(false);
	error = $state<string | null>(null);
	/** Whether the latest arrival's offer is decided. */
	settled = $state(true);
	/** The history rows the card stands for, once worked out. */
	group = $state<string[] | null>(null);

	#deps: LinkSessionDeps;
	#arrival = 0;
	#pageId = '';

	constructor(deps: LinkSessionDeps) {
		this.#deps = deps;
	}

	/** What the chosen show would be worth a second look for. */
	get warnings(): LinkWarning[] {
		return this.offer && this.chosen ? linkWarnings(this.offer.entry, this.chosen.ref) : [];
	}

	/** The episode the card's row is at, as the card shows it. */
	get episode(): string {
		return this.offer ? String(resolveHistoryEntry(this.offer.entry, null).displayEpisode) : '';
	}

	/** The page `pageId` was opened, from the guessed card's row `row`
	 *  or from anywhere else (null). */
	async arrive(row: string | null, pageId: string): Promise<void> {
		const arrival = ++this.#arrival;
		this.#pageId = pageId;
		this.offer = null;
		this.open = false;
		const offer = await linkOffer(this.#deps, row, pageId);
		if (arrival === this.#arrival) this.offer = offer;
	}

	/** Open the picker on the offer's candidates. */
	async start(): Promise<void> {
		const offer = this.offer;
		if (!offer) return;
		this.open = true;
		this.chosen = null;
		this.error = null;
		this.failed = false;
		this.candidates = null;
		const found = await loadCandidates(this.#deps, offer);
		if (this.open && this.offer === offer) this.candidates = found;
	}

	/** The candidates a typed search answers with. */
	async search(query: string): Promise<void> {
		const offer = this.offer;
		if (!offer) return;
		this.candidates = null;
		this.failed = false;
		try {
			const found = await searchCandidates(this.#deps, offer, query);
			if (this.offer === offer) this.candidates = found;
		} catch {
			if (this.offer === offer) {
				this.candidates = [];
				this.failed = true;
			}
		}
	}

	/** Pick a show, or go back to the list (null). */
	choose(candidate: LinkCandidate | null): void {
		this.chosen = candidate;
		this.error = null;
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
			const outcome = await confirmLink(this.#deps, offer, chosen.ref.id);
			this.open = false;
			this.offer = null;
			this.#deps.notify({
				kind: 'success',
				message: m.detail_link_done_toast({ title: chosen.ref.canonical_title }),
				actionLabel: m.detail_link_undo(),
				onAction: () => void this.undo(outcome, from, offer.entry.id)
			});
			if (chosen.ref.id === from) this.#deps.refresh();
			else this.#deps.showPage(chosen.ref.id, null);
		} catch (e) {
			this.error = kitsuEntryGone(e) ? m.detail_link_failed_gone() : m.detail_link_failed();
		} finally {
			this.busy = false;
		}
	}

	/** Undo `outcome`, made on the page `from` for the card row `row`. */
	async undo(outcome: LinkOutcome, from: string, row: string): Promise<void> {
		try {
			const all = await undoLink(this.#deps, outcome);
			this.#deps.notify(
				all
					? { kind: 'info', message: m.detail_link_undone_toast() }
					: { kind: 'error', message: m.detail_link_undo_kept_toast() }
			);
		} catch {
			this.#deps.notify({ kind: 'error', message: m.detail_link_failed() });
			return;
		}
		this.#deps.refresh();
		this.#deps.showPage(from, row);
	}
}
