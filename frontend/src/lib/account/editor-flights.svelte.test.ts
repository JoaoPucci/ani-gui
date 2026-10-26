import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { editorFlights } from './editor-flights.svelte';
import { accountStore } from './store.svelte';
import { withoutFlight, withFlight, NO_FLIGHTS, type Flight } from './editor-flight';

const account = (user_id: string) => ({
	access_token: 't',
	refresh_token: null,
	expires_at_epoch_s: 4_000_000_000,
	user_id,
	username: 'u',
	avatar_url: null
});

beforeEach(() => {
	accountStore.setConnected('anilist', account('u1'));
});

afterEach(() => {
	accountStore.setDisconnected('anilist');
	editorFlights.flights = NO_FLIGHTS;
	editorFlights.setPending('A', null);
});

describe('editorFlights', () => {
	it('locks a show while its tracker is still the user its write listed', () => {
		const flight: Flight = { kind: 'saving', listed: { anilist: { user: 'u1' } } };
		editorFlights.begin('A', flight);
		expect(editorFlights.locking('A')).toBe(flight);
		expect(editorFlights.locking('B')).toBeNull();
	});

	it('frees the show once the tracker is no longer that user', () => {
		editorFlights.begin('A', { kind: 'saving', listed: { anilist: { user: 'u1' } } });
		accountStore.setConnected('anilist', account('u2'));
		expect(editorFlights.locking('A')).toBeNull();
		accountStore.setDisconnected('anilist');
		expect(editorFlights.locking('A')).toBeNull();
	});

	it('ends the flight that settled and not a newer one for the same show', () => {
		const older: Flight = { kind: 'saving' };
		const newer: Flight = { kind: 'saving' };
		editorFlights.begin('A', older);
		editorFlights.begin('A', newer);
		editorFlights.end('A', older);
		expect(editorFlights.locking('A')).toBe(newer);
		editorFlights.end('A', newer);
		expect(editorFlights.locking('A')).toBeNull();
	});

	it('keeps a pending retry per show', () => {
		const pending = { kitsuId: 'A', intendedStatus: 'paused' as const, intendedProgress: 5 };
		editorFlights.setPending('A', pending);
		expect(editorFlights.pendingFor('A')).toEqual(pending);
		expect(editorFlights.pendingFor('B')).toBeNull();
		editorFlights.setPending('A', null);
		expect(editorFlights.pendingFor('A')).toBeNull();
	});
});

describe('withoutFlight given the flight', () => {
	it('removes only that flight', () => {
		const a: Flight = { kind: 'saving' };
		const flights = withFlight(NO_FLIGHTS, 'A', a);
		expect(withoutFlight(flights, 'A', { kind: 'saving' })).toBe(flights);
		expect(withoutFlight(flights, 'A', a).has('A')).toBe(false);
	});
});

describe('a pending retry and the account its save was made under', () => {
	const pending = { kitsuId: 'A', intendedStatus: 'paused' as const, intendedProgress: 5 };

	it('is kept while the tracker is still that user, a reconnect included', () => {
		editorFlights.setPending('A', pending, { anilist: { user: 'u1' } });
		expect(editorFlights.pendingFor('A')).toEqual(pending);
		accountStore.setConnected('anilist', account('u1'));
		expect(editorFlights.pendingFor('A')).toEqual(pending);
	});

	it('is absent once the tracker is disconnected or is another user', () => {
		editorFlights.setPending('A', pending, { anilist: { user: 'u1' } });
		accountStore.setConnected('anilist', account('u2'));
		expect(editorFlights.pendingFor('A')).toBeNull();
		accountStore.setDisconnected('anilist');
		expect(editorFlights.pendingFor('A')).toBeNull();
	});
});

describe('flightOf', () => {
	it('is the flight still out, even once an account change stops it locking', () => {
		const flight: Flight = { kind: 'saving', listed: { anilist: { user: 'u1' } } };
		editorFlights.begin('A', flight);
		accountStore.setConnected('anilist', account('u2'));
		expect(editorFlights.locking('A')).toBeNull();
		expect(editorFlights.flightOf('A')).toBe(flight);
		editorFlights.end('A', flight);
		expect(editorFlights.flightOf('A')).toBeNull();
	});
});

describe('a pending retry whose account comes back', () => {
	it('is hidden while the account does not hold, and offered again once it does', () => {
		const pending = { kitsuId: 'A', intendedStatus: 'paused' as const, intendedProgress: 5 };
		editorFlights.setPending('A', pending, { anilist: { user: 'u1' } });
		accountStore.setConnected('anilist', account('u2'));
		expect(editorFlights.pendingFor('A')).toBeNull();
		accountStore.setConnected('anilist', account('u1'));
		expect(editorFlights.pendingFor('A')).toEqual(pending);
	});
});
