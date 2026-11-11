/** Changes to the list that a source asks for: raise, resolve,
 *  withdraw. */
import { lifetimeOf, type Change, type Notice, type NoticeData, type RaiseInput } from './kinds';

/** Raise `input` at `now`. A new key adds a row, news unless it is
 *  raised already read. An existing row is
 *  updated in place, and is news again when it had resolved, or when
 *  it is an event the user had already read (a new batch). */
export function raise(list: readonly Notice[], input: RaiseInput, now: number): Change {
	const add = input.count ?? 1;
	const i = list.findIndex((n) => n.key === input.key);
	if (i === -1) {
		const read = input.read === true;
		const notice: Notice = {
			key: input.key,
			kind: input.kind,
			firstAt: now,
			at: now,
			count: add,
			read,
			resolvedAt: null,
			data: { ...input.data }
		};
		return { list: [...list, notice], notice, fresh: !read, changed: true };
	}
	const prev = list[i];
	const fresh = prev.resolvedAt !== null || (lifetimeOf(input.kind) === 'event' && prev.read);
	const data = { ...prev.data, ...input.data };
	if (!fresh && add === 0 && prev.kind === input.kind && sameData(prev.data, data)) {
		return { list: list.slice(), notice: null, fresh: false, changed: false };
	}
	const notice: Notice = {
		...prev,
		kind: input.kind,
		at: now,
		firstAt: fresh ? now : prev.firstAt,
		count: fresh ? add : prev.count + add,
		read: fresh ? false : prev.read,
		resolvedAt: null,
		data
	};
	const next = list.slice();
	next[i] = notice;
	return { list: next, notice, fresh, changed: true };
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
	if (i === -1) return { list: list.slice(), notice: null, fresh: false, changed: false };
	const notice: Notice = {
		...list[i],
		resolvedAt: now,
		at: now,
		read: false,
		data: { ...list[i].data, ...data }
	};
	const next = list.slice();
	next[i] = notice;
	return { list: next, notice, fresh: true, changed: true };
}

/** Remove a condition that stopped being true without resolving (the
 *  update was installed). Events and resolved rows stay. */
export function withdraw(list: readonly Notice[], key: string): Notice[] {
	return list.filter(
		(n) => n.key !== key || lifetimeOf(n.kind) === 'event' || n.resolvedAt !== null
	);
}

function sameData(a: NoticeData, b: NoticeData): boolean {
	const keys = Object.keys(b);
	return keys.length === Object.keys(a).length && keys.every((k) => a[k] === b[k]);
}
