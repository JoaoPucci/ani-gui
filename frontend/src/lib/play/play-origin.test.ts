import { describe, expect, it } from 'vitest';
import { openedFromGuess, recordableId, withGuess } from './play-origin';

describe('a play session opened from a guessed Continue match', () => {
	it('is told by the guess flag in its URL', () => {
		expect(openedFromGuess(new URLSearchParams('session=s&episode=3&guess=1'))).toBe(true);
		expect(openedFromGuess(new URLSearchParams('session=s&episode=3'))).toBe(false);
		expect(openedFromGuess(new URLSearchParams('guess=0'))).toBe(false);
	});

	it('records no Kitsu id, and any other session records its own', () => {
		expect(recordableId('42', true)).toBeUndefined();
		expect(recordableId('42', false)).toBe('42');
		expect(recordableId('', false)).toBeUndefined();
	});

	it('carries the flag on every URL it builds for itself', () => {
		const q = withGuess('?session=s&episode=3', true);
		expect(new URLSearchParams(q.slice(1)).get('guess')).toBe('1');
		expect(new URLSearchParams(q.slice(1)).get('session')).toBe('s');
		expect(withGuess('?session=s&episode=3', false)).toBe('?session=s&episode=3');
	});
});
