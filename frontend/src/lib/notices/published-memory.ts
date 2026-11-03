/**
 * What the center remembers of published notices across launches: the
 * ids read and the ids dismissed. A published notice is not stored
 * (./published raises it again from the flags file at each start), so
 * without this a notice read yesterday would arrive as news again
 * today, and a dismissed one would come back.
 */
import type { Notice } from './model';

export const PUBLISHED_STORAGE_KEY = 'ani-gui.notices.published.v1';

/** Ids kept of each list, the latest last. Well past the eight
 *  notices a file carries, and an id still published is moved to the
 *  end at each launch (`keepPublished`), so it is never the one let go. */
export const MAX_REMEMBERED = 64;

export interface PublishedMemory {
	read: string[];
	dismissed: string[];
}

const PREFIX = 'remote:';

export function serializeMemory(m: PublishedMemory): string {
	return JSON.stringify({ v: 1, read: m.read, dismissed: m.dismissed });
}

export function parseMemory(raw: string | null): PublishedMemory {
	const empty: PublishedMemory = { read: [], dismissed: [] };
	if (!raw) return empty;
	let o: unknown;
	try {
		o = JSON.parse(raw);
	} catch {
		return empty;
	}
	if (typeof o !== 'object' || o === null) return empty;
	const r = o as Record<string, unknown>;
	if (r.v !== 1) return empty;
	return { read: ids(r.read), dismissed: ids(r.dismissed) };
}

const ids = (v: unknown): string[] =>
	Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : [];

/** `m` with the published notices read in `after`, and those present
 *  in `before` and gone from `after` (dismissed), added. */
export function remember(
	m: PublishedMemory,
	before: readonly Notice[],
	after: readonly Notice[]
): PublishedMemory {
	const idOf = (n: Notice) => (n.key.startsWith(PREFIX) ? n.key.slice(PREFIX.length) : null);
	const published = (l: readonly Notice[]) => l.filter((n) => n.kind === 'remote');
	const kept = new Set(published(after).map((n) => n.key));
	const read = published(after)
		.filter((n) => n.read)
		.map(idOf);
	const gone = published(before)
		.filter((n) => !kept.has(n.key))
		.map(idOf);
	return { read: add(m.read, read), dismissed: add(m.dismissed, gone) };
}

/** `list` with `more` at its end, each once, the latest kept. */
function add(list: readonly string[], more: ReadonlyArray<string | null>): string[] {
	const fresh = more.filter((id): id is string => id !== null);
	return [...list.filter((id) => !fresh.includes(id)), ...new Set(fresh)].slice(-MAX_REMEMBERED);
}

/** `m` with the ids the file still publishes moved to the end of each
 *  list, so the ones let go when a list is full are those no longer
 *  published. */
export function keepPublished(m: PublishedMemory, published: readonly string[]): PublishedMemory {
	const keep = (list: readonly string[]) =>
		add(
			list,
			list.filter((id) => published.includes(id))
		);
	return { read: keep(m.read), dismissed: keep(m.dismissed) };
}
