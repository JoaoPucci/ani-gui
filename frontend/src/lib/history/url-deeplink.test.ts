import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';
import { decideEpisodeFetchAction, parsePageParam } from './url-deeplink';

const params = (qs: string) => new URLSearchParams(qs);

describe('parsePageParam', () => {
	it('returns 1 when ?page= is absent', () => {
		expect(parsePageParam(params(''))).toBe(1);
	});

	it('returns the parsed integer for valid ?page=', () => {
		expect(parsePageParam(params('page=2'))).toBe(2);
		expect(parsePageParam(params('page=42'))).toBe(42);
	});

	it('falls back to 1 for non-numeric or non-positive values', () => {
		expect(parsePageParam(params('page=foo'))).toBe(1);
		expect(parsePageParam(params('page=0'))).toBe(1);
		expect(parsePageParam(params('page=-3'))).toBe(1);
	});
});

describe('decideEpisodeFetchAction', () => {
	it("returns 'fetch-initial' on the first load (episodes null)", () => {
		const got = decideEpisodeFetchAction({
			episodes: null,
			episodesPage: 1,
			episodesLoading: false,
			targetPage: 1
		});
		expect(got).toBe('fetch-initial');
	});

	it("returns 'fetch-initial' even when targetPage is past 1", () => {
		// e.g. user deep-linked to ?page=2 — we still need a fresh
		// initial fetch, just with a different page number.
		const got = decideEpisodeFetchAction({
			episodes: null,
			episodesPage: 1,
			episodesLoading: false,
			targetPage: 2
		});
		expect(got).toBe('fetch-initial');
	});

	it("returns 'noop' when the target matches the current page", () => {
		const got = decideEpisodeFetchAction({
			episodes: [],
			episodesPage: 2,
			episodesLoading: false,
			targetPage: 2
		});
		expect(got).toBe('noop');
	});

	it("returns 'noop' while a fetch is already in flight", () => {
		// Defends against the effect double-firing on its own state
		// writes (episodesPage transitioning, etc.).
		const got = decideEpisodeFetchAction({
			episodes: [],
			episodesPage: 1,
			episodesLoading: true,
			targetPage: 2
		});
		expect(got).toBe('noop');
	});

	it("returns 'fetch' when the URL target diverges from the current page", () => {
		const got = decideEpisodeFetchAction({
			episodes: [],
			episodesPage: 1,
			episodesLoading: false,
			targetPage: 2
		});
		expect(got).toBe('fetch');
	});
});

// — Properties ──────────────────────────────────────────────────────
//
// Both URL parsers must coerce *any* user-controlled query-string
// value into the type the downstream effect expects, never throw,
// and never return out-of-range numbers. fast-check exercises the
// space of inputs (numbers, garbage, exotic unicode) and shrinks any
// failure to a minimal counter-example.

describe('parsePageParam (properties)', () => {
	it('returns >= 1 for any input', () => {
		fc.assert(
			fc.property(fc.string({ maxLength: 50 }), (raw) => {
				const sp = new URLSearchParams();
				sp.set('page', raw);
				return parsePageParam(sp) >= 1;
			})
		);
	});

	it('returns the integer value for any positive integer string', () => {
		fc.assert(
			fc.property(fc.integer({ min: 1, max: 1_000_000 }), (n) => {
				const sp = new URLSearchParams();
				sp.set('page', String(n));
				return parsePageParam(sp) === n;
			})
		);
	});

	it('returns 1 for any non-positive integer or non-numeric input', () => {
		fc.assert(
			fc.property(
				fc.oneof(
					fc.integer({ max: 0 }).map(String),
					// Non-numeric strings: anything that fails parseInt to a
					// finite positive number. Bias toward letter-only.
					fc.string({ minLength: 1, maxLength: 10 }).filter((s) => {
						const n = parseInt(s, 10);
						return !(Number.isFinite(n) && n > 0);
					})
				),
				(raw) => {
					const sp = new URLSearchParams();
					sp.set('page', raw);
					return parsePageParam(sp) === 1;
				}
			)
		);
	});
});

describe('decideEpisodeFetchAction (properties)', () => {
	it("returns 'fetch-initial' iff episodes is null", () => {
		fc.assert(
			fc.property(
				fc.boolean(),
				fc.integer({ min: 1, max: 10 }),
				fc.boolean(),
				fc.integer({ min: 1, max: 10 }),
				(episodesNull, episodesPage, episodesLoading, targetPage) => {
					const action = decideEpisodeFetchAction({
						episodes: episodesNull ? null : [],
						episodesPage,
						episodesLoading,
						targetPage
					});
					return episodesNull ? action === 'fetch-initial' : action !== 'fetch-initial';
				}
			)
		);
	});

	it("never returns 'fetch' while a fetch is already in flight", () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 1, max: 10 }),
				fc.integer({ min: 1, max: 10 }),
				(episodesPage, targetPage) => {
					const action = decideEpisodeFetchAction({
						episodes: [],
						episodesPage,
						episodesLoading: true,
						targetPage
					});
					return action === 'noop';
				}
			)
		);
	});
});
