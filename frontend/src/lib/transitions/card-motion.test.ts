// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { quintOut } from 'svelte/easing';

import { CARD_SCALE, CARD_SHIFT, cardScale, cardShift, STILL } from './card-motion';

const rect = (x: number) => new DOMRect(x, 0, 100, 150);

describe('card motion', () => {
	it("is the Continue Watching strip's delete motion: a 240 ms shrink-and-fade from 0.6", () => {
		const node = document.createElement('div');
		const out = cardScale(node);
		expect(CARD_SCALE).toEqual({ duration: 240, start: 0.6, opacity: 0, easing: quintOut });
		expect(out.duration).toBe(240);
		expect(out.easing).toBe(quintOut);
		// Halfway in, the card is part-shrunk and part-faded.
		expect(out.css?.(0.5, 0.5)).toMatch(/scale\(0\.8\)/);
	});

	it('slides a neighbour to its new place over 280 ms', () => {
		const node = document.createElement('div');
		const slide = cardShift(node, { from: rect(220), to: rect(0) });
		expect(CARD_SHIFT).toEqual({ duration: 280, easing: quintOut });
		expect(slide.easing).toBe(quintOut);
		expect(slide.duration).toBe(280);
	});

	it('holds still for a card the gate does not let move', () => {
		expect(STILL).toEqual({ duration: 0 });
	});
});
