/**
 * The link picker's state: the offer it is for, the candidates, the
 * typed searches and the show picked — split from
 * $lib/history/link-session.svelte.ts, which extends it, for the
 * per-file complexity bar. Each load or search is numbered, and only
 * the latest one's answer is shown.
 *
 * The offer's reads and the picker's searches are reads of the page the
 * picker is on ($lib/navigation/visit): each takes the signal of the
 * arrival that began them, which the next arrival or the page going
 * cancels, and nothing a cancelled one answers is shown.
 */

import { continueEpisode } from '$lib/play/next-episode';
import { visitSeries, type Visit } from '$lib/navigation/visit';
import type { LinkCandidate, LinkWarning } from '$lib/api';
import { loadCandidates, searchCandidates, type LinkDeps, type LinkOffer } from './link-offer';

export class LinkPicker {
	offer = $state<LinkOffer | null>(null);
	candidates = $state<LinkCandidate[] | null>(null);
	failed = $state(false);
	chosen = $state<LinkCandidate | null>(null);
	error = $state<string | null>(null);
	/** The episode the surface the picker opened from offers for the
	 *  card — what its card, breadcrumb or player shows — or null when
	 *  it named none. */
	protected shownEpisode = $state<number | null>(null);

	#pickerDeps: LinkDeps;
	#query = 0;
	#reads = visitSeries();
	#visit: Visit | null = null;

	constructor(deps: LinkDeps) {
		this.#pickerDeps = deps;
	}

	/** What the chosen show would be worth a second look for, as the
	 *  backend answered with the candidates. */
	get warnings(): LinkWarning[] {
		return this.offer && this.chosen ? this.chosen.warnings : [];
	}

	/** The episode the card offers, as the card, the breadcrumb and the
	 *  player show it: the one the surface named, else the one a
	 *  Continue click plays for the row's guessed show. */
	get episode(): string {
		return this.offer ? String(this.#episodeNumber(this.offer)) : '';
	}

	#episodeNumber(offer: LinkOffer): number {
		return (
			this.shownEpisode ??
			continueEpisode(offer.current.id, offer.entry.ep_no, offer.current.episode_count)
		);
	}

	/** Begin the reads of an arrival, cancelling those of the one before. */
	protected beginReads(): Visit {
		this.#visit = this.#reads.next();
		return this.#visit;
	}

	/** `read(opts)`, with the signal of the arrival's reads, settled
	 *  only while that arrival's reads last. */
	protected kept<T>(read: (opts: { signal?: AbortSignal }) => Promise<T>): Promise<T> {
		const visit = this.#visit;
		return visit ? visit.keep(read({ signal: visit.signal })) : read({});
	}

	/** Show the offer's own candidates. */
	protected async loadFor(offer: LinkOffer): Promise<void> {
		this.chosen = null;
		this.error = null;
		this.failed = false;
		this.candidates = null;
		const query = ++this.#query;
		const episode = this.#episodeNumber(offer);
		const found = await this.kept((opts) => loadCandidates(this.#pickerDeps, offer, episode, opts));
		if (query !== this.#query || this.offer !== offer) return;
		this.candidates = found.candidates;
		this.failed = found.failed;
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
			const episode = this.#episodeNumber(offer);
			const found = await this.kept((opts) =>
				searchCandidates(this.#pickerDeps, offer, query, episode, opts)
			);
			if (current()) this.candidates = found;
		} catch {
			if (current()) {
				this.candidates = [];
				this.failed = true;
			}
		}
	}

	/** The page went: the reads of the arrival still out are cancelled. */
	endReads(): void {
		this.#reads.end();
	}

	/** Pick a show, or go back to the list (null). */
	choose(candidate: LinkCandidate | null): void {
		this.chosen = candidate;
		this.error = null;
	}
}
