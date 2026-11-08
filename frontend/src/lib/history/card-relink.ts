/**
 * Home's guessed Continue cards after a link, made from one of them or
 * anywhere else.
 *
 * A card's guess is checked again before its picker opens: a link made
 * elsewhere, or a play that recorded the show, may have settled it
 * since the cards were read. A settled row loses its pill rather than
 * open an empty picker, and the cards are read again. Reading again
 * lists the history in Continue order and matches again every row whose
 * recorded show changed ($lib/history/reread-linked.ts), publishing
 * what each row is now. Reads run one after another, each against the
 * history the one before left: a link and its undo, close together,
 * both reach the cards. A row matched to another show forgets what was
 * kept for the former one: a cap a failed resume of a one-episode
 * special pinned as exact would hold the linked series at episode 1.
 *
 * The component holds the state; this decides what happens to it.
 */

import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { rereadLinked } from './reread-linked';

export interface CardRelinkDeps {
	/** Whether a link offer still stands for `row`, shown as `matchId`. */
	offerStands: (row: string, matchId: string) => Promise<boolean>;
	/** The rows found no longer guessed: their pill goes. */
	settled: { add: (row: string) => unknown };
	/** The history the cards show now. */
	current: () => readonly HistoryEntry[];
	list: () => Promise<HistoryEntry[]>;
	/** The order Continue Watching shows the rows in. */
	order: (rows: HistoryEntry[]) => HistoryEntry[];
	setHistory: (rows: HistoryEntry[]) => void;
	/** The row's match, as the first load resolves it. */
	resolveMatch: (entry: HistoryEntry) => Promise<KitsuAnimeRef | null>;
	/** Drops what was kept for the row's former show — the cap a click
	 *  pinned, the count a probe found. */
	forget: (row: string) => void;
	/** Publishes the row's match. A row whose match failed has none. */
	publish: (row: string, match: KitsuAnimeRef | null) => void;
}

export interface CardRelink {
	/** Whether the card for `row`, shown as `matchId`, is still a guess.
	 *  A row that is not settles, and the cards are read again. */
	stillGuessed: (row: string, matchId: string) => Promise<boolean>;
	/** Reads the cards again where they stand. */
	reread: () => Promise<void>;
}

export function createCardRelink(deps: CardRelinkDeps): CardRelink {
	let reads: Promise<unknown> = Promise.resolve();

	// A row matched to another show forgets the former one's cap, then
	// takes its new match.
	const rematch = (h: HistoryEntry) => {
		const settle = (match: KitsuAnimeRef | null) => {
			deps.forget(h.id);
			deps.publish(h.id, match);
		};
		void deps.resolveMatch(h).then(settle, () => settle(null));
	};

	const readOnce = async () => {
		const next = await rereadLinked(deps.current(), {
			list: deps.list,
			order: deps.order,
			rematch
		});
		deps.setHistory(next);
	};

	const reread = () => {
		const run = reads.then(readOnce);
		reads = run.catch(() => {});
		return run;
	};

	return {
		stillGuessed: async (row, matchId) => {
			if (await deps.offerStands(row, matchId)) return true;
			deps.settled.add(row);
			void reread().catch(() => {});
			return false;
		},
		reread
	};
}
