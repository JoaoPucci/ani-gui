import { describe, expect, it, vi } from 'vitest';
import { joinResumeRefresh, settledWithin } from './resume-join';

const entry = { id: 'hianime:show-4', ep_no: '24.5', title: 'Show' };

describe('joinResumeRefresh (asked again)', () => {
	it('runs again when asked, after a refresh has run', async () => {
		// A warm of the show pays for a listing too; the row is read
		// again after it — once the refresh that ran has settled.
		const run = vi.fn();
		const join = joinResumeRefresh(run);
		join.row(entry);
		join.resolved();
		await join.settled();
		join.again();
		expect(run).toHaveBeenCalledTimes(2);
	});

	it('says none is out once a new visit starts', () => {
		const join = joinResumeRefresh(() => new Promise<void>(() => {}));
		join.row(entry);
		join.resolved();
		expect(join.refreshing()).toBe(true);
		join.reset();
		expect(join.refreshing()).toBe(false);
	});
});

describe('settledWithin', () => {
	it('waits for a newer refresh too', async () => {
		let calls = 0;
		const finish: (() => void)[] = [];
		const join = joinResumeRefresh(() => {
			calls += 1;
			return new Promise<void>((r) => finish.push(r));
		});
		join.row(entry);
		join.resolved();
		const done = settledWithin(join, 1000);
		join.again();
		finish[0]();
		await Promise.resolve();
		await Promise.resolve();
		let settled = false;
		void done.then(() => (settled = true));
		await new Promise((r) => setTimeout(r, 5));
		expect(settled).toBe(false);
		finish[1]();
		expect(await done).toBe(true);
		expect(calls).toBe(2);
	});

	it('gives up after its bound on a refresh that never settles', async () => {
		const join = joinResumeRefresh(() => new Promise<void>(() => {}));
		join.row(entry);
		join.resolved();
		expect(await settledWithin(join, 20)).toBe(false);
	});
});

describe('joinResumeRefresh (asked again while something is out)', () => {
	it('leaves the run to a lookup that is out', () => {
		const run = vi.fn();
		const join = joinResumeRefresh(run);
		join.row(entry);
		join.resolved();
		join.unresolved();
		join.again();
		expect(run).toHaveBeenCalledTimes(1);
		join.resolved();
		expect(run).toHaveBeenCalledTimes(2);
	});

	it('runs once more after the refresh that is out, however often it is asked', async () => {
		const finish: (() => void)[] = [];
		const run = vi.fn(() => new Promise<void>((r) => finish.push(r)));
		const join = joinResumeRefresh(run);
		join.row(entry);
		join.resolved();
		join.again();
		join.again();
		join.again();
		expect(run).toHaveBeenCalledTimes(1);
		finish[0]();
		await new Promise((r) => setTimeout(r, 0));
		expect(run).toHaveBeenCalledTimes(2);
		finish[1]();
		await new Promise((r) => setTimeout(r, 0));
		expect(run).toHaveBeenCalledTimes(2);
	});
});

describe('settledWithin (the visit ends)', () => {
	it('stops waiting once the click no longer belongs to the visit', async () => {
		const join = joinResumeRefresh(() => new Promise<void>(() => {}));
		join.row(entry);
		join.resolved();
		let current = true;
		setTimeout(() => (current = false), 10);
		const started = Date.now();
		expect(await settledWithin(join, 5000, () => current)).toBe(false);
		expect(Date.now() - started).toBeLessThan(1000);
	});
});
