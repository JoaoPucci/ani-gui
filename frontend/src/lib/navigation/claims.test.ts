import { describe, expect, it } from 'vitest';
import { claimOrHold, createNavigationClaims } from './claims';

describe('createNavigationClaims', () => {
	it('holds a claim while the viewer stays', () => {
		const claims = createNavigationClaims();
		const here = claims.claim();
		expect(here()).toBe(true);
	});

	it('lapses a claim once a navigation starts', () => {
		const claims = createNavigationClaims();
		const here = claims.claim();
		claims.started();
		expect(here()).toBe(false);
	});

	it('lapses a claim once the page is left', () => {
		const claims = createNavigationClaims();
		const here = claims.claim();
		claims.left();
		expect(here()).toBe(false);
	});

	it('holds a claim taken after a navigation started', () => {
		const claims = createNavigationClaims();
		claims.started();
		const here = claims.claim();
		expect(here()).toBe(true);
	});

	it('holds no claim taken once the page is left', () => {
		const claims = createNavigationClaims();
		claims.left();
		expect(claims.claim()()).toBe(false);
	});

	it('advances its epoch with each navigation that starts', () => {
		const claims = createNavigationClaims();
		const before = claims.epoch;
		claims.started();
		claims.started();
		expect(claims.epoch).toBe(before + 2);
	});
});

describe('claimOrHold', () => {
	it('takes the claim it is given', () => {
		const claims = createNavigationClaims();
		const here = claimOrHold(() => claims.claim());
		claims.started();
		expect(here()).toBe(false);
	});

	it('holds when there is no claim to take', () => {
		expect(claimOrHold(undefined)()).toBe(true);
	});
});

// A navigation that changes only the query — the detail page's
// episode pager, the player's own episode switch, a link or goto to
// the URL on screen — keeps the page on screen as it is. An action
// pending through one is still the viewer's.
describe('createNavigationClaims, by where a navigation goes', () => {
	const at = (path: string) => ({ url: new URL(path, 'http://127.0.0.1:31337') });

	const cases: [
		string,
		Parameters<ReturnType<typeof createNavigationClaims>['started']>[0],
		boolean
	][] = [
		['a link to another page', { from: at('/anime/42'), to: at('/search?q=x') }, false],
		['another show', { from: at('/anime/42'), to: at('/anime/77') }, false],
		['back to another page', { from: at('/play/42?episode=2'), to: at('/anime/42') }, false],
		['the episode pager', { from: at('/anime/42'), to: at('/anime/42?page=2') }, true],
		['an episode switch', { from: at('/play/42?episode=1'), to: at('/play/42?episode=2') }, true],
		['the URL already on screen', { from: at('/'), to: at('/') }, true],
		['leaving the app', { from: at('/anime/42'), to: null }, false],
		[
			'another origin',
			{ from: at('/anime/42'), to: { url: new URL('https://example.com/anime/42') } },
			false
		],
		['a navigation reported without its ends', {}, false]
	];

	for (const [what, nav, holds] of cases) {
		it(`${holds ? 'holds' : 'lapses'} a claim for ${what}`, () => {
			const claims = createNavigationClaims();
			const here = claims.claim();
			claims.started(nav);
			expect(here()).toBe(holds);
		});
	}
});
