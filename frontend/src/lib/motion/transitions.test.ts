// The transition helpers every `transition:`, `in:`, `out:` and
// `animate:` directive in the app goes through. Under reduced motion
// they hand Svelte a transition with nothing to animate, which Svelte
// skips without starting an animation at all.
import fc from 'fast-check';
import { afterEach, describe, expect, it, vi } from 'vitest';

const motion = vi.hoisted(() => ({ reduced: false }));
vi.mock('./reduced-motion.svelte', () => ({
	reducedMotion: {
		get current() {
			return motion.reduced;
		}
	}
}));

import { fade, flip, fly, reducible, scale, slide, stillWhenReduced } from './transitions';

const node = {} as Element;

const timing = fc.record(
	{
		delay: fc.nat({ max: 10_000 }),
		duration: fc.nat({ max: 10_000 }),
		x: fc.integer()
	},
	{ requiredKeys: [] }
);

describe('stillWhenReduced', () => {
	it('leaves a transition untouched while motion is on', () => {
		fc.assert(
			fc.property(timing, (config) => {
				expect(stillWhenReduced(config, false)).toEqual(config);
			})
		);
	});

	it('takes every bit of time out of a transition under reduced motion', () => {
		fc.assert(
			fc.property(timing, (config) => {
				const still = stillWhenReduced(config, true);
				expect(still.duration).toBe(0);
				expect(still.delay).toBe(0);
			})
		);
	});

	it('keeps everything but the time under reduced motion', () => {
		fc.assert(
			fc.property(timing, (config) => {
				const still = stillWhenReduced(config, true);
				expect(still).toEqual({ ...config, duration: 0, delay: 0 });
			})
		);
	});

	it('stills a deferred transition when Svelte asks for it', () => {
		const deferred = () => ({ duration: 300, delay: 40 });
		const still = stillWhenReduced(deferred, true);
		expect(typeof still).toBe('function');
		expect((still as () => { duration: number; delay: number })()).toEqual({
			duration: 0,
			delay: 0
		});
	});
});

describe('reducible', () => {
	it('reads the preference each time the transition starts', () => {
		const inner = vi.fn((_node: Element, params: { duration: number }) => ({
			duration: params.duration,
			delay: 10
		}));
		const wrapped = reducible(inner);
		motion.reduced = false;
		expect(wrapped(node, { duration: 200 })).toEqual({ duration: 200, delay: 10 });
		motion.reduced = true;
		expect(wrapped(node, { duration: 200 })).toEqual({ duration: 0, delay: 0 });
		motion.reduced = false;
		expect(inner).toHaveBeenCalledWith(node, { duration: 200 });
	});
});

describe('the svelte transitions', () => {
	// Enough of a laid-out element for Svelte's own transitions to read.
	const box = { clientWidth: 10, clientHeight: 10, currentCSSZoom: 1 } as unknown as Element;
	const rect = { left: 0, top: 0, width: 10, height: 10 } as DOMRect;
	const computed = () =>
		new Proxy(
			{ opacity: '1', transform: 'none', transformOrigin: '5px 5px' } as Record<string, string>,
			{
				get: (style, key: string) => (key in style ? style[key] : '0px')
			}
		);
	const cases: [string, () => { duration?: number; delay?: number }][] = [
		['fade', () => fade(box, { duration: 160, delay: 5 })],
		['fly', () => fly(box, { y: 8, duration: 220, delay: 5 })],
		['scale', () => scale(box, { duration: 240, delay: 5 })],
		['slide', () => slide(box, { duration: 220, delay: 5 })],
		['flip', () => flip(box, { from: rect, to: rect }, { duration: 280, delay: 5 })]
	];

	afterEach(() => {
		vi.unstubAllGlobals();
		motion.reduced = false;
	});

	it.each(cases)('%s moves while motion is on', (_name, run) => {
		vi.stubGlobal('getComputedStyle', computed);
		const config = run();
		expect(config.delay).toBe(5);
		expect(config.duration).toBeGreaterThan(0);
	});

	it.each(cases)('%s stands still under reduced motion', (_name, run) => {
		motion.reduced = true;
		vi.stubGlobal('getComputedStyle', computed);
		const config = run();
		expect(config.duration).toBe(0);
		expect(config.delay).toBe(0);
	});
});
