import { describe, expect, it } from 'vitest';

import {
	heldPending,
	listingsCompatible,
	NO_SHOW_PENDINGS,
	settlePending
} from './editor-pendings';

const retry = { kitsuId: 'A', intendedStatus: 'dropped' as const, intendedProgress: 5 };
const AM = { anilist: { user: 'a' }, mal: { user: 'm' } };

describe('listingsCompatible', () => {
	it('takes in a tracker connected or disconnected since, with no account switched', () => {
		expect(listingsCompatible(AM, { ...AM, inhouse: { user: 'k' } })).toBe(true);
		expect(listingsCompatible(AM, { anilist: { user: 'a' } })).toBe(true);
	});

	it('leaves out a listing where a shared tracker is another user', () => {
		expect(listingsCompatible(AM, { anilist: { user: 'a' }, mal: { user: 'm2' } })).toBe(false);
	});

	it('leaves out a listing with no shared tracker of the same user', () => {
		expect(listingsCompatible(AM, { inhouse: { user: 'k' } })).toBe(false);
		expect(listingsCompatible({ anilist: { user: null } }, { anilist: { user: 'a' } })).toBe(false);
	});

	it('takes in the very same listing, and a write with no listing only its own kind', () => {
		expect(listingsCompatible({ anilist: { user: null } }, { anilist: { user: null } })).toBe(true);
		expect(listingsCompatible(undefined, undefined)).toBe(true);
		expect(listingsCompatible(undefined, AM)).toBe(false);
	});
});

describe('settlePending across tracker sets', () => {
	it('clears a retry made before a tracker was connected', () => {
		const left = settlePending(NO_SHOW_PENDINGS, 'A', AM, () => retry);
		const cleared = settlePending(left, 'A', { ...AM, inhouse: { user: 'k' } }, () => null);
		expect(heldPending(cleared, 'A', () => true)).toBeNull();
	});

	it('clears a retry made before a tracker was disconnected', () => {
		const left = settlePending(NO_SHOW_PENDINGS, 'A', AM, () => retry);
		const cleared = settlePending(left, 'A', { anilist: { user: 'a' } }, () => null);
		expect(heldPending(cleared, 'A', () => true)).toBeNull();
	});

	it("leaves another account's retry", () => {
		const left = settlePending(NO_SHOW_PENDINGS, 'A', AM, () => retry);
		const kept = settlePending(
			left,
			'A',
			{ anilist: { user: 'a2' }, mal: { user: 'm2' } },
			() => null
		);
		expect(heldPending(kept, 'A', () => true)).toEqual(retry);
	});
});
