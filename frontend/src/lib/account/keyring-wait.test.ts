import { describe, expect, it, vi, afterEach } from 'vitest';
import { KEYRING_WAIT_MS, KeyringWait, KeyringWaitNotice } from './keyring-wait';

function deferred() {
	let resolve: () => void = () => {};
	let reject: (e: unknown) => void = () => {};
	const promise = new Promise<void>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

function setup(waitMs?: number) {
	const changes: boolean[] = [];
	const wait = new KeyringWait(
		{
			setTimer: (fn, ms) => setTimeout(() => fn(), ms),
			clearTimer: (h) => clearTimeout(h as ReturnType<typeof setTimeout>)
		},
		(w) => changes.push(w),
		waitMs
	);
	return { wait, changes };
}

afterEach(() => {
	vi.useRealTimers();
});

describe('KeyringWait', () => {
	it('says nothing for a read that settles inside the wait', async () => {
		vi.useFakeTimers();
		const { wait, changes } = setup();
		const read = deferred();
		wait.track(read.promise);
		await vi.advanceTimersByTimeAsync(KEYRING_WAIT_MS - 1);
		read.resolve();
		await vi.advanceTimersByTimeAsync(KEYRING_WAIT_MS * 3);
		expect(changes).toEqual([]);
	});

	it('says waiting when a read runs past the wait, and done when it settles', async () => {
		vi.useFakeTimers();
		const { wait, changes } = setup();
		const read = deferred();
		wait.track(read.promise);
		await vi.advanceTimersByTimeAsync(KEYRING_WAIT_MS);
		expect(changes).toEqual([true]);
		read.resolve();
		await vi.advanceTimersByTimeAsync(0);
		expect(changes).toEqual([true, false]);
	});

	it('counts a rejected read as settled', async () => {
		vi.useFakeTimers();
		const { wait, changes } = setup();
		const read = deferred();
		wait.track(read.promise);
		await vi.advanceTimersByTimeAsync(KEYRING_WAIT_MS);
		read.reject(new Error('ipc gone'));
		await vi.advanceTimersByTimeAsync(0);
		expect(changes).toEqual([true, false]);
	});

	it('stays waiting until the last of several overdue reads settles', async () => {
		vi.useFakeTimers();
		const { wait, changes } = setup(100);
		const a = deferred();
		const b = deferred();
		wait.track(a.promise);
		await vi.advanceTimersByTimeAsync(50);
		wait.track(b.promise);
		await vi.advanceTimersByTimeAsync(100);
		expect(changes).toEqual([true]);
		a.resolve();
		await vi.advanceTimersByTimeAsync(0);
		expect(changes).toEqual([true]);
		b.resolve();
		await vi.advanceTimersByTimeAsync(0);
		expect(changes).toEqual([true, false]);
	});
});

describe('KeyringWaitNotice', () => {
	function sink() {
		const pushed: string[] = [];
		const dismissed: string[] = [];
		let n = 0;
		return {
			pushed,
			dismissed,
			push: () => {
				const id = `t${++n}`;
				pushed.push(id);
				return id;
			},
			dismiss: (id: string) => dismissed.push(id)
		};
	}

	it('puts one notice up while waiting and takes it down after', () => {
		const notice = new KeyringWaitNotice();
		const s = sink();
		notice.sync(false, s);
		expect(s.pushed).toEqual([]);
		notice.sync(true, s);
		notice.sync(true, s);
		expect(s.pushed).toEqual(['t1']);
		notice.sync(false, s);
		notice.sync(false, s);
		expect(s.dismissed).toEqual(['t1']);
		notice.sync(true, s);
		expect(s.pushed).toEqual(['t1', 't2']);
	});
});
