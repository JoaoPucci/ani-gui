/**
 * The show page's link session: the quiet offer a guessed Continue
 * card's page makes, the picker it opens, the link and its undo. Not
 * wired yet.
 */

import type { LinkOutcome } from '$lib/api';
import type { LinkCandidate, LinkWarning } from './link-candidates';
import type { LinkDeps, LinkOffer } from './link-offer';

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
}

export class LinkSession {
	offer = $state<LinkOffer | null>(null);
	open = $state(false);
	candidates = $state<LinkCandidate[] | null>(null);
	failed = $state(false);
	chosen = $state<LinkCandidate | null>(null);
	busy = $state(false);
	error = $state<string | null>(null);

	constructor(deps: LinkSessionDeps) {
		void deps;
	}

	get warnings(): LinkWarning[] {
		return [];
	}

	get episode(): string {
		return '';
	}

	async arrive(row: string | null, pageId: string): Promise<void> {
		void row;
		void pageId;
	}

	async start(): Promise<void> {}

	async search(query: string): Promise<void> {
		void query;
	}

	choose(candidate: LinkCandidate | null): void {
		void candidate;
	}

	cancel(): void {}

	async confirm(): Promise<void> {}

	async undo(outcome: LinkOutcome, from: string, row: string): Promise<void> {
		void outcome;
		void from;
		void row;
	}
}
