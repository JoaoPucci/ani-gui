/**
 * The fractional-episode reading must leave every whole episode where
 * it was. These cases compare each new reader and step with the
 * whole-number behaviour the app had before (parseInt, ±1, ceil paging,
 * counting tiles), over the strings the app and its backend write.
 */
import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import {
	extraOnPage,
	nextEpisode,
	parseEpisodeNumber,
	parseHistoryEpisode,
	previousEpisode,
	stripAnchor,
	wholeEpisodeCount
} from './episode-number';
import { decideAutoPlayNext } from './auto-play-next';
import { pickNextEpisode, pickResumeEpisode } from './next-episode';
import { StripPager } from './strip-pager';

const whole = fc.integer({ min: 1, max: 100_000 });

describe('a whole episode reads as it did', () => {
	it('from a play URL, written as the app writes it', () => {
		fc.assert(
			fc.property(whole, (n) => {
				// play-url.ts writes `episode=${n}`.
				return parseEpisodeNumber(String(n)) === parseInt(String(n), 10);
			})
		);
	});

	it('from a history row as the backend writes one: a slot, a translated slot, or an integer-valued tag', () => {
		fc.assert(
			fc.property(whole, (n) => {
				// read_ep_no can hand back a stamped tag verbatim ("8.0").
				return parseHistoryEpisode(`${n}.0`) === n && parseHistoryEpisode(String(n)) === n;
			})
		);
	});

	it('from a history row, padded or zero-led as rows may be', () => {
		fc.assert(
			fc.property(whole, fc.constantFrom('', ' ', '0', '00'), (n, pad) => {
				const raw = `${pad}${n}`;
				return parseHistoryEpisode(raw) === parseInt(raw, 10);
			})
		);
	});

	it('from a range-shaped history row, by its first episode', () => {
		fc.assert(fc.property(whole, whole, (a, b) => parseHistoryEpisode(`${a}-${b}`) === a));
	});
});

describe('a whole episode steps as it did', () => {
	it('prev and next move by one', () => {
		fc.assert(fc.property(whole, (n) => previousEpisode(n) === n - 1 && nextEpisode(n) === n + 1));
	});

	it('auto-play-next goes on by one and stops at the last episode', () => {
		fc.assert(
			fc.property(whole, fc.option(whole, { nil: null }), (n, total) => {
				const got = decideAutoPlayNext({ enabled: true, episodeNum: n, totalEpisodes: total });
				const before =
					total !== null && n >= total ? { advance: false } : { advance: true, target: n + 1 };
				expect(got).toEqual(before);
				return true;
			})
		);
	});

	it('Continue goes on by one, replays at the cap, and goes back when left part-way', () => {
		fc.assert(
			fc.property(whole, fc.option(whole, { nil: null }), fc.boolean(), (n, cap, left) => {
				const next = cap !== null && n + 1 > cap ? n : n + 1;
				return (
					pickNextEpisode(n, cap) === next && pickResumeEpisode(n, cap, left) === (left ? n : next)
				);
			})
		);
	});
});

describe('a whole episode pages as it did', () => {
	it('is its own anchor, and the strip pager opens it on the page ceil(n / size) names', () => {
		// Whole tiles are placed by the strip's own [start, end] window,
		// unchanged; only the pager's page for an episode, which a whole
		// episode shares with a recap's anchor, went through the new code.
		fc.assert(
			fc.property(whole, fc.integer({ min: 1, max: 50 }), (n, size) => {
				return stripAnchor(n) === n && new StripPager(size).pageOf(n) === Math.ceil(n / size);
			})
		);
	});

	it('the next episode is within the last episode exactly when the episode is before it', () => {
		// The play page's hasNext now reads next <= total for total < n.
		fc.assert(fc.property(whole, whole, (n, total) => nextEpisode(n) <= total === n < total));
	});

	it('a page without extras counts every tile, as it did', () => {
		fc.assert(
			fc.property(fc.array(whole, { maxLength: 30 }), fc.boolean(), (ns, relative) => {
				const tiles = ns.map((n) =>
					relative ? { number: null, relative_number: n } : { number: n, relative_number: null }
				);
				return wholeEpisodeCount(tiles) === tiles.length;
			})
		);
	});

	it('a recap beside whole episodes changes no whole episode’s count', () => {
		// Pages of five: a 5.5 recap joins page 1 and takes nothing from
		// its whole episodes; it lands on no other page.
		const page1 = [1, 2, 3, 4, 5, 5.5].map((number) => ({ number, relative_number: null }));
		expect(wholeEpisodeCount(page1)).toBe(5);
		expect(extraOnPage(5.5, 1, 5)).toBe(true);
		expect(extraOnPage(5.5, 2, 5)).toBe(false);
	});
});
