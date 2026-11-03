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

export type NoticeKind = 'update' | 'outage' | 'download-done' | 'download-failed';
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
}

/** The result of a change: the new list, the row it touched, and
 *  whether the change is news (worth announcing). */
export interface Change {
	list: Notice[];
	notice: Notice | null;
	fresh: boolean;
}

export const MAX_ROWS = 50;
export const MAX_AGE_MS = 30 * 24 * 3600_000;
export const RESOLVED_TTL_MS = 24 * 3600_000;

const LIFETIME: Record<NoticeKind, Lifetime> = {
	update: 'condition',
	outage: 'condition',
	'download-done': 'event',
	'download-failed': 'event'
};

export const lifetimeOf = (kind: NoticeKind): Lifetime => LIFETIME[kind];

export function isKind(v: unknown): v is NoticeKind {
	return typeof v === 'string' && Object.hasOwn(LIFETIME, v);
}

/** Raise `input` at `now`. A new key adds a row. An existing row is
 *  updated in place, and is news again when it had resolved, or when
 *  it is an event the user had already read (a new batch). */
export function raise(list: readonly Notice[], input: RaiseInput, now: number): Change {
	const add = input.count ?? 1;
	const i = list.findIndex((n) => n.key === input.key);
	if (i === -1) {
		const notice: Notice = {
			key: input.key,
			kind: input.kind,
			firstAt: now,
			at: now,
			count: add,
			read: false,
			resolvedAt: null,
			data: { ...input.data }
		};
		return { list: [...list, notice], notice, fresh: true };
	}
	const prev = list[i];
	const fresh = prev.resolvedAt !== null || (lifetimeOf(input.kind) === 'event' && prev.read);
	const notice: Notice = {
		...prev,
		kind: input.kind,
		at: now,
		firstAt: fresh ? now : prev.firstAt,
		count: fresh ? add : prev.count + add,
		read: fresh ? false : prev.read,
		resolvedAt: null,
		data: { ...prev.data, ...input.data }
	};
	const next = list.slice();
	next[i] = notice;
	return { list: next, notice, fresh };
}

/** Mark the live condition `key` resolved at `now`. It comes back
 *  unread: good news counts too. A row that is missing, already
 *  resolved, or an event is left alone. */
export function resolveKey(
	list: readonly Notice[],
	key: string,
	now: number,
	data?: NoticeData
): Change {
	const i = list.findIndex(
		(n) => n.key === key && n.resolvedAt === null && lifetimeOf(n.kind) === 'condition'
	);
	if (i === -1) return { list: list.slice(), notice: null, fresh: false };
	const notice: Notice = {
		...list[i],
		resolvedAt: now,
		at: now,
		read: false,
		data: { ...list[i].data, ...data }
	};
	const next = list.slice();
	next[i] = notice;
	return { list: next, notice, fresh: true };
}

/** Remove a condition that stopped being true without resolving (the
 *  update was installed). Events and resolved rows stay. */
export function withdraw(list: readonly Notice[], key: string): Notice[] {
	return list.filter(
		(n) => n.key !== key || lifetimeOf(n.kind) === 'event' || n.resolvedAt !== null
	);
}

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

export function toneOf(n: Notice): Tone {
	if (n.kind === 'download-failed') return 'failure';
	if (n.resolvedAt !== null) return 'good';
	if (n.kind === 'outage') return 'live';
	return 'info';
}

/** Display order: live conditions first, then newest first. */
export function ordered(list: readonly Notice[]): Notice[] {
	return list.slice().sort((a, b) => {
		const la = toneOf(a) === 'live' ? 0 : 1;
		const lb = toneOf(b) === 'live' ? 0 : 1;
		return la - lb || b.at - a.at;
	});
}

export interface CountOptions {
	/** The update badge shows the update, so the bell leaves it out. */
	updateOnBadge: boolean;
}

export function counted(n: Notice, opts: CountOptions): boolean {
	if (n.read) return false;
	return !(n.kind === 'update' && opts.updateOnBadge);
}

export function unreadCount(list: readonly Notice[], opts: CountOptions): number {
	return list.filter((n) => counted(n, opts)).length;
}

/** The bell's count colour: oxblood while a failure or a live
 *  condition is among the counted rows, neutral otherwise. */
export function bellTone(list: readonly Notice[], opts: CountOptions): 'attention' | 'neutral' {
	const urgent = list.some((n) => {
		const t = toneOf(n);
		return counted(n, opts) && (t === 'failure' || t === 'live');
	});
	return urgent ? 'attention' : 'neutral';
}

export const STORAGE_KEY = 'ani-gui.notices.v1';

/** What survives a restart: events only. */
export function serialize(list: readonly Notice[]): string {
	return JSON.stringify({
		v: 1,
		rows: list.filter((n) => lifetimeOf(n.kind) === 'event')
	});
}

/** The stored rows, each checked field by field; anything that is not
 *  a stored event is dropped, then retention applies. */
export function restore(raw: string | null, now: number): Notice[] {
	if (!raw) return [];
	let parsed: unknown;
	try {
		parsed = JSON.parse(raw);
	} catch {
		return [];
	}
	if (typeof parsed !== 'object' || parsed === null) return [];
	const { v, rows } = parsed as { v?: unknown; rows?: unknown };
	if (v !== 1 || !Array.isArray(rows)) return [];
	const seen = new Set<string>();
	const kept: Notice[] = [];
	for (const r of rows) {
		const n = storedRow(r);
		if (n && !seen.has(n.key)) {
			seen.add(n.key);
			kept.push(n);
		}
	}
	return prune(kept, now);
}

function storedRow(r: unknown): Notice | null {
	if (typeof r !== 'object' || r === null) return null;
	const o = r as Record<string, unknown>;
	const num = (v: unknown) => typeof v === 'number' && Number.isFinite(v);
	if (typeof o.key !== 'string' || !isKind(o.kind) || lifetimeOf(o.kind) !== 'event') return null;
	if (!num(o.firstAt) || !num(o.at) || !num(o.count) || typeof o.read !== 'boolean') return null;
	return {
		key: o.key,
		kind: o.kind,
		firstAt: o.firstAt as number,
		at: o.at as number,
		count: o.count as number,
		read: o.read,
		resolvedAt: null,
		data: storedData(o.data)
	};
}

function storedData(v: unknown): NoticeData {
	if (typeof v !== 'object' || v === null || Array.isArray(v)) return {};
	const out: Record<string, string | number | null> = {};
	for (const [k, x] of Object.entries(v)) {
		if (typeof x === 'string' || x === null || (typeof x === 'number' && Number.isFinite(x))) {
			out[k] = x;
		}
	}
	return out;
}
