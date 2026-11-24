/**
 * The reads behind Continue Watching's hand-offs, however often the
 * rail re-renders: each Kitsu id's hand-off (`$lib/api`
 * `watchOrderHandoff`, which walks the id's franchise and asks the
 * backend's resume lookup about the entry it would move on to), asked
 * once per id until the history changes (`forget`). A read that fails
 * settles as null (the row keeps its ordinary card, with no error).
 *
 * Each read is handed a signal, aborted when the page goes (`stop`)
 * and when forgetting drops its answer: a hand-off read holds its
 * response open while the backend walks the franchise, on the
 * franchise lane (`$lib/backend/lanes`), so a read whose answer nobody
 * will use must not hold a connection. The reads go out as many at a
 * time as the lane can spare (`handoffWalksAtOnce`), in the order the
 * rows ask for them.
 */
import type { Handoff } from '$lib/api';

export interface HandoffReads {
	/** Asks for `id`'s hand-off, unless it was asked for already. */
	want(id: string): void;
	/**
	 * Forgets what was asked, so the next want asks again, and drops
	 * the answers still out: the history changed, so a row may now
	 * resume an entry no row resumed before, or none may any more. The
	 * answers already handed on stand until the new ones land.
	 */
	forget(): void;
	/** The page has gone: cancels every read still out, and hands on no more answers. */
	stop(): void;
}

/**
 * How many series walks Home keeps out at once on a franchise lane of
 * its own: that lane's six connections, which Chromium gives each
 * origin. The walks have the lane to themselves while Home is open, and
 * Home's own calls (a Continue card's resume, a removal) go to the API
 * lane, so none waits behind them. More would only wait in Chromium's
 * queue instead of this one, which starts them in the order the rows
 * ask. Each walk reads up to four Kitsu listings at a time on a cold
 * cache, so six walks put up to 24 in flight; a listing is cached for a
 * day, or a week once everything it relates has finished, so a warm
 * Home asks Kitsu for little or nothing.
 */
export const HANDOFF_WALKS_ON_OWN_LANE = 6;

/**
 * How many walks Home keeps out at once when the franchise lane falls
 * back to the API's origin (browser-only dev, or a window opened
 * without the lane). The walks then share the API origin's six
 * connections with Home's own calls; the refill wait can hold one, so
 * two walks leave three for a Continue card's resume and a removal once
 * Home's first reads have answered.
 */
export const HANDOFF_WALKS_ON_SHARED_LANE = 2;

/** The walk budget for a franchise lane that is (`ownLane`) or is not
 *  an origin of its own (`windowLaneIsOwn` in `$lib/api`). */
export function handoffWalksAtOnce(ownLane: boolean): number {
	return ownLane ? HANDOFF_WALKS_ON_OWN_LANE : HANDOFF_WALKS_ON_SHARED_LANE;
}

export function createHandoffReads(
	read: (id: string, signal: AbortSignal) => Promise<Handoff | null>,
	settled: (id: string, handoff: Handoff | null) => void,
	atOnce: number
): HandoffReads {
	let asked = new Set<string>();
	// One per forget: an answer is handed on only while its generation
	// is the current one, and a forget or a stop cancels the reads of
	// the generation it ends.
	let generation = new AbortController();
	let stopped = false;
	// Wanted ids not yet read, in the order they were wanted.
	let waiting: string[] = [];
	let out = 0;
	const next = () => {
		while (out < atOnce && waiting.length > 0) {
			const id = waiting.shift()!;
			const mine = generation;
			out += 1;
			const hand = (handoff: Handoff | null) => {
				out -= 1;
				// The next read starts first, so an answer that cannot be
				// handed on never holds up the rest.
				next();
				if (mine === generation && !mine.signal.aborted) settled(id, handoff);
			};
			read(id, mine.signal).then(hand, () => hand(null));
		}
	};
	return {
		want(id) {
			if (asked.has(id) || stopped) return;
			asked.add(id);
			waiting.push(id);
			next();
		},
		forget() {
			if (stopped) return;
			generation.abort();
			generation = new AbortController();
			asked = new Set();
			waiting = [];
		},
		stop() {
			stopped = true;
			generation.abort();
			waiting = [];
		}
	};
}
