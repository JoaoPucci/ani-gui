/** Retention: what the list keeps, and for how long (see ./kinds). */
import { MAX_AGE_MS, MAX_ROWS, RESOLVED_TTL_MS, lifetimeOf, type Notice } from './kinds';

/** Drop rows past retention: events older than {@link MAX_AGE_MS},
 *  resolved conditions older than {@link RESOLVED_TTL_MS}, then the
 *  oldest events past {@link MAX_ROWS}. A live condition is never
 *  dropped here. A row dated in the future (a clock set back) is kept
 *  only as long as a row of age zero would be. */
export function prune(list: readonly Notice[], now: number): Notice[] {
	const age = (t: number) => Math.max(0, now - t);
	const kept = list.filter((n) => {
		if (n.resolvedAt !== null) return age(n.resolvedAt) < RESOLVED_TTL_MS;
		if (lifetimeOf(n.kind) === 'event') return age(n.at) < MAX_AGE_MS;
		return true;
	});
	const events = kept
		.filter((n) => lifetimeOf(n.kind) === 'event')
		.sort((a, b) => b.at - a.at || (a.key < b.key ? -1 : 1));
	if (events.length <= MAX_ROWS) return kept;
	const drop = new Set(events.slice(MAX_ROWS).map((n) => n.key));
	return kept.filter((n) => !drop.has(n.key));
}
