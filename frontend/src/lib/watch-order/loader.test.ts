import { describe, expect, it, vi } from 'vitest';
import { rereadAnswer, visitRead } from './loader';

describe('visitRead', () => {
	it("hands on the route's answer as the page's view", async () => {
		expect(await visitRead(() => Promise.resolve('view'))).toEqual({ view: 'view' });
	});

	it('answers no view for a read that fails: the franchise could not be read', async () => {
		expect(await visitRead(() => Promise.reject(new Error('kitsu unreachable')))).toEqual({
			view: null
		});
	});

	it("passes the visit's signal on to the read", async () => {
		const visit = new AbortController();
		const read = vi.fn(async (signal?: AbortSignal) => signal);
		expect(await visitRead(read, visit.signal)).toEqual({ view: visit.signal });
		expect(read).toHaveBeenCalledWith(visit.signal);
	});

	it('reads nothing for a visit that has already ended, and answers nothing', async () => {
		const visit = new AbortController();
		visit.abort();
		const read = vi.fn(async () => 'view');
		expect(await visitRead(read, visit.signal)).toBeNull();
		expect(read).not.toHaveBeenCalled();
	});

	it('answers nothing for a visit that ended while its read was out, however the read ended', async () => {
		for (const outcome of [Promise.resolve('view'), Promise.reject(new Error('aborted'))]) {
			const visit = new AbortController();
			const answered = visitRead(() => {
				visit.abort();
				return outcome;
			}, visit.signal);
			expect(await answered).toBeNull();
		}
	});
});

describe('rereadAnswer', () => {
	it('draws a read again that read the history', () => {
		const fresh = { view: { history: true } };
		expect(rereadAnswer(fresh)).toBe(fresh);
	});

	it('keeps what is drawn when the read failed, found no franchise, or could not read the history', () => {
		expect(rereadAnswer(null)).toBeNull();
		expect(rereadAnswer({ view: null })).toBeNull();
		expect(rereadAnswer({ view: { history: false } })).toBeNull();
	});
});
