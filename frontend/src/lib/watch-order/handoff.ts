/**
 * Continue Watching's hand-off (docs/proposals/franchise-watch-order.md,
 * option b): once an entry's finale has been played through, its card
 * moves on to the entry that comes next, the way the player's Up next
 * card names it, and opens that entry's show page.
 *
 * It moves on only where exactly one entry comes next, has aired, and
 * has no history row of its own; in every other case the row keeps
 * its ordinary card. Nothing here asks the provider.
 */
import type { WatchOrder } from './order';
import type { FranchiseData, WatchEntry } from './types';
import { parseHistoryEpisode } from '$lib/play/episode-number';
import { runOf } from './strip-model';
import { isFinale, upNextFor } from './up-next';

/** A Continue card moved on: the entry played through, and the one after it. */
export interface Handoff {
	readonly finished: WatchEntry;
	readonly next: WatchEntry;
	/** The titles of the finished entry's series, which its short title is cut against. */
	readonly titles: readonly string[];
}

/** What a Continue row's Kitsu match says about the entry it plays. */
export interface HandoffMatch {
	readonly id: string;
	/** Kitsu's status: `finished` once the entry has finished airing. */
	readonly status: string | null;
	readonly episode_count: number | null;
}

/**
 * The Kitsu id whose series a row's hand-off needs, or null: a matched
 * row whose last episode is the finale of an entry that has finished
 * airing, and was not left part-way. Only such a row asks for its
 * series, so no other card costs a request. `playableCount` is the
 * episode count the row's availability probe found, null until it
 * answers.
 */
export function handoffAsk(
	match: HandoffMatch | null | undefined,
	epNo: string,
	playableCount: number | null,
	leftPartWay: (kitsuId: string, episode: number) => boolean
): string | null {
	void playableCount;
	const episode = parseHistoryEpisode(epNo);
	if (!match || episode === null) return null;
	if (!isFinale({ status: match.status, episodes: match.episode_count }, episode)) return null;
	return leftPartWay(match.id, episode) ? null : match.id;
}

/**
 * The hand-off from `id`, the entry a row played through, or null: the
 * series not read (or read for another id), the settings not settled
 * (`order` null), `id` outside the series, a fork, an entry not aired
 * yet, nothing after it, or an entry after it that `played` holds.
 */
export function handoffFrom(
	franchise: FranchiseData | null,
	id: string,
	order: WatchOrder | null,
	today: string,
	played: ReadonlySet<string> | null
): Handoff | null {
	if (played === null) return null;
	if (franchise === null || order === null || franchise.anchor !== id) return null;
	const run = runOf(franchise);
	const entry = run?.entries.find((e) => e.id === id);
	if (!run || !entry) return null;
	const up = upNextFor(run, id, order, today);
	if (up.kind !== 'one' || played.has(up.entry.id)) return null;
	return { finished: entry, next: up.entry, titles: run.entries.map((e) => e.title) };
}

/** A history row as the played set reads it. */
export interface PlayedRow {
	readonly id: string;
	readonly kitsu_id?: string;
}

/**
 * The Kitsu ids with a history row: each row's recorded id, else the
 * id its match found (rows written before history recorded one).
 */
export function playedIds(
	rows: readonly PlayedRow[],
	matches: Readonly<Record<string, { readonly id: string } | null | undefined>>
): Set<string> | null {
	const out = new Set<string>();
	for (const row of rows) {
		const id = row.kitsu_id ?? matches[row.id]?.id;
		if (id !== undefined) out.add(id);
	}
	return out;
}

/**
 * Each rail row's hand-off, keyed by row id. `asks` is the rail in
 * order, each row with its `handoffAsk`. Two rows that would move on
 * to the same entry show it once: the first in the rail moves on, the
 * other keeps its ordinary card.
 */
export function railHandoffs(
	asks: readonly (readonly [rowId: string, kitsuId: string | null])[],
	from: (kitsuId: string) => Handoff | null
): Map<string, Handoff> {
	const out = new Map<string, Handoff>();
	const shown = new Set<string>();
	for (const [rowId, kitsuId] of asks) {
		const handoff = kitsuId === null ? null : from(kitsuId);
		if (!handoff || shown.has(handoff.next.id)) continue;
		shown.add(handoff.next.id);
		out.set(rowId, handoff);
	}
	return out;
}
