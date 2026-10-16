import { describe, expect, it } from 'vitest';
import { createProbeLimiter } from './probe-limiter';

/** A task the test settles by hand, recording whether it started. */
function gate() {
	let release!: (v: string) => void;
	let fail!: (e: unknown) => void;
	const state = { started: false, signal: undefined as AbortSignal | undefined };
	const task = (signal: AbortSignal | undefined) => {
		state.started = true;
		state.signal = signal;
		return new Promise<string>((res, rej) => {
			release = res;
			fail = rej;
		});
	};
	return {
		task,
		state,
		release: (v = 'ok') => release(v),
		fail: (e: unknown) => fail(e)
	};
}

const tick = () => new Promise((r) => setTimeout(r, 0));

describe('createProbeLimiter', () => {
	it('runs at most `max` at once and starts the rest in order as slots free', async () => {
		const limiter = createProbeLimiter({ max: 2, backgroundMax: 2 });
		const g = [gate(), gate(), gate()];
		const runs = g.map((x) => limiter.run(x.task, { background: false }));
		await tick();
		expect(g.map((x) => x.state.started)).toEqual([true, true, false]);
		expect(limiter.inFlight()).toBe(2);
		g[0].release('a');
		expect(await runs[0]).toBe('a');
		await tick();
		expect(g[2].state.started).toBe(true);
		g[1].release();
		g[2].release();
		await Promise.all(runs);
		expect(limiter.inFlight()).toBe(0);
	});

	it('keeps a slot from background probes for an interactive one', async () => {
		const limiter = createProbeLimiter({ max: 3, backgroundMax: 2 });
		const bg = [gate(), gate(), gate()];
		bg.forEach((x) => void limiter.run(x.task, { background: true }).catch(() => {}));
		await tick();
		expect(bg.map((x) => x.state.started)).toEqual([true, true, false]);
		const click = gate();
		void limiter.run(click.task, { background: false });
		await tick();
		expect(click.state.started).toBe(true);
	});

	it('serves a waiting interactive probe before a waiting background one', async () => {
		const limiter = createProbeLimiter({ max: 1, backgroundMax: 1 });
		const first = gate();
		void limiter.run(first.task, { background: true });
		const bg = gate();
		void limiter.run(bg.task, { background: true });
		const click = gate();
		void limiter.run(click.task, { background: false });
		await tick();
		first.release();
		await tick();
		expect(click.state.started).toBe(true);
		expect(bg.state.started).toBe(false);
	});

	it('a waiting probe whose signal aborts leaves the queue without running', async () => {
		const limiter = createProbeLimiter({ max: 1, backgroundMax: 1 });
		const busy = gate();
		void limiter.run(busy.task, { background: false });
		const left = gate();
		const ctrl = new AbortController();
		const run = limiter.run(left.task, { background: false, signal: ctrl.signal });
		ctrl.abort();
		await expect(run).rejects.toMatchObject({ name: 'AbortError' });
		busy.release();
		await tick();
		expect(left.state.started).toBe(false);
		expect(limiter.inFlight()).toBe(0);
	});

	it('a probe already aborted never runs', async () => {
		const limiter = createProbeLimiter({ max: 1, backgroundMax: 1 });
		const g = gate();
		const ctrl = new AbortController();
		ctrl.abort();
		await expect(
			limiter.run(g.task, { background: false, signal: ctrl.signal })
		).rejects.toMatchObject({ name: 'AbortError' });
		expect(g.state.started).toBe(false);
	});

	it('hands a running probe its signal, and a failure frees its slot', async () => {
		const limiter = createProbeLimiter({ max: 1, backgroundMax: 1 });
		const ctrl = new AbortController();
		const g = gate();
		const run = limiter.run(g.task, { background: false, signal: ctrl.signal });
		await tick();
		expect(g.state.signal).toBe(ctrl.signal);
		g.fail(new Error('boom'));
		await expect(run).rejects.toThrow('boom');
		const next = gate();
		void limiter.run(next.task, { background: false });
		await tick();
		expect(next.state.started).toBe(true);
	});
});
