/**
 * The link picker's state: the offer it is for, the candidates, the
 * typed searches and the show picked — split from
 * $lib/history/link-session.svelte.ts, which extends it, for the
 * per-file complexity bar. Each load or search is numbered, and only
 * the latest one's answer is shown.
 */

import { resolveHistoryEntry } from './resolve';
import { linkWarnings, type LinkCandidate, type LinkWarning } from './link-candidates';
import { loadCandidates, searchCandidates, type LinkDeps, type LinkOffer } from './link-offer';

export class LinkPicker {
	offer = $state<LinkOffer | null>(null);
	candidates = $state<LinkCandidate[] | null>(null);
	failed = $state(false);
	chosen = $state<LinkCandidate | null>(null);
	error = $state<string | null>(null);

	#pickerDeps: LinkDeps;
	#query = 0;

	constructor(deps: LinkDeps) {
		this.#pickerDeps = deps;
	}

	/** What the chosen show would be worth a second look for. */
	get warnings(): LinkWarning[] {
		return this.offer && this.chosen ? linkWarnings(this.offer.entry, this.chosen.ref) : [];
	}

	/** The episode the card's row is at, as the card shows it. */
	get episode(): string {
		return this.offer ? String(resolveHistoryEntry(this.offer.entry, null).displayEpisode) : '';
	}

	/** Show the offer's own candidates. */
	protected async loadFor(offer: LinkOffer): Promise<void> {
		this.chosen = null;
		this.error = null;
		this.failed = false;
		this.candidates = null;
		const query = ++this.#query;
		const found = await loadCandidates(this.#pickerDeps, offer);
		if (query === this.#query && this.offer === offer) this.candidates = found;
	}

	/** The candidates a typed search answers with. */
	async search(query: string): Promise<void> {
		const offer = this.offer;
		if (!offer) return;
		this.candidates = null;
		this.failed = false;
		const asked = ++this.#query;
		const current = () => asked === this.#query && this.offer === offer;
		try {
			const found = await searchCandidates(this.#pickerDeps, offer, query);
			if (current()) this.candidates = found;
		} catch {
			if (current()) {
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
}
