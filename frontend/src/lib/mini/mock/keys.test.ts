import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { decideMiniKeyAction, isMiniShortcut } from './keys';
import { PLAYER_SEEK_STEP_SECONDS } from '$lib/play/keyboard';

const ctx = {
	key: ' ',
	inField: false,
	inButton: false,
	modifier: false,
	repeat: false,
	menuOpen: false
};

describe('the mini window keys', () => {
	it('keeps the main player map for play, seek, volume, episodes and mute', () => {
		expect(decideMiniKeyAction({ ...ctx, key: ' ' })).toEqual({ kind: 'togglePlay' });
		expect(decideMiniKeyAction({ ...ctx, key: 'ArrowLeft' })).toEqual({
			kind: 'seek',
			deltaSeconds: -PLAYER_SEEK_STEP_SECONDS
		});
		expect(decideMiniKeyAction({ ...ctx, key: 'ArrowUp' })?.kind).toBe('volume');
		expect(decideMiniKeyAction({ ...ctx, key: 'n' })).toEqual({ kind: 'next' });
		expect(decideMiniKeyAction({ ...ctx, key: 'P' })).toEqual({ kind: 'prev' });
		expect(decideMiniKeyAction({ ...ctx, key: 'm' })).toEqual({ kind: 'mute' });
	});

	it('returns to the app on F, where the main player goes fullscreen', () => {
		expect(decideMiniKeyAction({ ...ctx, key: 'f' })).toEqual({ kind: 'return' });
		expect(decideMiniKeyAction({ ...ctx, key: 'F' })).toEqual({ kind: 'return' });
		expect(decideMiniKeyAction({ ...ctx, key: 'f', repeat: true })).toBeNull();
	});

	it('closes an open menu on Escape, and otherwise ignores it', () => {
		expect(decideMiniKeyAction({ ...ctx, key: 'Escape', menuOpen: true })).toEqual({
			kind: 'closeMenu'
		});
		expect(decideMiniKeyAction({ ...ctx, key: 'Escape' })).toBeNull();
	});

	it('never yields fullscreen, and Escape never does more than close a menu', () => {
		fc.assert(
			fc.property(
				fc.record({
					key: fc.oneof(
						fc.constantFrom(' ', 'Escape', 'f', 'F', 'n', 'm', 'ArrowRight', 'i'),
						fc.string()
					),
					inField: fc.boolean(),
					inButton: fc.boolean(),
					modifier: fc.boolean(),
					repeat: fc.boolean(),
					menuOpen: fc.boolean()
				}),
				(c) => {
					const action = decideMiniKeyAction(c);
					expect(action?.kind).not.toBe('fullscreen');
					if (c.key === 'Escape') expect([null, 'closeMenu']).toContain(action?.kind ?? null);
				}
			)
		);
	});

	it('opens the mini player from the main player on I, under the letter-key rules', () => {
		expect(isMiniShortcut({ ...ctx, key: 'i' })).toBe(true);
		expect(isMiniShortcut({ ...ctx, key: 'I' })).toBe(true);
		expect(isMiniShortcut({ ...ctx, key: 'i', inField: true })).toBe(false);
		expect(isMiniShortcut({ ...ctx, key: 'i', modifier: true })).toBe(false);
		expect(isMiniShortcut({ ...ctx, key: 'i', repeat: true })).toBe(false);
		expect(isMiniShortcut({ ...ctx, key: 'f' })).toBe(false);
	});
});
