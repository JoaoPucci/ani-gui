// Scrolling the app does for the user — a strip's arrow, a jump to an
// episode — glides with motion on and lands at once without it.
import fc from 'fast-check';
import { describe, expect, it, vi } from 'vitest';

const motion = vi.hoisted(() => ({ reduced: false }));
vi.mock('./reduced-motion.svelte', () => ({
	reducedMotion: {
		get current() {
			return motion.reduced;
		}
	}
}));

import { behaviorFor, scrollBehavior } from './scroll';

describe('behaviorFor', () => {
	it('glides only while motion is on', () => {
		fc.assert(
			fc.property(fc.boolean(), (reduced) => {
				expect(behaviorFor(reduced)).toBe(reduced ? 'auto' : 'smooth');
			})
		);
	});
});

describe('scrollBehavior', () => {
	it('follows the preference at the moment of the scroll', () => {
		motion.reduced = false;
		expect(scrollBehavior()).toBe('smooth');
		motion.reduced = true;
		expect(scrollBehavior()).toBe('auto');
		motion.reduced = false;
	});
});
