import { afterEach, describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import { holdScroll, windowScroll } from './scroll-hold';

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

describe('windowScroll', () => {
	/** A stand-in window: its scroll position and the calls made on it. */
	const stubWindow = () => {
		const page = {
			scrollY: 0,
			calls: [] as { self: unknown; x: number; y: number }[],
			scrollTo(x: number, y: number) {
				page.calls.push({ self: this, x, y });
			}
		};
		vi.stubGlobal('window', page);
		return page;
	};

	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it("reads the window's scroll position live, not when it was made", () => {
		const page = stubWindow();
		const win = windowScroll();
		page.scrollY = 400;
		expect(win.scrollY).toBe(400);
		page.scrollY = 120;
		expect(win.scrollY).toBe(120);
	});

	it('scrolls the window through its own call, bound to it', () => {
		const page = stubWindow();
		const { scrollTo } = windowScroll();
		scrollTo(0, 698);
		expect(page.calls).toEqual([{ self: page, x: 0, y: 698 }]);
	});
});
