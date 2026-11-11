import { describe, expect, it, vi } from 'vitest';
import { NoticeCenter, type NoticeStorage } from './center.svelte';
import { STORAGE_KEY, type Notice } from './model';

const NOW = 1_800_000_000_000;

function memory(initial: string | null = null) {
	let value = initial;
	const storage: NoticeStorage = {
		getItem: (k) => (k === STORAGE_KEY ? value : null),
		setItem: (k, v) => {
			if (k === STORAGE_KEY) value = v;
		}
	};
	return { storage, read: () => value };
}

function center(storage: NoticeStorage | null = memory().storage) {
	const toasts: Notice[] = [];
	let now = NOW;
	const c = new NoticeCenter({ storage, now: () => now, toast: (n) => toasts.push(n) });
	return { c, toasts, tick: (ms: number) => (now += ms) };
}

const failed = { key: 'f1', kind: 'download-failed' as const, data: { title: 'T', episode: '1' } };
const outage = { key: 'outage:hianime', kind: 'outage' as const };

describe('the notification center', () => {
	it('toasts a new notice and counts it', () => {
		const { c, toasts } = center();
		c.raise(failed);
		expect(toasts.map((n) => n.key)).toEqual(['f1']);
		expect(c.count(true)).toBe(1);
		expect(c.tone(true)).toBe('attention');
	});

	it('does not toast again for a change that is not news', () => {
		const { c, toasts } = center();
		c.raise(outage);
		c.raise({ ...outage, data: { failures: 9 } });
		expect(toasts).toHaveLength(1);
	});

	it('keeps quiet while a video plays, except for a failure', () => {
		const { c, toasts } = center();
		c.setMoment({ playing: true, fullscreen: false });
		c.raise(outage);
		c.raise(failed);
		expect(toasts.map((n) => n.key)).toEqual(['f1']);
		expect(c.count(true)).toBe(2);
	});

	it('holds toasts during fullscreen and shows them on leaving it', () => {
		const { c, toasts } = center();
		c.setMoment({ playing: true, fullscreen: true });
		c.raise(failed);
		c.raise(outage);
		expect(toasts).toEqual([]);
		expect(c.count(true)).toBe(2);
		c.setMoment({ playing: true, fullscreen: true });
		expect(toasts).toEqual([]);
		c.setMoment({ playing: true, fullscreen: false });
		expect(toasts.map((n) => n.key)).toEqual(['f1']);
		c.setMoment({ playing: false, fullscreen: false });
		expect(toasts).toHaveLength(1);
	});

	it('resolves an outage as news, and applies source changes', () => {
		const { c, toasts } = center();
		c.take([{ op: 'raise', input: outage }]);
		c.markAllRead();
		expect(c.count(true)).toBe(0);
		c.take([{ op: 'resolve', key: outage.key, at: NOW + 5, data: { since: 1 } }]);
		expect(c.rows[0].resolvedAt).toBe(NOW + 5);
		expect(c.count(true)).toBe(1);
		expect(c.tone(true)).toBe('neutral');
		expect(toasts).toHaveLength(2);
		c.take([{ op: 'raise', input: { key: 'update', kind: 'update' } }]);
		expect(c.count(true)).toBe(1);
		expect(c.count(false)).toBe(2);
		c.take([{ op: 'withdraw', key: 'update' }]);
		expect(c.rows.some((n) => n.key === 'update')).toBe(false);
	});

	it('marks read and dismisses, closing a row opened in place', () => {
		const { c } = center();
		c.raise(failed);
		c.raise({ ...failed, key: 'f2' });
		c.markRead('f1');
		expect(c.count(true)).toBe(1);
		c.expanded = 'f2';
		c.dismiss('f2');
		expect(c.expanded).toBeNull();
		c.expanded = 'f1';
		c.dismiss('nope');
		expect(c.expanded).toBe('f1');
		expect(c.rows.map((n) => n.key)).toEqual(['f1']);
	});

	it('keeps events across a restart, and only events', () => {
		const mem = memory();
		const first = center(mem.storage).c;
		first.raise(failed);
		first.raise(outage);
		const second = center(mem.storage).c;
		second.load();
		expect(second.rows.map((n) => n.key)).toEqual(['f1']);
	});

	it('works without storage, or with storage that refuses', () => {
		const none = center(null).c;
		none.load();
		none.raise(failed);
		expect(none.rows).toHaveLength(1);
		const refusing: NoticeStorage = {
			getItem: () => {
				throw new Error('denied');
			},
			setItem: () => {
				throw new Error('quota');
			}
		};
		const c = center(refusing).c;
		c.load();
		expect(c.rows).toEqual([]);
		c.raise(failed);
		expect(c.rows).toHaveLength(1);
	});

	it('writes nothing when a source repeats itself', () => {
		const writes = vi.fn();
		const storage: NoticeStorage = { getItem: () => null, setItem: writes };
		const { c } = center(storage);
		c.take([{ op: 'raise', input: { ...outage, count: 0, data: { failures: 4 } } }]);
		const after = writes.mock.calls.length;
		const rows = c.rows;
		c.take([{ op: 'raise', input: { ...outage, count: 0, data: { failures: 4 } } }]);
		c.take([{ op: 'withdraw', key: 'nope' }]);
		expect(writes.mock.calls.length).toBe(after);
		expect(c.rows).toBe(rows);
	});

	it('resolves at the time a source gives, or now when it gives none', () => {
		const { c, tick } = center();
		c.raise(outage);
		tick(50);
		c.take([{ op: 'resolve', key: outage.key, data: {} }]);
		expect(c.rows[0].resolvedAt).toBe(NOW + 50);
	});

	it('reads what the window is doing at each arrival when it can', () => {
		const toasts: Notice[] = [];
		let playing = true;
		const c = new NoticeCenter({
			storage: null,
			now: () => NOW,
			toast: (n) => toasts.push(n),
			moment: () => ({ playing, fullscreen: false })
		});
		c.raise(outage);
		expect(toasts).toEqual([]);
		playing = false;
		c.raise({ ...failed, kind: 'download-done' });
		expect(toasts.map((n) => n.key)).toEqual(['f1']);
	});

	it('does nothing for a resolve of a row it does not have', () => {
		const { c, toasts } = center();
		c.resolve('outage:x', NOW);
		expect(c.rows).toEqual([]);
		expect(toasts).toEqual([]);
	});

	it('lets the update arrive without a toast', () => {
		const { c, toasts } = center();
		const spy = vi.fn();
		c.raise({ key: 'update', kind: 'update' });
		expect(toasts).toEqual([]);
		expect(spy).not.toHaveBeenCalled();
	});
});

describe('watching the list change', () => {
	it('tells its observer the rows before and after each change, reads and dismissals included', () => {
		const seen: Array<[string[], string[]]> = [];
		const keys = (l: readonly Notice[]) => l.map((n) => `${n.key}:${n.read ? 'r' : 'u'}`);
		const c = new NoticeCenter({
			storage: null,
			now: () => NOW,
			toast: () => {},
			observe: (before, after) => seen.push([keys(before), keys(after)])
		});
		c.raise({ key: 'remote:a', kind: 'remote', data: { title: 'A' } });
		c.markRead('remote:a');
		c.dismiss('remote:a');
		expect(seen).toEqual([
			[[], ['remote:a:u']],
			[['remote:a:u'], ['remote:a:r']],
			[['remote:a:r'], []]
		]);
	});
});
