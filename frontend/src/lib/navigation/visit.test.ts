import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import { beginVisit, visitSeries } from './visit';

/** Lets every settled promise run its callbacks. */
const flush = () => new Promise((r) => setTimeout(r, 0));

describe('beginVisit', () => {
	it('starts with a signal that has not been aborted', () => {
		const visit = beginVisit();
		expect(visit.signal.aborted).toBe(false);
		expect(visit.ended).toBe(false);
	});

	it('aborts its signal when the visit ends, so its reads are cancelled', () => {
		const visit = beginVisit();
		const onAbort = vi.fn();
		visit.signal.addEventListener('abort', onAbort);
		visit.end();
		expect(visit.signal.aborted).toBe(true);
		expect(visit.ended).toBe(true);
		expect(onAbort).toHaveBeenCalledTimes(1);
	});

	it('ends once: a second end changes nothing', () => {
		const visit = beginVisit();
		const onAbort = vi.fn();
		visit.signal.addEventListener('abort', onAbort);
		visit.end();
		visit.end();
		expect(onAbort).toHaveBeenCalledTimes(1);
	});

	it('gives each visit a signal of its own', () => {
		const left = beginVisit();
		const next = beginVisit();
		left.end();
		expect(next.signal.aborted).toBe(false);
	});
});

describe('Visit.keep', () => {
	it("passes a read's answer on while the visit lasts", async () => {
		const visit = beginVisit();
		await expect(visit.keep(Promise.resolve(7))).resolves.toBe(7);
	});

	it("passes a read's failure on while the visit lasts", async () => {
		const visit = beginVisit();
		const failure = { kind: 'http', status: 503 };
		await expect(visit.keep(Promise.reject(failure))).rejects.toBe(failure);
	});

	it('answers nothing, neither the value nor an error, for a visit that ended first', async () => {
		const visit = beginVisit();
		let answer!: (v: number) => void;
		const read = new Promise<number>((r) => (answer = r));
		const then = vi.fn();
		const caught = vi.fn();
		visit.keep(read).then(then, caught);
		visit.end();
		answer(1);
		await flush();
		expect(then).not.toHaveBeenCalled();
		expect(caught).not.toHaveBeenCalled();
	});

	it('answers nothing for the abort a cancelled read rejects with', async () => {
		const visit = beginVisit();
		const read = new Promise<number>((_, reject) =>
			visit.signal.addEventListener('abort', () =>
				reject(new DOMException('The operation was aborted.', 'AbortError'))
			)
		);
		const caught = vi.fn();
		visit.keep(read).catch(caught);
		visit.end();
		await flush();
		expect(caught).not.toHaveBeenCalled();
	});

	it('answers nothing for a read kept after the visit ended', async () => {
		const visit = beginVisit();
		visit.end();
		const then = vi.fn();
		const caught = vi.fn();
		visit.keep(Promise.resolve(1)).then(then, caught);
		visit.keep(Promise.reject(new Error('x'))).then(then, caught);
		await flush();
		expect(then).not.toHaveBeenCalled();
		expect(caught).not.toHaveBeenCalled();
	});

	it('property: an answer is passed on exactly when it lands before the visit ends', async () => {
		await fc.assert(
			fc.asyncProperty(fc.boolean(), fc.boolean(), async (endFirst, fails) => {
				const visit = beginVisit();
				let settle!: () => void;
				const read = new Promise<string>((resolve, reject) => {
					settle = () => (fails ? reject(new Error('no')) : resolve('yes'));
				});
				const seen: string[] = [];
				visit.keep(read).then(
					() => seen.push('value'),
					() => seen.push('error')
				);
				if (endFirst) visit.end();
				settle();
				await flush();
				if (!endFirst) visit.end();
				await flush();
				expect(seen).toEqual(endFirst ? [] : [fails ? 'error' : 'value']);
			})
		);
	});
});

describe('visitSeries', () => {
	it('ends the visit before when the next begins', () => {
		const series = visitSeries();
		const first = series.next();
		const second = series.next();
		expect(first.ended).toBe(true);
		expect(second.ended).toBe(false);
	});

	it('ends the current visit when the page goes', () => {
		const series = visitSeries();
		const current = series.next();
		series.end();
		expect(current.ended).toBe(true);
	});

	it('ends nothing when no visit has begun', () => {
		expect(() => visitSeries().end()).not.toThrow();
	});
});
