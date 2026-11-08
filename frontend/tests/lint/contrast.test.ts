import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	composite,
	contrastRatio,
	parseHex,
	relativeLuminance,
	textContrast,
	type Rgb
} from './contrast';

const channel = fc.integer({ min: 0, max: 255 });
const rgb = fc.tuple(channel, channel, channel) as fc.Arbitrary<Rgb>;
const alpha = fc.double({ min: 0, max: 1, noNaN: true });

const BLACK: Rgb = [0, 0, 0];
const WHITE: Rgb = [255, 255, 255];

describe('parseHex', () => {
	it('reads long and short forms', () => {
		expect(parseHex('#0c0a09')).toEqual([12, 10, 9]);
		expect(parseHex('#F4ECE1')).toEqual([244, 236, 225]);
		expect(parseHex('#fff')).toEqual([255, 255, 255]);
	});

	it('refuses anything that is not a hex colour', () => {
		for (const bad of ['0c0a09', '#0c0a0', '#0c0a0911', 'var(--ink-000)', '#ggg', ''])
			expect(() => parseHex(bad)).toThrow();
	});

	it('round-trips every colour written as #rrggbb', () => {
		fc.assert(
			fc.property(rgb, (c) => {
				const hex = '#' + c.map((v) => v.toString(16).padStart(2, '0')).join('');
				expect(parseHex(hex)).toEqual(c);
			})
		);
	});
});

describe('relativeLuminance', () => {
	it('is 0 for black and 1 for white', () => {
		expect(relativeLuminance(BLACK)).toBe(0);
		expect(relativeLuminance(WHITE)).toBeCloseTo(1, 10);
	});

	it('matches the WCAG reference value for a mid grey', () => {
		// #777777 is the classic 4.48:1-on-white grey.
		expect(contrastRatio(parseHex('#777777'), WHITE)).toBeCloseTo(4.48, 2);
	});

	it('never decreases when any channel brightens', () => {
		fc.assert(
			fc.property(rgb, fc.integer({ min: 0, max: 2 }), (c, i) => {
				const brighter = [...c] as [number, number, number];
				brighter[i] = Math.min(255, brighter[i] + 1);
				expect(relativeLuminance(brighter)).toBeGreaterThanOrEqual(relativeLuminance(c));
			})
		);
	});
});

describe('contrastRatio', () => {
	it('is 21 for black on white', () => {
		expect(contrastRatio(BLACK, WHITE)).toBeCloseTo(21, 10);
	});

	it('runs from 1 to 21, is 1 for a colour on itself, and ignores order', () => {
		fc.assert(
			fc.property(rgb, rgb, (a, b) => {
				const r = contrastRatio(a, b);
				expect(r).toBeGreaterThanOrEqual(1);
				expect(r).toBeLessThanOrEqual(21 + 1e-9);
				expect(contrastRatio(b, a)).toBe(r);
				expect(contrastRatio(a, a)).toBe(1);
			})
		);
	});
});

describe('composite', () => {
	it('is the surface at alpha 0 and the colour at alpha 1', () => {
		fc.assert(
			fc.property(rgb, rgb, (fg, bg) => {
				expect(composite(fg, 0, bg)).toEqual(bg);
				expect(composite(fg, 1, bg)).toEqual(fg);
			})
		);
	});

	it('refuses an alpha outside 0–1', () => {
		expect(() => composite(WHITE, 1.01, BLACK)).toThrow();
		expect(() => composite(WHITE, -0.01, BLACK)).toThrow();
		expect(() => composite(WHITE, Number.NaN, BLACK)).toThrow();
	});

	it('stays between the colour and the surface on every channel', () => {
		fc.assert(
			fc.property(rgb, alpha, rgb, (fg, a, bg) => {
				composite(fg, a, bg).forEach((v, i) => {
					expect(v).toBeGreaterThanOrEqual(Math.min(fg[i], bg[i]) - 1e-9);
					expect(v).toBeLessThanOrEqual(Math.max(fg[i], bg[i]) + 1e-9);
				});
			})
		);
	});
});

describe('textContrast', () => {
	it('fades to 1 as light text fades out, and never gains contrast by fading', () => {
		// The property the design tests lean on: lowering the alpha of a
		// light text colour over a darker surface can only lose contrast.
		// It is stated for text at least as light as its surface on every
		// channel, which is the app's case (bone on ink); with channels
		// moving in opposite directions luminance need not be monotonic.
		const lightOnDark = fc
			.tuple(rgb, rgb)
			.map(
				([surface, lift]) =>
					[surface, surface.map((v, i) => Math.min(255, v + lift[i])) as unknown as Rgb] as const
			);
		fc.assert(
			fc.property(lightOnDark, alpha, alpha, ([surface, text], a, b) => {
				const [lo, hi] = a <= b ? [a, b] : [b, a];
				expect(textContrast(text, lo, surface)).toBeLessThanOrEqual(
					textContrast(text, hi, surface) + 1e-9
				);
				expect(textContrast(text, 0, surface)).toBe(1);
			})
		);
	});

	it('measures a known translucent case', () => {
		// Bone text at 45% over the page ink, the poster-card meta before
		// the fix: about 4.0:1, short of AA for 12px text.
		const ratio = textContrast(parseHex('#f4ece1'), 0.45, parseHex('#0c0a09'));
		expect(ratio).toBeGreaterThan(3.9);
		expect(ratio).toBeLessThan(4.1);
	});
});
