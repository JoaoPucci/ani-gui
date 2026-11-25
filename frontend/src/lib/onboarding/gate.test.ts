import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { OnboardingTip } from '$lib/api';
import {
	armedNow,
	arrivesOn,
	arrivingNow,
	onScreen,
	surfaceOf,
	watchedHere,
	type ScreenNow
} from './gate';

const tip = (over: Partial<OnboardingTip> = {}): OnboardingTip => ({
	id: 'watch-order',
	kind: 'single',
	surface: 'show',
	moment: 'arrive',
	since: '0.19.0',
	places: ['watch-order-strip'],
	how_to: null,
	pending: true,
	...over
});

const now = (over: Partial<ScreenNow> = {}): ScreenNow => ({
	surface: 'show',
	fullscreen: false,
	modalOpen: false,
	closedThisVisit: false,
	settled: () => true,
	...over
});

describe('surfaceOf', () => {
	it('names the pages tips live on', () => {
		expect(surfaceOf('/')).toBe('home');
		expect(surfaceOf('/anime/[id]')).toBe('show');
		expect(surfaceOf('/anime/[id]/franchise')).toBe('franchise');
		expect(surfaceOf('/play/[id]')).toBe('play');
		expect(surfaceOf('/settings')).toBe('settings');
		expect(surfaceOf('/about')).toBe('other');
		expect(surfaceOf(null)).toBe('other');
	});
});

describe('onScreen', () => {
	const view = { h: 800, top: 64 };
	it('counts a place half on screen', () => {
		expect(onScreen({ top: 701, bottom: 901, width: 10 }, view)).toBe(false);
		expect(onScreen({ top: 600, bottom: 800, width: 10 }, view)).toBe(true);
	});
	it('counts 160px of a tall place', () => {
		expect(onScreen({ top: 640, bottom: 2000, width: 10 }, view)).toBe(true);
		expect(onScreen({ top: 641, bottom: 2000, width: 10 }, view)).toBe(false);
	});
	it('never counts what is under the top bar or has no width', () => {
		expect(onScreen({ top: 0, bottom: 64, width: 10 }, view)).toBe(false);
		expect(onScreen({ top: 100, bottom: 200, width: 0 }, view)).toBe(false);
	});
	it('a place fully between the top bar and the bottom always counts (property)', () => {
		fc.assert(
			fc.property(fc.integer({ min: 64, max: 799 }), fc.integer({ min: 1, max: 736 }), (top, h) => {
				const bottom = Math.min(top + h, 800);
				return bottom <= top || onScreen({ top, bottom, width: 5 }, view);
			})
		);
	});
});

describe('arrivesOn and watchedHere', () => {
	it('a tip arrives on its own page, or any page for the chrome, never in the player', () => {
		expect(arrivesOn(tip(), 'show')).toBe(true);
		expect(arrivesOn(tip(), 'home')).toBe(false);
		expect(arrivesOn(tip({ surface: 'any' }), 'home')).toBe(true);
		expect(arrivesOn(tip({ surface: 'any' }), 'play')).toBe(false);
		expect(arrivesOn(tip({ surface: 'any' }), 'other')).toBe(false);
		expect(arrivesOn(tip({ moment: 'stream-load', surface: 'play' }), 'play')).toBe(false);
	});
	it('watches only pending tips', () => {
		expect(
			watchedHere([tip({ pending: false }), tip({ id: 'b' })], 'show').map((t) => t.id)
		).toEqual(['b']);
	});
});

describe('arrivingNow', () => {
	it('shows the pending tip whose place has settled', () => {
		expect(arrivingNow([tip()], now())?.id).toBe('watch-order');
	});
	it('waits for its place', () => {
		expect(arrivingNow([tip()], now({ settled: () => false }))).toBeNull();
	});
	it('takes the first in order whose place settled', () => {
		const a = tip({ id: 'a', places: ['p-a'] });
		const b = tip({ id: 'b', places: ['p-b'] });
		expect(arrivingNow([a, b], now({ settled: (p) => p === 'p-b' }))?.id).toBe('b');
		expect(arrivingNow([a, b], now())?.id).toBe('a');
	});
	it('never shows in fullscreen, over a dialog, or a second time on one visit', () => {
		for (const over of [{ fullscreen: true }, { modalOpen: true }, { closedThisVisit: true }]) {
			expect(arrivingNow([tip()], now(over))).toBeNull();
		}
	});
	it('never shows a seen tip, or one for another page', () => {
		expect(arrivingNow([tip({ pending: false })], now())).toBeNull();
		expect(arrivingNow([tip()], now({ surface: 'home' }))).toBeNull();
	});
});

describe('armedNow', () => {
	it('shows a tip asked for once its place settles on its page, seen or not, past the visit’s one', () => {
		const t = tip({ pending: false });
		expect(armedNow(t, now())?.id).toBe('watch-order');
		expect(armedNow(t, now({ settled: () => false }))).toBeNull();
		expect(armedNow(t, now({ surface: 'home' }))).toBeNull();
	});
	it('still waits out fullscreen and dialogs, and needs a tip', () => {
		expect(armedNow(tip(), now({ fullscreen: true }))).toBeNull();
		expect(armedNow(tip(), now({ modalOpen: true }))).toBeNull();
		expect(armedNow(null, now())).toBeNull();
		expect(armedNow(tip({ places: [] }), now())).toBeNull();
	});
});
