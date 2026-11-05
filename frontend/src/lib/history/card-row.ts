/**
 * The history line a Continue card stands for.
 *
 * Continue Watching shows the rows guessed as one show as one card, the
 * newer row's ($lib/history/dedupe.ts), for its progress. A guessed
 * card's play can land on another listing and write a row of its own
 * there: JoJo's 2000 card, guessed as the 2012 series, writes the 2012
 * listing's row, later than the card's own. The backend names, beside
 * that row, the card it came from (`guessed_from`). While that line is
 * still in the history and shows as the same show, the card stands for
 * it: the pill, the failed resume and the player correct the line the
 * user clicked, not the 2012 series' own history. Once the line is
 * linked to another show, each row is a card of its own. A shown row
 * that records its own show — played from that show's page — is the
 * user's, and the card stands for it.
 */

import type { HistoryEntry } from '$lib/api';

export function cardRow(
	shown: HistoryEntry,
	rows: readonly HistoryEntry[],
	matches: Readonly<Record<string, { id: string } | null | undefined>>,
	/** Whether a row's match is only a guess. A shown row that records
	 *  its own show is the user's, and stands for itself. Omitted, every
	 *  row counts as one. */
	isGuess: (id: string) => boolean = () => true
): string {
	const from = shown.guessed_from;
	if (!from || from === shown.id || !isGuess(shown.id)) return shown.id;
	const show = matches[shown.id]?.id;
	if (show === undefined || matches[from]?.id !== show) return shown.id;
	return rows.some((r) => r.id === from) ? from : shown.id;
}
