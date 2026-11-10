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
 * And whatever was still on its way about the former show — the first
 * load's probe, an earlier rematch — lands without putting it back. A
 * read whose list a remove or clear overtook reads again.
 *
 * The component holds the state; this decides what happens to it.
 */

import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { rereadLinked } from './reread-linked';

export interface CardRelinkDeps {
	/** Whether a link offer still stands for `row`, shown as `matchId`;
	 *  null when a later check overtook this one. */
	offerStands: (row: string, matchId: string) => Promise<boolean | null>;
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
	/** How many removes and clears have landed on the cards. */
	writes?: () => number;
}

export interface CardRelink {
	/** Whether the card for `row`, shown as `matchId`, is still a guess.
	 *  A row that is not settles, and the cards are read again; a check
	 *  a later one overtook answers no and settles nothing. */
	stillGuessed: (row: string, matchId: string) => Promise<boolean>;
	/** Reads the cards again where they stand. */
	reread: () => Promise<void>;
	/** A view of the rows as they are matched now: it answers, for a
	 *  row, whether the row has not been matched again since — and no for
	 *  a row whose rematch was still on its way when the view was taken. An answer
	 *  asked for before — the first load's probe — is about the show the
	 *  row was then, and is dropped once the view says no. */
	since: () => (row: string) => boolean;
}

export function createCardRelink(deps: CardRelinkDeps): CardRelink {
	let reads: Promise<unknown> = Promise.resolve();
	// How many times each row has been matched again. Only the latest
	// rematch of a row publishes: a link's answer landing after its
	// undo's is about a show the row no longer records.
	const rematches = new Map<string, number>();
	const count = (row: string) => rematches.get(row) ?? 0;
	// Rows whose latest rematch has not landed yet: they are leaving the
	// show they are shown as.
	const leaving = new Set<string>();

	// A row matched to another show forgets the former one's cap, then
	// takes its new match.
	const rematch = (h: HistoryEntry) => {
		const ask = count(h.id) + 1;
		rematches.set(h.id, ask);
		leaving.add(h.id);
		const settle = (match: KitsuAnimeRef | null) => {
			if (count(h.id) !== ask) return;
			leaving.delete(h.id);
			deps.forget(h.id);
			deps.publish(h.id, match);
		};
		void deps.resolveMatch(h).then(settle, () => settle(null));
	};

	const writes = deps.writes ?? (() => 0);
	// A list that went out before a remove or clear landed answers with
	// the cards that write took away: read again rather than put them back.
	const readOnce = async (): Promise<void> => {
		const at = writes();
		// Matched again only once the read is known to stand.
		const changed: HistoryEntry[] = [];
		const next = await rereadLinked(deps.current(), {
			list: deps.list,
			order: deps.order,
			rematch: (h) => changed.push(h)
		});
		if (writes() !== at) return readOnce();
		for (const h of changed) rematch(h);
		deps.setHistory(next);
	};

	const reread = () => {
		const run = reads.then(readOnce);
		reads = run.catch(() => {});
		return run;
	};

	return {
		stillGuessed: async (row, matchId) => {
			const stands = await deps.offerStands(row, matchId);
			if (stands) return true;
			// A later check overtook this one, and answers for itself: this
			// card is not known to be settled.
			if (stands === null) return false;
			deps.settled.add(row);
			void reread().catch(() => {});
			return false;
		},
		reread,
		since: () => {
			const taken = new Map(rematches);
			// A row being matched again when the view is taken is already
			// another show: a click on it then is on the one it is leaving.
			const left = new Set(leaving);
			return (row) => !left.has(row) && count(row) === (taken.get(row) ?? 0);
		}
	};
}
