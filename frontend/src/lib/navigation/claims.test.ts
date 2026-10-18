import { describe, expect, it } from 'vitest';
import { createNavigationClaims } from './claims';

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
