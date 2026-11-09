import { describe, expect, test, vi } from 'vitest';
import fc from 'fast-check';
import { createConfirmRun } from './confirm-run';

/** A run whose action settles when the test says. */
function held() {
	let resolve: () => void = () => {};
	let reject: (e: unknown) => void = () => {};
	const promise = new Promise<void>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

function harness() {
	const log: string[] = [];
	const run = createConfirmRun({
		setBusy: (busy) => log.push(`busy:${busy}`),
		close: () => log.push('close')
	});
	return { log, run };
}

describe('createConfirmRun', () => {
	test('a confirmed action that lands closes the dialog and reports nothing', async () => {
		const { log, run } = harness();
		const failed = vi.fn();
		await run.confirm(async () => {
			log.push('action');
		}, failed);
		expect(log).toEqual(['busy:true', 'action', 'busy:false', 'close']);
		expect(failed).not.toHaveBeenCalled();
	});

	test('a rejected action is reported, handled, and still closes the dialog', async () => {
		const { log, run } = harness();
		const error = new Error('refused');
		const failed = vi.fn((e: unknown) => log.push(`failed:${(e as Error).message}`));
		// The run settles: the rejection does not escape to the caller.
		await expect(run.confirm(() => Promise.reject(error), failed)).resolves.toBeUndefined();
		expect(failed).toHaveBeenCalledTimes(1);
		expect(failed).toHaveBeenCalledWith(error);
		// Reported before the dialog closes, so the message is up as
		// focus goes back to the control that opened it.
		expect(log).toEqual(['busy:true', 'failed:refused', 'busy:false', 'close']);
	});

	test('an action that throws before it returns a promise is reported the same way', async () => {
		const { log, run } = harness();
		const failed = vi.fn();
		await run.confirm(() => {
			throw new TypeError('Failed to fetch');
		}, failed);
		expect(failed).toHaveBeenCalledTimes(1);
		expect(log).toEqual(['busy:true', 'busy:false', 'close']);
		// The run is free again: a throw as the action is called settles
		// after the run is noted, so it cannot leave the run marked out.
		expect(run.busy).toBe(false);
		const next = vi.fn(async () => {});
		await run.confirm(next, failed);
		expect(next).toHaveBeenCalledTimes(1);
	});

	test('a second confirm while the first is in flight starts nothing', async () => {
		const { run } = harness();
		const h = held();
		const action = vi.fn(() => h.promise);
		const failed = vi.fn();
		const first = run.confirm(action, failed);
		const second = run.confirm(action, failed);
		expect(action).toHaveBeenCalledTimes(1);
		expect(run.busy).toBe(true);
		h.reject(new Error('down'));
		await Promise.all([first, second]);
		// One action, one report.
		expect(failed).toHaveBeenCalledTimes(1);
		expect(run.busy).toBe(false);
	});

	test('cancelling while the action is in flight keeps the dialog open until it settles', async () => {
		const { log, run } = harness();
		const h = held();
		const done = run.confirm(() => h.promise, vi.fn());
		run.cancel();
		expect(log).toEqual(['busy:true']);
		h.resolve();
		await done;
		expect(log).toEqual(['busy:true', 'busy:false', 'close']);
	});

	test('cancelling with nothing in flight closes the dialog', () => {
		const { log, run } = harness();
		run.cancel();
		expect(log).toEqual(['close']);
	});

	test('after a failure the user can confirm again, and that try runs', async () => {
		const { run } = harness();
		const failed = vi.fn();
		const action = vi
			.fn<() => Promise<void>>()
			.mockRejectedValueOnce(new Error('down'))
			.mockResolvedValueOnce(undefined);
		await run.confirm(action, failed);
		await run.confirm(action, failed);
		expect(action).toHaveBeenCalledTimes(2);
		expect(failed).toHaveBeenCalledTimes(1);
	});

	test('a report that throws still leaves the dialog closed and the run free', async () => {
		const { log, run } = harness();
		await run.confirm(
			() => Promise.reject(new Error('down')),
			() => {
				throw new Error('report broke');
			}
		);
		expect(log).toEqual(['busy:true', 'busy:false', 'close']);
		expect(run.busy).toBe(false);
	});

	test('property: whatever the outcomes and however many presses, each run ends closed and free, and every failure is reported once', async () => {
		await fc.assert(
			fc.asyncProperty(
				fc.array(fc.record({ fails: fc.boolean(), presses: fc.integer({ min: 1, max: 4 }) }), {
					maxLength: 8
				}),
				async (runs) => {
					const { log, run } = harness();
					const failed = vi.fn();
					let actions = 0;
					for (const { fails, presses } of runs) {
						log.length = 0;
						const action = () => {
							actions += 1;
							return fails ? Promise.reject(new Error('down')) : Promise.resolve();
						};
						await Promise.all(Array.from({ length: presses }, () => run.confirm(action, failed)));
						expect(log).toEqual(['busy:true', 'busy:false', 'close']);
						expect(run.busy).toBe(false);
					}
					expect(actions).toBe(runs.length);
					expect(failed).toHaveBeenCalledTimes(runs.filter((r) => r.fails).length);
				}
			)
		);
	});
});
