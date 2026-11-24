/**
 * What Home sends the backend to read its Continue rail's hand-offs
 * (`$lib/api` `watchOrderRail`): what only Home has read so far. The
 * backend decides from it which rows ask for a hand-off and which rows
 * show one; this only gathers it.
 *
 * - each row's Kitsu match, as far as the hand-off reads it, and the
 *   episode count its availability probe found, once it answers;
 * - whether the player kept a resume position for the row's last
 *   episode (`leftPartWay`), which only the renderer's storage holds;
 * - every history row with its match id, a row still being matched
 *   left out, so the backend knows the played set only once it is
 *   whole;
 * - the hand-offs read so far, by the Kitsu id that asked.
 */
import type {
	AskRow,
	Handoff,
	HistoryEntry,
	KitsuAnimeRef,
	PlayedRow,
	RailRequest
} from '$lib/api';
import { parseHistoryEpisode } from '$lib/play/episode-number';

type Match = Pick<KitsuAnimeRef, 'id' | 'status' | 'episode_count'>;

/** One rail row as the hand-off's ask reads it. */
export function askRow(
	row: Pick<HistoryEntry, 'id' | 'ep_no'>,
	match: Match | null | undefined,
	playableCount: number | null | undefined,
	leftPartWay: (kitsuId: string, episode: number) => boolean
): AskRow {
	const episode = parseHistoryEpisode(row.ep_no);
	return {
		row_id: row.id,
		matched: match
			? { id: match.id, status: match.status, episode_count: match.episode_count }
			: null,
		ep_no: row.ep_no,
		playable_count: playableCount ?? null,
		left_part_way: match != null && episode !== null && leftPartWay(match.id, episode)
	};
}

/**
 * The rail's request, or null for a rail with no rows, which has
 * nothing to ask. `rows` is the rail in order; `history` every row.
 */
export function railRequest(
	rows: readonly Pick<HistoryEntry, 'id' | 'ep_no'>[],
	history: readonly Pick<HistoryEntry, 'id' | 'kitsu_id'>[],
	matches: Readonly<Record<string, Match | null | undefined>>,
	playableCounts: Readonly<Record<string, number | undefined>>,
	series: Readonly<Record<string, Handoff | null>>,
	leftPartWay: (kitsuId: string, episode: number) => boolean
): RailRequest | null {
	if (rows.length === 0) return null;
	const played: PlayedRow[] = history.map((r) => ({ id: r.id, kitsu_id: r.kitsu_id ?? null }));
	const matchIds: Record<string, string | null> = {};
	for (const r of history) {
		const match = matches[r.id];
		if (match !== undefined) matchIds[r.id] = match?.id ?? null;
	}
	return {
		rows: rows.map((r) => askRow(r, matches[r.id], playableCounts[r.id], leftPartWay)),
		history: played,
		matches: matchIds,
		series: { ...series }
	};
}
