import { afterEach, describe, expect, it } from 'vitest';
import { qualityPin } from './quality-pin';
import { playerSession } from './quality-session';

afterEach(() => {
	playerSession.ends();
	qualityPin.clear();
});

describe('playerSession — how long a pick in the player lasts', () => {
	it('keeps a pick across the episodes of the show it was made on', () => {
		playerSession.shows('42');
		qualityPin.set(720);
		playerSession.shows('42');
		expect(qualityPin.get()).toBe(720);
	});

	it('lets a pick go when the player is left', () => {
		playerSession.shows('42');
		qualityPin.set(720);
		playerSession.ends();
		expect(qualityPin.choice()).toBeNull();
	});

	it('lets a pick go when the player turns to another show', () => {
		playerSession.shows('42');
		qualityPin.set({ rank: 1 });
		playerSession.shows('43');
		expect(qualityPin.choice()).toBeNull();
	});

	it('a session that starts after one was left starts on Auto', () => {
		playerSession.shows('42');
		qualityPin.set(480);
		playerSession.ends();
		playerSession.shows('42');
		expect(qualityPin.choice()).toBeNull();
	});
});
