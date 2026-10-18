import { describe, it, expect } from 'vitest';
import { fallbackTileTitle } from './tile-title';

const episode = (n: string) => `Episode ${n}`;
const extra = (n: string) => `Extra ${n}`;

describe('fallbackTileTitle', () => {
	it('names a whole episode as the episode', () => {
		expect(fallbackTileTitle(13, episode, extra)).toBe('Episode 13');
		expect(fallbackTileTitle(1, episode, extra)).toBe('Episode 1');
	});

	it('names a fractional tile as an extra, by its own number', () => {
		expect(fallbackTileTitle(12.1, episode, extra)).toBe('Extra 12.1');
		expect(fallbackTileTitle(7.5, episode, extra)).toBe('Extra 7.5');
		expect(fallbackTileTitle(0.5, episode, extra)).toBe('Extra 0.5');
	});

	it('names a tile without a number as an episode without one', () => {
		expect(fallbackTileTitle(null, episode, extra)).toBe('Episode ');
	});
});
