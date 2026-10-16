import { describe, expect, it } from 'vitest';
import { extraOnPage, nextEpisode, parseEpisodeNumber, previousEpisode } from './episode-number';

describe('parseEpisodeNumber', () => {
	it('keeps a fractional extra as the number it is', () => {
		// Play URLs carry `episode=7.5` for a recap; read as a whole
		// number it became episode 7, and everything keyed by it with it.
		expect(parseEpisodeNumber('7.5')).toBe(7.5);
		expect(parseEpisodeNumber(' 12.5 ')).toBe(12.5);
		expect(parseEpisodeNumber('0.5')).toBe(0.5);
	});

	it('reads a whole episode as before', () => {
		expect(parseEpisodeNumber('7')).toBe(7);
		expect(parseEpisodeNumber('1')).toBe(1);
	});

	it('has no episode for anything else', () => {
		for (const raw of [null, undefined, '', '  ', 'abc', '0', '-3', 'NaN', 'Infinity', '7abc']) {
			expect(parseEpisodeNumber(raw), String(raw)).toBeNull();
		}
	});
});

describe('previousEpisode / nextEpisode', () => {
	it('step a whole episode by one', () => {
		expect(previousEpisode(7)).toBe(6);
		expect(nextEpisode(7)).toBe(8);
	});

	it('step from a fractional extra to the whole episodes either side', () => {
		// The recap 7.5 sits between episodes 7 and 8; there is no 6.5
		// or 8.5 to go to.
		expect(previousEpisode(7.5)).toBe(7);
		expect(nextEpisode(7.5)).toBe(8);
		expect(previousEpisode(0.5)).toBe(0);
		expect(nextEpisode(0.5)).toBe(1);
	});
});

describe('extraOnPage', () => {
	it('places a recap on the page of the whole episode it follows', () => {
		// Pages of five: 1–5, 6–10. 5.5 follows 5, so page 1; 10.5
		// follows 10, so page 2.
		expect(extraOnPage(5.5, 1, 5)).toBe(true);
		expect(extraOnPage(5.5, 2, 5)).toBe(false);
		expect(extraOnPage(7.5, 2, 5)).toBe(true);
		expect(extraOnPage(10.5, 2, 5)).toBe(true);
		expect(extraOnPage(10.5, 3, 5)).toBe(false);
	});

	it('places a recap before episode 1 on page 1', () => {
		expect(extraOnPage(0.5, 1, 5)).toBe(true);
		expect(extraOnPage(0.5, 2, 5)).toBe(false);
	});
});
