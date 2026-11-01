/**
 * Whether a show page offers to link the guessed card it was opened
 * from, and the flow that does it.
 *
 * The page offers it only when it was opened from a guessed Continue
 * card ($lib/history/guessed-context.ts), the card's row is still in
 * the history, and Continue Watching's match for the row
 * (`resolveKitsuMatchWithTrust`) is still a guess of this very show —
 * a row with no recorded id, or one whose recorded entry Kitsu has
 * deleted. Anything else, and the page says nothing.
 *
 * A link covers every guessed row the card stands for: the rows
 * Continue Watching shows as one card because they are guessed as the
 * same show. Linking the shown row alone would leave a hidden sibling
 * guessed as the old show, and it would surface as a card of its own.
 * A row Continue Watching trusts — one whose recorded entry Kitsu
 * still serves, or one a play's mapping names — stays out.
 */

import type { HistoryEntry, KitsuAnimeRef, LinkedRow, LinkOutcome, UnlinkOutcome } from '$lib/api';
import { resolveHistoryEntry } from './resolve';
import { mergeCandidates, slugSearchTerm, type LinkCandidate } from './link-candidates';

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

/** Whether a row is matched to `pageId` by a guess. */
async function guessedAs(
	deps: LinkDeps,
	entry: HistoryEntry,
	pageId: string
): Promise<KitsuAnimeRef | null> {
	const { match, trusted } = await deps.resolve(entry);
	return match && !trusted && match.id === pageId ? match : null;
}

/** The offer the show page `pageId` makes for the guessed card's row
 *  `row`, or null when it makes none. A failure to read makes none. */
export async function linkOffer(
	deps: LinkDeps,
	row: string | null,
	pageId: string
): Promise<LinkOffer | null> {
	if (!row) return null;
	try {
		const entry = (await deps.history()).find((e) => e.id === row);
		if (!entry) return null;
		const current = await guessedAs(deps, entry, pageId);
		return current ? { entry, current } : null;
	} catch {
		return null;
	}
}

/** The candidates for the offer's row: both searches, each allowed to
 *  fail on its own. */
export async function loadCandidates(deps: LinkDeps, offer: LinkOffer): Promise<LinkCandidate[]> {
	const title = resolveHistoryEntry(offer.entry, null).searchTitle;
	const slug = slugSearchTerm(offer.entry.id);
	const quiet = (q: string | null) => (q ? deps.search(q).catch(() => []) : Promise.resolve([]));
	const [titleHits, slugHits] = await Promise.all([quiet(title), quiet(slug)]);
	return mergeCandidates(offer.current, titleHits, slugHits);
}

/** The candidates a typed search answers with. Its failure is the
 *  caller's to show. */
export async function searchCandidates(
	deps: LinkDeps,
	offer: LinkOffer,
	query: string
): Promise<LinkCandidate[]> {
	const hits = await deps.search(query.trim());
	return mergeCandidates(null, hits, []).map((c) => ({
		...c,
		current: c.ref.id === offer.current.id
	}));
}

/** Every guessed row the offer's card stands for: its own, and each
 *  other row Continue Watching guesses as the same show. The rows are
 *  asked side by side, as Continue Watching asks them; a row whose
 *  match cannot be read is left out. */
export async function guessedGroup(deps: LinkDeps, offer: LinkOffer): Promise<string[]> {
	const others = (await deps.history()).filter((e) => e.id !== offer.entry.id);
	const same = await Promise.all(
		others.map((e) => guessedAs(deps, e, offer.current.id).catch(() => null))
	);
	return [offer.entry.id, ...others.filter((_, i) => same[i] !== null).map((e) => e.id)];
}

/** Link the offer's card, and every guessed row it stands for, to
 *  `kitsuId`. */
export async function confirmLink(
	deps: LinkDeps,
	offer: LinkOffer,
	kitsuId: string
): Promise<LinkOutcome> {
	return deps.link(await guessedGroup(deps, offer), kitsuId);
}

/** Undo `outcome`; true when every row it linked got its id back. */
export async function undoLink(deps: LinkDeps, outcome: LinkOutcome): Promise<boolean> {
	const undone = await deps.unlink(outcome.kitsu_id, outcome.linked);
	return undone.kept.length === 0;
}
