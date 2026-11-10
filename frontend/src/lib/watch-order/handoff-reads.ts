/**
 * The reads behind Continue Watching's hand-offs, however often the
 * rail re-renders: each Kitsu id's franchise, asked once per page, and
 * whether a history row resumes the entry a card would move on to,
 * asked once per id until the history changes. A read that fails
 * settles as null (the row keeps its ordinary card, with no error).
 *
 * Each read is handed a signal, aborted when the page goes (`stop`)
 * and, for the resume lookups, when forgetting drops their answers:
 * a franchise read holds its response open while the backend walks the
 * franchise, on the API lane every page shares (`$lib/backend/lanes`),
 * so a read whose answer nobody will use must not hold a connection.
 * For the same reason the series are walked a few at a time
 * (`HANDOFF_WALKS_AT_ONCE`), in the order the rows ask for them.
 */
import type { FranchiseData } from './types';

export interface FranchiseReads {
	/** Asks for `id`'s franchise, unless it was asked for already. */
	want(id: string): void;
	/** The page has gone: cancels every read still out, and hands on no more answers. */
	stop(): void;
}

/**
 * How many series walks Home keeps out at once. Chromium gives the API
 * lane six connections, the refill wait can hold one, and Home's own
 * calls need the rest: a Continue card's resume reads its row again
 * before it plays, and a removal writes. Two walks leave three once
 * Home's own first reads have answered.
 */
export const HANDOFF_WALKS_AT_ONCE = 2;

export function createFranchiseReads(
	read: (id: string, signal: AbortSignal) => Promise<FranchiseData>,
	settled: (id: string, franchise: FranchiseData | null) => void
): FranchiseReads {
	const asked = new Set<string>();
	const page = new AbortController();
	// Wanted ids not yet read, in the order they were wanted.
	const waiting: string[] = [];
	let out = 0;
	const hand = (id: string, franchise: FranchiseData | null) => {
		out -= 1;
		if (page.signal.aborted) return;
		// The next walk starts first, so an answer that cannot be
		// handed on never holds up the rest.
		next();
		settled(id, franchise);
	};
	const next = () => {
		while (out < HANDOFF_WALKS_AT_ONCE && waiting.length > 0) {
			const id = waiting.shift()!;
			out += 1;
			read(id, page.signal).then(
				(franchise) => hand(id, franchise),
				() => hand(id, null)
			);
		}
	};
	return {
		want(id) {
			if (asked.has(id) || page.signal.aborted) return;
			asked.add(id);
			waiting.push(id);
			next();
		},
		stop() {
			page.abort();
			waiting.length = 0;
		}
	};
}

export interface NamedReads extends FranchiseReads {
	/**
	 * Forgets what was asked, so the next want asks again, and drops
	 * the answers still out: the history changed, so a row may now
	 * resume an entry no row resumed before, or none may any more.
	 */
	forget(): void;
}

/**
 * Whether a history row resumes `id`, by the backend's resume lookup
 * (`read`, `historyByKitsuEach`): true or false, or null when the read
 * failed. Asked once per id until `forget`.
 */
export function createNamedReads(
	read: (ids: readonly string[], signal: AbortSignal) => Promise<Readonly<Record<string, unknown>>>,
	settled: (id: string, named: boolean | null) => void
): NamedReads {
	let asked = new Set<string>();
	// One per forget: an answer is handed on only while its generation
	// is the current one, and a forget or a stop cancels the lookups
	// of the generation it ends.
	let generation = new AbortController();
	let stopped = false;
	return {
		want(id) {
			if (asked.has(id) || stopped) return;
			asked.add(id);
			const mine = generation;
			const hand = (named: boolean | null) => {
				if (mine === generation && !mine.signal.aborted) settled(id, named);
			};
			read([id], mine.signal).then(
				(rows) => hand(rows[id] != null),
				() => hand(null)
			);
		},
		forget() {
			if (stopped) return;
			generation.abort();
			generation = new AbortController();
			asked = new Set();
		},
		stop() {
			stopped = true;
			generation.abort();
		}
	};
}
