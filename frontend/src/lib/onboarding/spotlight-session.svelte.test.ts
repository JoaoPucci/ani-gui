// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { OnboardingTip } from '$lib/api';
import { isModalOpen, openModal } from '$lib/components/modal-layer';
import { PRESS_GRACE_MS } from './gate';
import { SpotlightSession } from './spotlight-session.svelte';

const TIP: OnboardingTip = {
	id: 'watch-order',
	kind: 'single',
	surface: 'show',
	moment: 'arrive',
	since: '0.19.0',
	places: ['watch-order-strip'],
	how_to: null,
	pending: true
};

let clock: number;
let seen: string[];
let closed: boolean[];
let sessions: SpotlightSession[];
let resized: ResizeObserverCallback[];

class FakeRO {
	constructor(cb: ResizeObserverCallback) {
		resized.push(cb);
	}
	observe() {}
	disconnect() {}
}
const inert = (id: string) => document.getElementById(id)!.closest('[inert]') !== null;

function start() {
	const place = document.getElementById('place')!;
	const s = new SpotlightSession(TIP, place, {
		doc: document,
		now: () => clock,
		markSeen: (id) => seen.push(id),
		onClosed: (wasSeen) => closed.push(wasSeen)
	});
	const balloon = document.getElementById('balloon')!;
	s.hold(balloon, document.getElementById('scrim'));
	sessions.push(s);
	return s;
}

beforeEach(() => {
	document.body.innerHTML = `
		<header class="topbar"><button class="winctl-btn" id="min">_</button><span id="title">t</span></header>
		<main><button id="opener">open</button>
			<div id="place" data-onboard="watch-order-strip"><a id="link" href="#x">next</a></div>
			<p id="other">other</p></main>
		<div id="scrim"></div>
		<div id="balloon" tabindex="-1"><button id="got" data-autofocus>Got it</button></div>`;
	document.getElementById('opener')!.focus();
	clock = 1000;
	seen = [];
	closed = [];
	sessions = [];
	resized = [];
	vi.stubGlobal('ResizeObserver', FakeRO);
});

afterEach(() => {
	for (const s of sessions) s.close('yield');
	document.body.innerHTML = '';
	vi.unstubAllGlobals();
});

describe('SpotlightSession', () => {
	it('holds the page as a modal, focused on Got it, with the place and window buttons in reach', () => {
		start();
		expect(isModalOpen()).toBe(true);
		expect(document.activeElement?.id).toBe('got');
		expect(inert('other')).toBe(true);
		expect(inert('title')).toBe(true);
		expect(inert('link')).toBe(false);
		expect(inert('min')).toBe(false);
		// A press outside lands on the dim's catcher, not on an inert page.
		expect(inert('scrim')).toBe(false);
	});

	it('Got it closes it seen and moves focus to the place', () => {
		const s = start();
		s.close('got-it');
		expect(seen).toEqual(['watch-order']);
		expect(closed).toEqual([true]);
		expect(isModalOpen()).toBe(false);
		expect(inert('other')).toBe(false);
		expect(document.activeElement?.id).toBe('link');
	});

	it('Escape closes it seen and gives focus back', () => {
		start();
		document.activeElement!.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
		);
		expect(seen).toEqual(['watch-order']);
		expect(document.activeElement?.id).toBe('opener');
	});

	it('a press outside in the first moments does nothing, and after them closes it seen', () => {
		const s = start();
		clock += PRESS_GRACE_MS - 1;
		s.close('outside');
		expect(closed).toEqual([]);
		clock += 1;
		s.close('outside');
		expect(seen).toEqual(['watch-order']);
	});

	it('a press on the place closes it seen; a press elsewhere is not the place', () => {
		const s = start();
		clock += PRESS_GRACE_MS;
		s.pressed(document.getElementById('other'));
		expect(closed).toEqual([]);
		s.pressed(document.getElementById('link'));
		expect(seen).toEqual(['watch-order']);
	});

	it('yields, still pending, to a dialog that opens over it', () => {
		const s = start();
		const dialog = document.createElement('div');
		dialog.innerHTML = '<button id="d">ok</button>';
		document.body.appendChild(dialog);
		const h = openModal(dialog);
		s.modalsChanged();
		expect(closed).toEqual([false]);
		expect(seen).toEqual([]);
		// The dialog keeps the page and focus.
		expect(isModalOpen()).toBe(true);
		expect(document.activeElement?.id).toBe('d');
		h.close();
	});

	it('yields when its place leaves the page, with no scroll to tell it', async () => {
		start();
		document.getElementById('place')!.remove();
		await new Promise((r) => setTimeout(r, 0));
		expect(closed).toEqual([false]);
		expect(isModalOpen()).toBe(false);
	});

	it('reads its place again when the place changes size, with no scroll', async () => {
		const s = start();
		const place = document.getElementById('place')!;
		place.getBoundingClientRect = () => ({ left: 10, top: 20, width: 300, height: 90 }) as DOMRect;
		resized.forEach((cb) => cb([], {} as ResizeObserver));
		expect(s.rect).toEqual({ x: 10, y: 20, w: 300, h: 90 });
	});

	it('with nothing focused when it opened, Escape sends focus to the place', () => {
		(document.activeElement as HTMLElement | null)?.blur();
		start();
		document.activeElement!.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
		);
		expect(document.activeElement?.id).toBe('link');
	});

	it('a yield leaves focus where the page put it, never on the place', () => {
		(document.activeElement as HTMLElement | null)?.blur();
		const s = start();
		s.close('yield');
		expect(document.activeElement?.id).not.toBe('link');
	});

	it('counts the first moments from when the balloon holds the page, not from when the tip was chosen', () => {
		const place = document.getElementById('place')!;
		const s = new SpotlightSession(TIP, place, {
			doc: document,
			now: () => clock,
			markSeen: (id) => seen.push(id),
			onClosed: (wasSeen) => closed.push(wasSeen)
		});
		sessions.push(s);
		clock += 1000;
		s.hold(document.getElementById('balloon')!);
		s.close('outside');
		expect(closed).toEqual([]);
	});

	it('closes once', () => {
		const s = start();
		s.close('got-it');
		s.close('escape');
		expect(closed).toEqual([true]);
	});

	it('reads the show’s colour at the place', () => {
		document.getElementById('place')!.style.setProperty('--accent', '#e2a04b');
		const s = start();
		expect(s.accent).toBe('#e2a04b');
	});

	it('a yield before the balloon is placed closes nothing it never opened', () => {
		const place = document.getElementById('place')!;
		const onClosed = vi.fn();
		const s = new SpotlightSession(TIP, place, {
			doc: document,
			now: () => clock,
			markSeen: () => {},
			onClosed
		});
		s.close('yield');
		expect(onClosed).toHaveBeenCalledWith(false);
		expect(isModalOpen()).toBe(false);
	});
});
