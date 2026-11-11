/**
 * Whether a show page offers to link the guessed card it was opened
 * from, and the flow that does it.
 *
 * The page offers it only when it was opened from a guessed Continue
 * card ($lib/history/guessed-context.ts), the card's row is still in
 * the history, and Continue Watching's match for the row
 * (`resolveKitsuMatchWithTrust`) is still a guess of this very show —
 * a row with no recorded id, or one whose recorded entry Kitsu has
 * deleted — or, for a surface whose session the guessed card opened
 * (`carried`), still this show and not one the user named. Anything
 * else, and the page says nothing.
 *
 * A link covers every guessed row the card stands for on the card's
 * own listing: the rows Continue Watching shows as one card because
 * they are guessed as the same show, less any another listing wrote.
 * Linking the shown row alone would leave a hidden sibling guessed as
 * the old show, and it would surface as a card of its own. A row
 * Continue Watching trusts — one whose recorded entry Kitsu still
 * serves, or one a play's mapping names — stays out, and so does a row
 * of another listing the same guess matches: JoJo's 2000 OVA card
 * plays the 2012 series, whose play writes the 2012 listing's row, and
 * that row is the 2012 series' history. Which rows share the card's
 * listing is the backend's to say (`history_link_plan.rs`): the
 * renderer never parses a slug.
 */

import type { HistoryEntry, KitsuAnimeRef, LinkOutcome, ReadOptions } from '$lib/api';
import { readOpts } from './read-opts';
import { resolveHistoryEntry } from './resolve';
import { mergeCandidates, type LinkCandidate } from './link-candidates';

/** What the page needs from the app. */
export interface LinkDeps {
	history: (opts?: ReadOptions) => Promise<HistoryEntry[]>;
	/** Continue Watching's match for a row, with whether it is trusted. */
	resolve: (
		entry: HistoryEntry,
		opts?: ReadOptions
	) => Promise<{ match: KitsuAnimeRef | null; trusted: boolean }>;
	search: (query: string, opts?: ReadOptions) => Promise<KitsuAnimeRef[]>;
	/** Link `rows` from the card whose row is `card`. */
	link: (card: string, rows: string[], kitsuId: string) => Promise<LinkOutcome>;
	/** The rows of `rows` a link from the card `card` would write. */
	group: (card: string, rows: string[]) => Promise<string[]>;
}

/** The offer: the card's row, and the show it is guessed as. For a
 *  row the user named a show for, the offer stays as the way to change
 *  it again ($lib/history/picked-offer.ts): `current` is then the show
 *  the surface shows, and `saved` the id the row records now. */
export interface LinkOffer {
	entry: HistoryEntry;
	current: KitsuAnimeRef;
	saved?: string;
}

/** Whether a row is matched to `pageId` by a guess. With `carried`,
 *  the surface's session was opened from the row as a guess, and keeps
 *  that verdict: the row is asked about while it shows `pageId` and the
 *  user named no show for it, trusted or not. */
async function guessedAs(
	deps: LinkDeps,
	entry: HistoryEntry,
	pageId: string,
	opts?: ReadOptions,
	carried = false
): Promise<KitsuAnimeRef | null> {
	const { match, trusted } = await deps.resolve(entry, ...readOpts(opts ?? {}));
	const guess = !trusted || (carried && entry.named !== true);
	return match && guess && match.id === pageId ? match : null;
}

/** The offer the show page `pageId` makes for the guessed card's row
 *  `row`, or null when it makes none. A failure to read makes none, and
 *  so does a read cancelled with the visit `opts` names.
 *
 *  `carried` is for a surface whose session a guessed card opened —
 *  the player, and the show page it goes up to. That session is the
 *  guess the card was clicked as for its whole life
 *  ($lib/play/play-origin.ts), so a row its own play has since
 *  recorded — a guess that landed on the card's own listing, which the
 *  backend records on the row, before the player reads it when the
 *  show's Kitsu detail is cached — is still asked about. Home judges
 *  each row as it reads it, and passes none. */
export async function linkOffer(
	deps: LinkDeps,
	row: string | null,
	pageId: string,
	opts?: ReadOptions,
	carried = false
): Promise<LinkOffer | null> {
	if (!row) return null;
	try {
		const entry = (await deps.history(...readOpts(opts ?? {}))).find((e) => e.id === row);
		if (!entry) return null;
		const current = await guessedAs(deps, entry, pageId, opts, carried);
		return current ? { entry, current } : null;
	} catch {
		return null;
	}
}

/** The candidates for the offer's row: both searches, each allowed to
 *  fail on its own. */
export async function loadCandidates(
	deps: LinkDeps,
	offer: LinkOffer,
	opts?: ReadOptions
): Promise<LinkCandidate[]> {
	const title = resolveHistoryEntry(offer.entry, null).searchTitle;
	// The words of the row's key, which the backend reads off it.
	const slug = offer.entry.slug_term ?? null;
	const quiet = (q: string | null) =>
		q ? deps.search(q, ...readOpts(opts ?? {})).catch(() => []) : Promise.resolve([]);
	const [titleHits, slugHits] = await Promise.all([quiet(title), quiet(slug)]);
	return mergeCandidates(offer.current, titleHits, slugHits);
}

/** The candidates a typed search answers with. Its failure is the
 *  caller's to show. */
export async function searchCandidates(
	deps: LinkDeps,
	offer: LinkOffer,
	query: string,
	opts?: ReadOptions
): Promise<LinkCandidate[]> {
	const hits = await deps.search(query.trim(), ...readOpts(opts ?? {}));
	return mergeCandidates(null, hits, []).map((c) => ({
		...c,
		current: c.ref.id === offer.current.id
	}));
}

/** Every guessed row the offer's card stands for: its own, and each
 *  other row Continue Watching guesses as the same show on the card's
 *  own listing. The rows are asked side by side, as Continue Watching
 *  asks them; a row whose match cannot be read is left out. For a card
 *  the user named a show for (`saved`), the rows are those recorded as
 *  that show: the naming moved them all, and no guess matches them now.
 *  The picker reads it ahead with its arrival's signal (`opts`); a link
 *  that finds none ready reads it with none, since a link still out when
 *  the page goes is saved all the same. */
export async function guessedGroup(
	deps: LinkDeps,
	offer: LinkOffer,
	opts?: ReadOptions
): Promise<string[]> {
	const read = readOpts(opts ?? {});
	const others = (await deps.history(...read)).filter((e) => e.id !== offer.entry.id);
	const saved = offer.saved;
	const same = await Promise.all(
		others.map((e) =>
			(saved === undefined
				? guessedAs(deps, e, offer.current.id, opts)
				: deps.resolve(e, ...read).then(({ match }) => (match?.id === saved ? match : null))
			).catch(() => null)
		)
	);
	const card = offer.entry.id;
	return deps.group(card, [card, ...others.filter((_, i) => same[i] !== null).map((e) => e.id)]);
}

/** Link the offer's card, and every guessed row it stands for, to
 *  `kitsuId`. */
export async function confirmLink(
	deps: LinkDeps,
	offer: LinkOffer,
	kitsuId: string
): Promise<LinkOutcome> {
	return deps.link(offer.entry.id, await guessedGroup(deps, offer), kitsuId);
}
