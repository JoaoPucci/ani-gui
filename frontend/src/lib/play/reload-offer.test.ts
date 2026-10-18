import { describe, expect, it } from 'vitest';
import { deadEnd } from './reload-offer';

const OPEN = { listed: true, unaired: false, beyondPlayable: false };

describe('deadEnd — the episodes a play cannot reach', () => {
	it('is null when nothing known keeps the play from the episode', () => {
		expect(deadEnd(OPEN)).toBeNull();
		// An unanswered listing claims nothing.
		expect(deadEnd({ ...OPEN, listed: null })).toBeNull();
	});

	it('names each refusal', () => {
		expect(deadEnd({ ...OPEN, listed: false })).toBe('delisted');
		expect(deadEnd({ ...OPEN, unaired: true })).toBe('unaired');
		expect(deadEnd({ ...OPEN, beyondPlayable: true })).toBe('beyond-playable');
	});

	it('puts a delisted show ahead of what is said about its episodes', () => {
		// A delisted show keeps whatever cap it had, so its episodes'
		// standing says nothing about why the play is refused.
		expect(deadEnd({ listed: false, unaired: true, beyondPlayable: true })).toBe('delisted');
		expect(deadEnd({ listed: true, unaired: true, beyondPlayable: true })).toBe('unaired');
	});
});
