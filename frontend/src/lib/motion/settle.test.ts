import { afterEach, describe, expect, it, vi } from 'vitest';

// The preference comes from the app's one reduced-motion source; the
// test sets it by hand.
const motion = vi.hoisted(() => ({ reduced: false }));
vi.mock('./reduced-motion.svelte', () => ({
	reducedMotion: {
		get current() {
			return motion.reduced;
		}
	}
}));

import { settle, settleOut } from './settle';

const dummyNode = {} as Element;

afterEach(() => {
	motion.reduced = false;
});

describe('settle (entrance transition)', () => {
	it('uses default delay 0 + duration 620 when no opts are given', () => {
		const r = settle(dummyNode);
		expect(r.delay).toBe(0);
		expect(r.duration).toBe(620);
	});

	it('honours delay and duration overrides', () => {
		const r = settle(dummyNode, { delay: 80, duration: 400 });
		expect(r.delay).toBe(80);
		expect(r.duration).toBe(400);
	});

	it('starts at opacity 0 + scale 0.9 + 28px down + 8px blur (t=0)', () => {
		// css(t=0, u=1) is the entrance starting frame. Asserting the
		// boundary values pins the gesture's intent in case anyone
		// tweaks the curve and accidentally drops the blur or shifts
		// the starting offset.
		const css = settle(dummyNode).css(0, 1);
		expect(css).toContain('opacity: 0');
		expect(css).toContain('translateY(28px)');
		expect(css).toContain('scale(0.9)');
		expect(css).toContain('blur(8px)');
	});

	it('rests at opacity 1 + scale 1 + 0px + 0 blur (t=1)', () => {
		const css = settle(dummyNode).css(1, 0);
		expect(css).toContain('opacity: 1');
		expect(css).toContain('translateY(0px)');
		// 0.9 + 1*0.1 = 1
		expect(css).toContain('scale(1)');
		expect(css).toContain('blur(0px)');
	});

	it('stands still under reduced motion, stagger delay included', () => {
		// Reduced motion means no motion: not a shorter settle and not
		// a staggered pop-in, which a kept delay would still produce.
		motion.reduced = true;
		const r = settle(dummyNode, { delay: 90, duration: 620 });
		expect(r.duration).toBe(0);
		expect(r.delay).toBe(0);
	});
});

describe('settleOut (exit transition)', () => {
	it('uses default delay 0 + duration 320', () => {
		// Shorter than settle's 620 — old tiles clear before new
		// ones finish landing.
		const r = settleOut(dummyNode);
		expect(r.delay).toBe(0);
		expect(r.duration).toBe(320);
	});

	it('drops upward (negative translateY) and uses milder blur than settle', () => {
		// settleOut mirrors the curve but shifts by -16px and 4px
		// blur — distinct from settle's +28 / 8 so the exit feels
		// like a different gesture.
		const css = settleOut(dummyNode).css(0, 1);
		expect(css).toContain('translateY(-16px)');
		expect(css).toContain('blur(4px)');
	});

	it('rests at full opacity / scale 1 / no offset / no blur (t=1)', () => {
		const css = settleOut(dummyNode).css(1, 0);
		expect(css).toContain('opacity: 1');
		// `${0 * -16}px` stringifies to `0px`, not `-0px`.
		expect(css).toContain('translateY(0px)');
		// 0.94 + 1*0.06 = 1
		expect(css).toContain('scale(1)');
		expect(css).toContain('blur(0px)');
	});

	it('stands still under reduced motion, stagger delay included', () => {
		motion.reduced = true;
		const r = settleOut(dummyNode, { delay: 36, duration: 320 });
		expect(r.duration).toBe(0);
		expect(r.delay).toBe(0);
	});
});
