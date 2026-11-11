/**
 * The notification center's vocabulary: what a notice is, its kinds,
 * and how long each kind lives. The rules over it are split by concern
 * — the list (./model), retention (./retention), the bell's tally
 * (./tally) and storage (./storage) — and ./model re-exports them all.
 */

export type NoticeKind = 'update' | 'outage' | 'download-done' | 'download-failed' | 'remote';
export type Lifetime = 'condition' | 'event';

/** How a row is drawn: a failed event (oxblood), a live condition
 *  (amber), a condition that resolved (neutral, a check), or news. */
export type Tone = 'failure' | 'live' | 'good' | 'info';

export type NoticeData = Readonly<Record<string, string | number | null>>;

export interface Notice {
	key: string;
	kind: NoticeKind;
	/** When the row was first raised (or raised afresh). */
	firstAt: number;
	/** When it last changed. */
	at: number;
	/** How many times it was raised since it was last news. */
	count: number;
	read: boolean;
	resolvedAt: number | null;
	data: NoticeData;
}

export interface RaiseInput {
	key: string;
	kind: NoticeKind;
	data?: NoticeData;
	/** Added to the row's count; 1 when omitted. */
	count?: number;
	/** A new row that is already read: news the user saw at an earlier
	 *  launch (a published notice), raised again without announcing it. */
	read?: boolean;
}

/** The result of a change: the new list, the row it touched, and
 *  whether the change is news (worth announcing). */
export interface Change {
	list: Notice[];
	notice: Notice | null;
	fresh: boolean;
	/** Whether the list changed at all: a source repeating itself
	 *  changes nothing, and nothing is written for it. */
	changed: boolean;
}

export const MAX_ROWS = 50;
export const MAX_AGE_MS = 30 * 24 * 3600_000;
export const RESOLVED_TTL_MS = 24 * 3600_000;

const LIFETIME: Record<NoticeKind, Lifetime> = {
	update: 'condition',
	outage: 'condition',
	'download-done': 'event',
	'download-failed': 'event',
	// Published by the maintainer: true while the flags file carries it,
	// worked out again at each start, so never stored (./published).
	remote: 'condition'
};

export const lifetimeOf = (kind: NoticeKind): Lifetime => LIFETIME[kind];

export function isKind(v: unknown): v is NoticeKind {
	return typeof v === 'string' && Object.hasOwn(LIFETIME, v);
}
