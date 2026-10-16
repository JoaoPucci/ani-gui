import { describe, expect, it } from 'vitest';
import { createDownloadQueue, DOWNLOAD_CONCURRENCY } from './queue';

function held() {
	let finish!: (v: string) => void;
	let fail!: (e: unknown) => void;
	const state = { started: false, ran: false };
	const task = () => {
		state.ran = true;
		return new Promise<string>((res, rej) => {
			finish = res;
			fail = rej;
		});
	};
	const onStart = () => {
		state.started = true;
	};
	return { task, onStart, state, finish: (v = 'ok') => finish(v), fail: (e: unknown) => fail(e) };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

describe('createDownloadQueue', () => {
	it('starts at most `max` at once, at once, and the rest in order as each ends', async () => {
		const queue = createDownloadQueue(2);
		const d = [held(), held(), held(), held()];
		const runs = d.map((x) =>
			queue.run(x.task, { signal: new AbortController().signal, onStart: x.onStart })
		);
		// Given a slot, a download starts in the same turn it was asked.
		expect(d.map((x) => x.state.started)).toEqual([true, true, false, false]);
		expect(d.map((x) => x.state.ran)).toEqual([true, true, false, false]);
		d[1].finish('b');
		expect(await runs[1]).toBe('b');
		await tick();
		expect(d.map((x) => x.state.started)).toEqual([true, true, true, false]);
		d[0].fail(new Error('boom'));
		await expect(runs[0]).rejects.toThrow('boom');
		await tick();
		expect(d[3].state.started).toBe(true);
	});

	it('a waiting download that is cancelled leaves the queue without ever starting', async () => {
		const queue = createDownloadQueue(1);
		const busy = held();
		void queue.run(busy.task, { signal: new AbortController().signal, onStart: busy.onStart });
		const waiting = held();
		const ctrl = new AbortController();
		const run = queue.run(waiting.task, { signal: ctrl.signal, onStart: waiting.onStart });
		ctrl.abort();
		await expect(run).rejects.toMatchObject({ name: 'AbortError' });
		const next = held();
		void queue.run(next.task, { signal: new AbortController().signal, onStart: next.onStart });
		busy.finish();
		await tick();
		expect(waiting.state.ran).toBe(false);
		expect(waiting.state.started).toBe(false);
		expect(next.state.started).toBe(true);
	});

	it('runs as many at once as the download lane has connections', () => {
		// Each running download holds one of the six connections
		// Chromium opens to the download lane for its progress stream;
		// a seventh would wait for one with nothing saying so.
		expect(DOWNLOAD_CONCURRENCY).toBe(6);
	});
});
