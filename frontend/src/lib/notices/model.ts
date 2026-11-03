/**
 * The notification center's list, as pure functions.
 *
 * - **One list, keyed rows.** Raising a key that exists updates that
 *   row instead of adding one: fifty failed requests are one outage
 *   row, three finished episodes of a show are one row with a count.
 * - **Conditions and events.** A condition is true for a while and
 *   resolves itself (an update exists, the source is not answering);
 *   an event happened once (a download finished or failed). Events are
 *   kept across restarts, at most {@link MAX_ROWS} of them and none
 *   older than {@link MAX_AGE_MS}; conditions are never stored, they
 *   are worked out again at each start from what raises them. A
 *   condition that resolves stays, marked resolved, for
 *   {@link RESOLVED_TTL_MS} or until the app restarts.
 * - **Every row counts on the bell while it is unread**, good news
 *   included, except the update while its own badge shows it: the
 *   bell does not count one update twice.
 */

import { lifetimeOf, type Notice } from './kinds';

export * from './kinds';
export * from './change';
export * from './retention';
export * from './tally';
export * from './storage';

export function markRead(list: readonly Notice[], key: string): Notice[] {
	return list.map((n) => (n.key === key && !n.read ? { ...n, read: true } : n));
}

export function markAllRead(list: readonly Notice[]): Notice[] {
	return list.map((n) => (n.read ? n : { ...n, read: true }));
}

/** The user may remove events, and conditions once they resolved; a
 *  live condition goes when it resolves. */
export function canDismiss(n: Notice): boolean {
	return lifetimeOf(n.kind) === 'event' || n.resolvedAt !== null;
}

export function dismiss(list: readonly Notice[], key: string): Notice[] {
	return list.filter((n) => n.key !== key || !canDismiss(n));
}
