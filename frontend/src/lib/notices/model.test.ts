import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	MAX_AGE_MS,
	MAX_ROWS,
	RESOLVED_TTL_MS,
	bellTone,
	canDismiss,
	dismiss,
	isKind,
	markAllRead,
	markRead,
	ordered,
	prune,
	raise,
	resolveKey,
	restore,
	serialize,
	toneOf,
	unreadCount,
	withdraw,
	type Notice,
	type NoticeKind,
	type RaiseInput
} from './model';

const NOW = 1_800_000_000_000;
const outage: RaiseInput = { key: 'outage:hianime', kind: 'outage', data: { since: 1 } };
const failed = (n: number): RaiseInput => ({
	key: `download-failed:${n}`,
	kind: 'download-failed',
	data: { title: 'Frieren', episode: '5' }
});
const done: RaiseInput = { key: 'download-done:Frieren', kind: 'download-done' };
const update: RaiseInput = { key: 'update', kind: 'update', data: { tag: 'v0.20.1' } };
const BADGE = { updateOnBadge: true };
const NO_BADGE = { updateOnBadge: false };

function build(inputs: RaiseInput[], at = NOW): Notice[] {
	return inputs.reduce<Notice[]>((list, i, k) => raise(list, i, at + k).list, []);
}

describe('raising a notice', () => {
	it('adds a row for a new key, unread, as news', () => {
		const r = raise([], outage, NOW);
		expect(r.fresh).toBe(true);
		expect(r.list).toHaveLength(1);
		expect(r.notice).toMatchObject({
			key: 'outage:hianime',
			count: 1,
			read: false,
			resolvedAt: null,
			firstAt: NOW,
			at: NOW
		});
	});

	it('updates the row of an existing key instead of adding one', () => {
		let list = raise([], { ...done, count: 2 }, NOW).list;
		const r = raise(list, { ...done, count: 1, data: { destDir: '/d' } }, NOW + 5);
		list = r.list;
		expect(list).toHaveLength(1);
		expect(r.fresh).toBe(false);
		expect(list[0]).toMatchObject({ count: 3, at: NOW + 5, firstAt: NOW });
		expect(list[0].data).toEqual({ destDir: '/d' });
	});

	it('keeps a live condition read while it only changes', () => {
		const list = markAllRead(raise([], outage, NOW).list);
		const r = raise(list, { ...outage, data: { failures: 9 } }, NOW + 1);
		expect(r.fresh).toBe(false);
		expect(r.notice?.read).toBe(true);
		expect(r.notice?.data).toEqual({ since: 1, failures: 9 });
	});

	it('makes a read event news again: a new batch', () => {
		const list = markAllRead(raise([], { ...done, count: 3 }, NOW).list);
		const r = raise(list, done, NOW + 10);
		expect(r.fresh).toBe(true);
		expect(r.notice).toMatchObject({ read: false, count: 1, firstAt: NOW + 10 });
	});

	it('makes a resolved condition news again when it comes back', () => {
		let list = raise([], outage, NOW).list;
		list = resolveKey(list, outage.key, NOW + 1).list;
		const r = raise(list, outage, NOW + 2);
		expect(r.fresh).toBe(true);
		expect(r.notice).toMatchObject({ resolvedAt: null, read: false, firstAt: NOW + 2 });
	});
});

describe('resolving a condition', () => {
	it('marks it resolved and unread: good news counts', () => {
		const list = markAllRead(raise([], outage, NOW).list);
		const r = resolveKey(list, outage.key, NOW + 9, { recovered: 9 });
		expect(r.fresh).toBe(true);
		expect(r.notice).toMatchObject({ resolvedAt: NOW + 9, read: false, at: NOW + 9 });
		expect(r.notice?.data).toEqual({ since: 1, recovered: 9 });
		expect(unreadCount(r.list, NO_BADGE)).toBe(1);
		expect(toneOf(r.notice as Notice)).toBe('good');
	});

	it('leaves a missing, resolved or event row alone', () => {
		const list = build([outage, failed(1)]);
		expect(resolveKey(list, 'nope', NOW).notice).toBeNull();
		expect(resolveKey(list, 'download-failed:1', NOW).notice).toBeNull();
		const once = resolveKey(list, outage.key, NOW).list;
		const twice = resolveKey(once, outage.key, NOW + 5);
		expect(twice.notice).toBeNull();
		expect(twice.list).toEqual(once);
	});
});

describe('withdrawing a condition', () => {
	it('removes a live condition but keeps events and resolved rows', () => {
		let list = build([update, failed(1), outage]);
		list = resolveKey(list, outage.key, NOW).list;
		expect(withdraw(list, 'update').map((n) => n.key)).toEqual([
			'download-failed:1',
			'outage:hianime'
		]);
		expect(withdraw(list, 'download-failed:1')).toHaveLength(3);
		expect(withdraw(list, outage.key)).toHaveLength(3);
	});
});

describe('read state', () => {
	it('marks one row or all rows read', () => {
		const list = build([outage, failed(1)]);
		expect(markRead(list, outage.key).map((n) => n.read)).toEqual([true, false]);
		expect(markAllRead(list).every((n) => n.read)).toBe(true);
	});
});

describe('dismissing', () => {
	it('removes events and resolved conditions, never a live one', () => {
		let list = build([outage, failed(1), update]);
		expect(canDismiss(list[0])).toBe(false);
		expect(canDismiss(list[1])).toBe(true);
		expect(canDismiss(list[2])).toBe(false);
		expect(dismiss(list, outage.key)).toHaveLength(3);
		expect(dismiss(list, 'download-failed:1')).toHaveLength(2);
		list = resolveKey(list, outage.key, NOW).list;
		expect(dismiss(list, outage.key)).toHaveLength(2);
	});
});

describe('retention', () => {
	it('drops events past thirty days and resolved conditions past a day', () => {
		let list = build([failed(1), outage]);
		list = resolveKey(list, outage.key, NOW).list;
		expect(prune(list, NOW + RESOLVED_TTL_MS - 1)).toHaveLength(2);
		expect(prune(list, NOW + RESOLVED_TTL_MS).map((n) => n.key)).toEqual(['download-failed:1']);
		expect(prune(list, NOW + MAX_AGE_MS + 1)).toHaveLength(0);
	});

	it('keeps a live condition however old', () => {
		const list = build([outage]);
		expect(prune(list, NOW + 10 * MAX_AGE_MS)).toHaveLength(1);
	});

	it('keeps only the newest events past the row cap', () => {
		const inputs = Array.from({ length: MAX_ROWS + 5 }, (_, i) => failed(i));
		const list = prune(build([...inputs, outage]), NOW + MAX_ROWS + 10);
		expect(list.filter((n) => n.kind === 'download-failed')).toHaveLength(MAX_ROWS);
		expect(list.some((n) => n.key === 'download-failed:0')).toBe(false);
		expect(list.some((n) => n.key === `download-failed:${MAX_ROWS + 4}`)).toBe(true);
		expect(list.some((n) => n.key === outage.key)).toBe(true);
	});

	it('breaks a tie between events of the same moment by key, so the cut is stable', () => {
		const inputs = Array.from({ length: MAX_ROWS + 1 }, (_, i) => failed(i));
		const list = inputs.reduce<Notice[]>((l, i) => raise(l, i, NOW).list, []);
		const kept = prune(list, NOW).map((n) => n.key);
		expect(kept).toHaveLength(MAX_ROWS);
		expect(
			prune([...list].reverse(), NOW)
				.map((n) => n.key)
				.sort()
		).toEqual([...kept].sort());
	});

	it('treats a row dated in the future as new, not as kept forever', () => {
		const list = build([failed(1)], NOW + MAX_AGE_MS * 2);
		expect(prune(list, NOW)).toHaveLength(1);
	});
});

describe('counting', () => {
	it('counts every unread row, the update only while its badge does not show it', () => {
		const list = build([update, outage, done]);
		expect(unreadCount(list, BADGE)).toBe(2);
		expect(unreadCount(list, NO_BADGE)).toBe(3);
		expect(unreadCount(markRead(list, done.key), BADGE)).toBe(1);
	});

	it('turns the bell oxblood for an unread failure or live problem, neutral otherwise', () => {
		expect(bellTone(build([done]), BADGE)).toBe('neutral');
		expect(bellTone(build([failed(1)]), BADGE)).toBe('attention');
		expect(bellTone(build([outage]), BADGE)).toBe('attention');
		expect(bellTone(markAllRead(build([outage])), BADGE)).toBe('neutral');
		const resolved = resolveKey(build([outage]), outage.key, NOW).list;
		expect(bellTone(resolved, BADGE)).toBe('neutral');
		expect(bellTone(build([update]), NO_BADGE)).toBe('neutral');
	});
});

describe('tone and order', () => {
	it('tells a failed event, a live condition, a resolved one and news apart', () => {
		const [u, o, d, f] = build([update, outage, done, failed(1)]);
		expect([u, o, d, f].map(toneOf)).toEqual(['info', 'live', 'info', 'failure']);
	});

	it('puts live conditions first, then the newest', () => {
		const list = build([failed(1), done, outage, failed(2)]);
		expect(ordered(list).map((n) => n.key)).toEqual([
			'outage:hianime',
			'download-failed:2',
			'download-done:Frieren',
			'download-failed:1'
		]);
	});
});

describe('kinds', () => {
	it('knows its own kinds and nothing else', () => {
		expect(isKind('outage')).toBe(true);
		expect(isKind('toString')).toBe(false);
		expect(isKind('nope')).toBe(false);
		expect(isKind(3)).toBe(false);
	});
});

describe('storage', () => {
	it('stores events only, and restores them', () => {
		const list = build([update, outage, failed(1), done]);
		const back = restore(serialize(list), NOW + 10);
		expect(back.map((n) => n.key)).toEqual(['download-failed:1', 'download-done:Frieren']);
		expect(back[0].data).toEqual({ title: 'Frieren', episode: '5' });
	});

	it('restores nothing from nothing, junk, or another version', () => {
		for (const raw of [null, '', '{', 'null', '3', '[]', '{"v":2,"rows":[]}', '{"v":1}']) {
			expect(restore(raw, NOW)).toEqual([]);
		}
	});

	it('drops malformed rows and their unknown data, and repeated keys', () => {
		const good = build([failed(1)])[0];
		const raw = JSON.stringify({
			v: 1,
			rows: [
				null,
				'x',
				{ ...good, kind: 'outage' },
				{ ...good, kind: 'nope' },
				{ ...good, key: 3 },
				{ ...good, at: 'later' },
				{ ...good, count: Infinity },
				{ ...good, read: 'no' },
				{ ...good, data: { title: 'T', bad: { nested: 1 }, n: 2, z: null } },
				{ ...good, data: [1] }
			]
		});
		const back = restore(raw, NOW + 1);
		expect(back).toHaveLength(1);
		expect(back[0].data).toEqual({ title: 'T', n: 2, z: null });
	});

	it('applies retention as it restores', () => {
		const list = build([failed(1)]);
		expect(restore(serialize(list), NOW + MAX_AGE_MS + 1)).toEqual([]);
	});
});

const KINDS: NoticeKind[] = ['update', 'outage', 'download-done', 'download-failed'];
const op = fc.oneof(
	fc.record({
		t: fc.constant('raise' as const),
		key: fc.constantFrom('a', 'b', 'c', 'd', 'e'),
		kind: fc.constantFrom(...KINDS),
		count: fc.integer({ min: 1, max: 5 })
	}),
	fc.record({ t: fc.constant('resolve' as const), key: fc.constantFrom('a', 'b', 'c') }),
	fc.record({ t: fc.constant('read' as const), key: fc.constantFrom('a', 'b', 'c') }),
	fc.record({ t: fc.constant('dismiss' as const), key: fc.constantFrom('a', 'b', 'c') }),
	fc.record({ t: fc.constant('readAll' as const) })
);

describe('over any sequence of changes', () => {
	it('keys stay unique, counts stay positive, and the bell never counts a read row', () => {
		fc.assert(
			fc.property(
				fc.array(fc.tuple(op, fc.integer({ min: 0, max: 1e6 })), { maxLength: 60 }),
				(ops) => {
					let list: Notice[] = [];
					for (const [o, dt] of ops) {
						const now = NOW + dt;
						if (o.t === 'raise')
							list = raise(list, { key: o.key, kind: o.kind, count: o.count }, now).list;
						else if (o.t === 'resolve') list = resolveKey(list, o.key, now).list;
						else if (o.t === 'read') list = markRead(list, o.key);
						else if (o.t === 'dismiss') list = dismiss(list, o.key);
						else list = markAllRead(list);
						list = prune(list, now);
						const keys = list.map((n) => n.key);
						expect(new Set(keys).size).toBe(keys.length);
						expect(list.every((n) => n.count >= 1)).toBe(true);
						const unread = list.filter((n) => !n.read).length;
						expect(unreadCount(list, NO_BADGE)).toBe(unread);
						expect(unreadCount(list, BADGE)).toBeLessThanOrEqual(unread);
						expect(list.filter((n) => n.kind.startsWith('download')).length).toBeLessThanOrEqual(
							MAX_ROWS
						);
					}
				}
			)
		);
	});

	it('a restore of what was stored is the events, and never throws on any string', () => {
		fc.assert(
			fc.property(fc.string(), (s) => {
				expect(Array.isArray(restore(s, NOW))).toBe(true);
			})
		);
		fc.assert(
			fc.property(fc.array(fc.constantFrom(...KINDS), { maxLength: 20 }), (kinds) => {
				const list = build(kinds.map((kind, i) => ({ key: `k${i}`, kind })));
				const back = restore(serialize(list), NOW + 100);
				expect(back.map((n) => n.key)).toEqual(
					list.filter((n) => n.kind.startsWith('download')).map((n) => n.key)
				);
			})
		);
	});
});
