import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { SettledLook } from './card-look';
import { createStillLandings, stillLanded } from './still-landing';

const POSTER = 'poster';
const look = (src: string | null, matchId = '1', episode = 4): SettledLook => ({
	image: src ? { src, fallback: src === POSTER ? null : POSTER } : null,
	matchId,
	episode
});

describe('stillLanded', () => {
	it('is a still arriving on the poster or the placeholder for the same show and episode', () => {
		expect(stillLanded(look(POSTER), look('still'), POSTER)).toBe(true);
		expect(stillLanded(look(null), look('still'), null)).toBe(true);
	});

	it('is not a still replacing another, a still lost, or another show or episode', () => {
		expect(stillLanded(look('a'), look('b'), POSTER)).toBe(false);
		expect(stillLanded(look('a'), look(POSTER), POSTER)).toBe(false);
		expect(stillLanded(look(POSTER), look('still', '2'), POSTER)).toBe(false);
		expect(stillLanded(look(POSTER), look('still', '1', 5), POSTER)).toBe(false);
	});

	it('holds only when the look before had no still and the look after has one', () => {
		const src = fc.option(fc.constantFrom(POSTER, 'a', 'b'), { nil: null });
		fc.assert(
			fc.property(src, src, fc.constantFrom('1', '2'), fc.constantFrom(4, 5), (b, a, m, e) => {
				const landed = stillLanded(look(b), look(a, m, e), POSTER);
				const hadStill = b !== null && b !== POSTER;
				const hasStill = a !== null && a !== POSTER;
				expect(landed).toBe(!hadStill && hasStill && m === '1' && e === 4);
			})
		);
	});
});

describe('createStillLandings', () => {
	it('hands back a still that lands on the look the details had settled on', () => {
		const landings = createStillLandings();
		expect(landings.observe('a', look(POSTER), look(POSTER), POSTER)).toBeNull();
		expect(landings.observe('a', look('still'), look(POSTER), POSTER)).toEqual(look('still'));
	});

	it('takes no still the details came with, or one the card is not showing the details for', () => {
		const landings = createStillLandings();
		// First details already carry a still: no landing.
		expect(landings.observe('a', look('still'), look(POSTER), POSTER)).toBeNull();
		// The card shows something other than what the details settled on.
		const other = createStillLandings();
		other.observe('a', look(POSTER), look(null), POSTER);
		expect(other.observe('a', look('still'), look(null), POSTER)).toBeNull();
		// Nothing on show yet.
		const none = createStillLandings();
		none.observe('a', look(POSTER), null, POSTER);
		expect(none.observe('a', look('still'), null, POSTER)).toBeNull();
	});

	it('forgets the details once they are out again, and keeps cards apart', () => {
		const landings = createStillLandings();
		landings.observe('a', look(POSTER), look(POSTER), POSTER);
		expect(landings.observe('a', null, look(POSTER), POSTER)).toBeNull();
		expect(landings.observe('a', look('still'), look(POSTER), POSTER)).toBeNull();
		landings.observe('b', look(POSTER), look(POSTER), POSTER);
		expect(landings.observe('c', look('still'), look(POSTER), POSTER)).toBeNull();
	});
});
