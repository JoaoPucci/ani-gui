import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { holdScroll } from './scroll-hold';

describe('holdScroll', () => {
	/** A window whose scroll position the test moves, recording each call. */
	const fakeWindow = (y: number) => {
		const calls: [string, number, number][] = [];
		const win = {
			scrollY: y,
			scrollBy: (dx: number, dy: number) => void calls.push(['by', dx, dy]),
			scrollTo: (x: number, ty: number) => void calls.push(['to', x, ty])
		};
		return { win, calls };
	};
	const above = { top: -500, height: 302 };

	it('scrolls to where the page was, less what the change gave back, even after the browser clamped it', () => {
		// At the bottom of the page: removing the room shortens it, and the
		// browser clamps 1000 to 698 before the compensation runs.
		const { win, calls } = fakeWindow(1000);
		const hold = holdScroll(win, above);
		win.scrollY = 698;
		hold(0);
		expect(calls).toEqual([['to', 0, 698]]);
	});

	it('targets the position before the change, whatever the browser did since', () => {
		const px = fc.integer({ min: 0, max: 5000 });
		const height = fc.integer({ min: 0, max: 600 });
		fc.assert(
			fc.property(px, px, height, height, (before, after, h, drawn) => {
				const { win, calls } = fakeWindow(before);
				const hold = holdScroll(win, { top: -1, height: h });
				win.scrollY = after;
				hold(drawn);
				const dy = drawn - h;
				expect(calls).toEqual(dy === 0 ? [] : [['to', 0, before + dy]]);
			})
		);
	});

	it('does not scroll for a room in view', () => {
		const { win, calls } = fakeWindow(1000);
		holdScroll(win, { top: 10, height: 302 })(0);
		expect(calls).toEqual([]);
	});
});
