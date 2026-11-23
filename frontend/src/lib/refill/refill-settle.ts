/**
 * How a wait's answer reaches the pages waiting on it
 * (`./refill-waiter`): a landed ticket resolves every page waiting on
 * it; an unlanded one leaves a page waiting on its other tickets, and
 * resolves it unlanded when none is left. Split from the waiter so each
 * file stays inside the CRAP gate's per-file bar.
 */
import type { RefillEnded } from './refill-waiter';

/** A page waiting on its tickets. */
export interface Listener {
	tickets: Set<number>;
	done: (landed: boolean) => void;
}

/** Hand each ended ticket to the pages waiting on it. */
export function settle(listeners: Set<Listener>, ended: RefillEnded[]): void {
	for (const { ticket, landed } of ended) {
		for (const l of [...listeners]) {
			if (!l.tickets.has(ticket)) continue;
			if (landed) {
				l.done(true);
			} else {
				l.tickets.delete(ticket);
				if (l.tickets.size === 0) l.done(false);
			}
		}
	}
}
