import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { onTopView } from './on-top';

describe('how the window speaks about staying on top', () => {
	it('shows the card once on Wayland, then never again', () => {
		expect(onTopView('wayland', 'card', false)).toEqual({ card: true, pin: null });
		expect(onTopView('wayland', 'card', true)).toEqual({ card: false, pin: null });
	});

	it('makes the pin explain on Wayland, and toggle elsewhere', () => {
		expect(onTopView('wayland', 'pin', false)).toEqual({ card: false, pin: 'explain' });
		expect(onTopView('windows', 'pin', false)).toEqual({ card: false, pin: 'toggle' });
		expect(onTopView('x11', 'both', false)).toEqual({ card: false, pin: 'toggle' });
		expect(onTopView('wayland', 'both', false)).toEqual({ card: true, pin: 'explain' });
	});

	it('never shows the card where the app can keep the window on top', () => {
		fc.assert(
			fc.property(
				fc.constantFrom('windows' as const, 'x11' as const),
				fc.constantFrom('card' as const, 'pin' as const, 'both' as const),
				fc.boolean(),
				(platform, choice, seen) => {
					const view = onTopView(platform, choice, seen);
					expect(view.card).toBe(false);
					expect(view.pin).not.toBe('explain');
				}
			)
		);
	});
});
