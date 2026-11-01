/**
 * Whether a show page offers to link the guessed card it was opened
 * from, and the flow that does it. Not wired yet.
 */

import type { HistoryEntry, KitsuAnimeRef, LinkedRow, LinkOutcome, UnlinkOutcome } from '$lib/api';
import type { LinkCandidate } from './link-candidates';

/** What the page needs from the app. */
export interface LinkDeps {
	history: () => Promise<HistoryEntry[]>;
	/** Continue Watching's match for a row, with whether it is trusted. */
	resolve: (entry: HistoryEntry) => Promise<{ match: KitsuAnimeRef | null; trusted: boolean }>;
	search: (query: string) => Promise<KitsuAnimeRef[]>;
	link: (rows: string[], kitsuId: string) => Promise<LinkOutcome>;
	unlink: (kitsuId: string, rows: LinkedRow[]) => Promise<UnlinkOutcome>;
}

/** The offer: the card's row, and the show it is guessed as. */
export interface LinkOffer {
	entry: HistoryEntry;
	current: KitsuAnimeRef;
}

export async function linkOffer(
	deps: LinkDeps,
	row: string | null,
	pageId: string
): Promise<LinkOffer | null> {
	void deps;
	void row;
	void pageId;
	return null;
}

export async function loadCandidates(deps: LinkDeps, offer: LinkOffer): Promise<LinkCandidate[]> {
	void deps;
	void offer;
	return [];
}

export async function searchCandidates(
	deps: LinkDeps,
	offer: LinkOffer,
	query: string
): Promise<LinkCandidate[]> {
	void deps;
	void offer;
	void query;
	return [];
}

export async function guessedGroup(deps: LinkDeps, offer: LinkOffer): Promise<string[]> {
	void deps;
	return [offer.entry.id];
}

export async function confirmLink(
	deps: LinkDeps,
	offer: LinkOffer,
	kitsuId: string
): Promise<LinkOutcome> {
	void deps;
	void offer;
	return { kitsu_id: kitsuId, linked: [] };
}

export async function undoLink(deps: LinkDeps, outcome: LinkOutcome): Promise<boolean> {
	void deps;
	void outcome;
	return false;
}
