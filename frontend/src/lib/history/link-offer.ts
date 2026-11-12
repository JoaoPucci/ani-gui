/**
 * The show page's, the player's and Home's way to correct a guessed
 * Continue card's line: the reads it makes, and what it asks the
 * backend.
 *
 * Every decision here is the backend's: whether a surface asks about
 * the line, offers it again or finds a kept pick
 * (`commands/link_verdict.rs`), which shows the picker lists and what
 * each is worth a second look for (`commands/link_rules.rs`), and which
 * rows a correction covers (`link_verdict.rs` `group_members`, then the
 * card's own listing, `history_link_plan.rs`). What the renderer brings
 * is what only it holds: Continue Watching's match for each row
 * (`resolveKitsuMatchWithTrust`), the episode its surface offers, and
 * its session's facts — whether the session was opened from the line
 * as a guess and that guessed play stands ($lib/history/guess-plays.ts),
 * and whether the line was picked on the player in this run
 * ($lib/history/guess-slip.ts).
 */

import type {
	CandidatesRequest,
	HistoryEntry,
	KitsuAnimeRef,
	LinkCandidate,
	LinkGroupRequest,
	LinkOutcome,
	LinkVerdict,
	ReadOptions,
	RowMatch,
	VerdictRequest
} from '$lib/api';
import { readOpts } from './read-opts';
import { resolveHistoryEntry } from './resolve';

/** What the page needs from the app. */
export interface LinkDeps {
	history: (opts?: ReadOptions) => Promise<HistoryEntry[]>;
	/** Continue Watching's match for a row, with whether it is trusted. */
	resolve: (
		entry: HistoryEntry,
		opts?: ReadOptions
	) => Promise<{ match: KitsuAnimeRef | null; trusted: boolean }>;
	/** The backend's candidates for a row ($lib/api `historyLinkCandidates`). */
	candidates: (request: CandidatesRequest, opts?: ReadOptions) => Promise<LinkCandidate[]>;
	/** Whether a surface asks about a line ($lib/api `historyLinkVerdict`). */
	verdict: (request: VerdictRequest, opts?: ReadOptions) => Promise<LinkVerdict>;
	/** Link `rows` from the card whose row is `card`. */
	link: (card: string, rows: string[], kitsuId: string) => Promise<LinkOutcome>;
	/** The rows a link from a card would write ($lib/api `historyLinkGroup`). */
	group: (request: LinkGroupRequest) => Promise<string[]>;
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

/** The picker's candidates for the offer's row, the card offering
 *  `episode`. A backend that cannot answer says so (`failed`), and
 *  leaves the card's own show to pick, as a picker whose searches both
 *  failed always did; what it would be worth a look for is not known. */
export async function loadCandidates(
	deps: LinkDeps,
	offer: LinkOffer,
	episode: number,
	opts?: ReadOptions
): Promise<{ candidates: LinkCandidate[]; failed: boolean }> {
	try {
		const found = await deps.candidates(request(offer, episode, null), ...readOpts(opts ?? {}));
		return { candidates: found, failed: false };
	} catch {
		return { candidates: [{ ref: offer.current, current: true, warnings: [] }], failed: true };
	}
}

/** The candidates a typed search answers with. Its failure is the
 *  caller's to show. */
export function searchCandidates(
	deps: LinkDeps,
	offer: LinkOffer,
	query: string,
	episode: number,
	opts?: ReadOptions
): Promise<LinkCandidate[]> {
	return deps.candidates(request(offer, episode, query), ...readOpts(opts ?? {}));
}

function request(offer: LinkOffer, episode: number, query: string | null): CandidatesRequest {
	return {
		row: offer.entry.id,
		// The title as Continue Watching searches it.
		title: resolveHistoryEntry(offer.entry, null).searchTitle,
		current: offer.current,
		query,
		episode
	};
}

/** Every row the offer's card stands for, as the backend reads them
 *  from each row's match. The rows are matched side by side, as
 *  Continue Watching matches them; a row whose match cannot be read
 *  names none. The picker reads it ahead with its arrival's signal
 *  (`opts`); a link that finds none ready reads it with none, since a
 *  link still out when the page goes is saved all the same. */
export async function guessedGroup(
	deps: LinkDeps,
	offer: LinkOffer,
	opts?: ReadOptions
): Promise<string[]> {
	const read = readOpts(opts ?? {});
	const others = (await deps.history(...read)).filter((e) => e.id !== offer.entry.id);
	const rows: RowMatch[] = await Promise.all(
		others.map((e) =>
			deps.resolve(e, ...read).then(
				({ match, trusted }) => ({ id: e.id, show: match?.id ?? null, trusted }),
				() => ({ id: e.id, show: null, trusted: false })
			)
		)
	);
	return deps.group({
		card: offer.entry.id,
		show: offer.current.id,
		saved: offer.saved ?? null,
		rows
	});
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
