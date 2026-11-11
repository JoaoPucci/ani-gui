import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	ARRIVE_MS,
	arriveCss,
	arriveOf,
	FIRST_HOP_CAP_MS,
	type FirstHop,
	heldShape,
	LEAVE_COLLAPSE_MS,
	LEAVE_FADE_MS,
	leaveOf,
	scrollBack,
	withFirstHop
} from './arrival';

/** A promise and the hands that settle it. */
function deferred<T>() {
	let resolve!: (v: T) => void;
	let reject!: (e: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

/** A wait the test ends by hand, recording what it was asked for. */
function waits() {
	const asked: number[] = [];
	const pending: (() => void)[] = [];
	return {
		asked,
		wait: (ms: number) => {
			asked.push(ms);
			return new Promise<void>((r) => pending.push(r));
		},
		end: () => pending.splice(0).forEach((r) => r())
	};
}

const turn = () => new Promise((r) => setTimeout(r, 0));

describe('waiting for the first hop with the details', () => {
	it('gives the first hop that answered before the details, without waiting', async () => {
		const w = waits();
		const got = await withFirstHop(
			Promise.resolve('d'),
			Promise.resolve<FirstHop>('strip'),
			w.wait
		);
		expect(got).toEqual({ details: 'd', hop: 'strip' });
	});

	it('waits for a first hop that answers within the cap after the details', async () => {
		const w = waits();
		const hop = deferred<FirstHop>();
		const done = withFirstHop(Promise.resolve('d'), hop.promise, w.wait);
		await turn();
		expect(w.asked).toEqual([FIRST_HOP_CAP_MS]);
		hop.resolve('link');
		expect(await done).toEqual({ details: 'd', hop: 'link' });
	});

	it('gives up on the first hop at the cap', async () => {
		const w = waits();
		const hop = deferred<FirstHop>();
		const done = withFirstHop(Promise.resolve('d'), hop.promise, w.wait);
		await turn();
		w.end();
		expect(await done).toEqual({ details: 'd', hop: null });
		// Too late: answered after the page was drawn.
		hop.resolve('strip');
	});

	it('draws without a first hop that failed', async () => {
		const w = waits();
		const got = await withFirstHop(
			Promise.resolve('d'),
			Promise.reject(new Error('offline')),
			w.wait
		);
		expect(got).toEqual({ details: 'd', hop: null });
	});

	it('stops waiting for the first hop once the walk has answered', async () => {
		// Nothing will be held then, so the page need not wait.
		const w = waits();
		const hop = deferred<FirstHop>();
		const got = await withFirstHop(Promise.resolve('d'), hop.promise, w.wait, Promise.resolve());
		expect(got).toEqual({ details: 'd', hop: null });
		hop.resolve('strip');
	});

	it('stops waiting too when the walk failed, and still draws the details', async () => {
		const w = waits();
		const hop = deferred<FirstHop>();
		const got = await withFirstHop(
			Promise.resolve('d'),
			hop.promise,
			w.wait,
			Promise.reject(new Error('offline'))
		);
		expect(got).toEqual({ details: 'd', hop: null });
		hop.resolve('strip');
	});

	it('fails as the details do', async () => {
		const w = waits();
		await expect(
			withFirstHop(Promise.reject(new Error('gone')), Promise.resolve<FirstHop>('strip'), w.wait)
		).rejects.toThrow('gone');
	});

	it('waits for the details however long the first hop takes', async () => {
		const w = waits();
		const details = deferred<string>();
		const done = withFirstHop(details.promise, Promise.resolve<FirstHop>('none'), w.wait);
		await turn();
		details.resolve('d');
		expect(await done).toEqual({ details: 'd', hop: 'none' });
	});
});

describe('the room held while the walk runs', () => {
	it('holds what the first hop foretold until the walk answers', () => {
		expect(heldShape('strip', false)).toBe('strip');
		expect(heldShape('link', false)).toBe('link');
		expect(heldShape('strip', true)).toBeNull();
		expect(heldShape('link', true)).toBeNull();
	});

	it('holds nothing when the first hop foretold nothing or did not come', () => {
		expect(heldShape('none', false)).toBeNull();
		expect(heldShape(null, false)).toBeNull();
	});
});

describe('the held room giving way', () => {
	const inView = { top: 400, height: 302 };
	const above = { top: -500, height: 302 };

	it('goes at once where the strip or link takes its place', () => {
		expect(leaveOf(true, inView)).toEqual({ fade: 0, collapse: 0 });
	});

	it('fades, then folds away, when nothing comes', () => {
		expect(leaveOf(false, inView)).toEqual({ fade: LEAVE_FADE_MS, collapse: LEAVE_COLLAPSE_MS });
		expect(leaveOf(false, null)).toEqual({ fade: LEAVE_FADE_MS, collapse: LEAVE_COLLAPSE_MS });
	});

	it('goes at once when it is above the viewport', () => {
		expect(leaveOf(false, above)).toEqual({ fade: 0, collapse: 0 });
	});

	it('fades the strip in from the held height', () => {
		expect(arriveOf(inView)).toEqual({ fade: ARRIVE_MS, from: 302 });
	});

	it('draws the strip at once where nothing was held, or the room was above the viewport', () => {
		expect(arriveOf(null)).toEqual({ fade: 0, from: null });
		expect(arriveOf(above)).toEqual({ fade: 0, from: null });
	});

	it('scrolls back by what an above-the-viewport change gave back', () => {
		expect(scrollBack(above, 0)).toBe(-302);
		expect(scrollBack(above, 70)).toBe(-232);
		expect(scrollBack(above, 302)).toBe(0);
		expect(scrollBack(inView, 0)).toBe(0);
		expect(scrollBack(null, 302)).toBe(0);
	});

	it('scrolls only for an above-the-viewport room, by exactly the height change', () => {
		const px = fc.integer({ min: -2000, max: 2000 });
		const height = fc.integer({ min: 0, max: 600 });
		fc.assert(
			fc.property(px, height, height, (top, h, drawn) => {
				const dy = scrollBack({ top, height: h }, drawn);
				expect(dy).toBe(top < 0 ? drawn - h : 0);
			})
		);
	});
});

describe('the arriving block’s style', () => {
	it('only fades where the heights match or nothing was held', () => {
		expect(arriveCss(null, 302, 0.5)).toBe('opacity: 0.5;');
		expect(arriveCss(302, 302, 0.5)).toBe('opacity: 0.5;');
	});

	it('moves from the held height to its own', () => {
		expect(arriveCss(302, 70, 0)).toBe('opacity: 0; block-size: 302px; overflow: hidden;');
		expect(arriveCss(302, 70, 0.5)).toBe('opacity: 0.5; block-size: 186px; overflow: hidden;');
		expect(arriveCss(302, 70, 1)).toBe('opacity: 1; block-size: 70px; overflow: hidden;');
	});
});
