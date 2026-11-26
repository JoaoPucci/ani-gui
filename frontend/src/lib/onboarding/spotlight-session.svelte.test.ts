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
	const s = new SpotlightSession(TIP, [place], {
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
		const s = new SpotlightSession(TIP, [place], {
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
		const s = new SpotlightSession(TIP, [place], {
			doc: document,
			now: () => clock,
			markSeen: () => {},
			onClosed
		});
		s.close('yield');
		expect(onClosed).toHaveBeenCalledWith(false);
		expect(isModalOpen()).toBe(false);
	});

	it('scrolls a place with no room beside it into room once, then holds the page', () => {
		const place = document.getElementById('place')!;
		const s = new SpotlightSession(TIP, [place], {
			doc: document,
			now: () => clock,
			markSeen: () => {},
			onClosed: () => {}
		});
		sessions.push(s);
		const scrolled: number[] = [];
		const balloon = document.getElementById('balloon')!;
		const none = {
			mode: 'anchored',
			side: 'below',
			x: 0,
			y: 0,
			width: 300,
			fits: false,
			arrow: 0
		} as const;
		s.placed(none, { h: 180 }, balloon, null, (_el, margin) => scrolled.push(margin));
		expect(scrolled).toEqual([16 + 180 + 12 + 8]);
		expect(isModalOpen()).toBe(true);
		s.placed(none, { h: 180 }, balloon, null, (_el, margin) => scrolled.push(margin));
		expect(scrolled).toHaveLength(1);
	});

	it('holds the page at once when the balloon fits beside its place', () => {
		const place = document.getElementById('place')!;
		const s = new SpotlightSession(TIP, [place], {
			doc: document,
			now: () => clock,
			markSeen: () => {},
			onClosed: () => {}
		});
		sessions.push(s);
		const scroll = vi.fn();
		const fits = {
			mode: 'anchored',
			side: 'below',
			x: 0,
			y: 0,
			width: 300,
			fits: true,
			arrow: 0
		} as const;
		s.placed(fits, { h: 180 }, document.getElementById('balloon')!, null, scroll);
		expect(scroll).not.toHaveBeenCalled();
		expect(document.activeElement?.id).toBe('got');
	});

	it('a closed session neither scrolls nor holds', () => {
		const s = start();
		s.close('got-it');
		const scroll = vi.fn();
		const none = {
			mode: 'anchored',
			side: 'below',
			x: 0,
			y: 0,
			width: 300,
			fits: false,
			arrow: 0
		} as const;
		s.placed(none, { h: 180 }, document.getElementById('balloon')!, null, scroll);
		expect(scroll).not.toHaveBeenCalled();
		expect(isModalOpen()).toBe(false);
	});

	it('a sequence moves its spotlight and what a press must reach from step to step, and Done closes it seen', () => {
		document
			.querySelector('main')!
			.insertAdjacentHTML('beforeend', '<div id="second"><button id="second-btn">b</button></div>');
		const first = document.getElementById('place')!;
		const second = document.getElementById('second')!;
		const s = new SpotlightSession(
			{ ...TIP, kind: 'sequence', places: ['a', 'b'] },
			[first, second],
			{
				doc: document,
				now: () => clock,
				markSeen: (id) => seen.push(id),
				onClosed: (wasSeen) => closed.push(wasSeen)
			}
		);
		sessions.push(s);
		s.hold(document.getElementById('balloon')!);
		expect(inert('second-btn')).toBe(true);
		s.step('next');
		expect(s.place).toBe(second);
		expect(s.steps.travelling).toBe(true);
		expect(inert('second-btn')).toBe(false);
		expect(inert('link')).toBe(true);
		s.step('back');
		expect(s.place).toBe(first);
		s.step('next');
		s.step('next');
		expect(seen).toEqual(['watch-order']);
		expect(closed).toEqual([true]);
		expect(document.activeElement?.id).toBe('second-btn');
	});

	it('Skip closes a sequence seen, and gives focus back', () => {
		const s = start();
		s.close('skip');
		expect(seen).toEqual(['watch-order']);
		expect(document.activeElement?.id).toBe('opener');
	});

	it('Show me how closes an entry seen and moves focus nowhere itself', () => {
		const s = start();
		s.close('show-how');
		expect(seen).toEqual(['watch-order']);
		// The modal layer gives focus back; the tip does not send it to the
		// place, since the press that follows opens the feature's page.
		expect(document.activeElement?.id).toBe('opener');
	});

	it('a step whose place is off screen is scrolled into room as the step comes up', () => {
		document.querySelector('main')!.insertAdjacentHTML('beforeend', '<div id="far"></div>');
		const first = document.getElementById('place')!;
		const far = document.getElementById('far')!;
		first.getBoundingClientRect = () => ({ left: 0, top: 200, width: 300, height: 50 }) as DOMRect;
		far.getBoundingClientRect = () => ({ left: 0, top: 5000, width: 300, height: 50 }) as DOMRect;
		const s = new SpotlightSession({ ...TIP, kind: 'sequence', places: ['a', 'b'] }, [first, far], {
			doc: document,
			now: () => clock,
			markSeen: () => {},
			onClosed: () => {}
		});
		sessions.push(s);
		const scrolled: HTMLElement[] = [];
		const fits = {
			mode: 'anchored',
			side: 'below',
			x: 0,
			y: 0,
			width: 300,
			fits: true,
			arrow: 0
		} as const;
		s.placed(fits, { h: 180 }, document.getElementById('balloon')!, null, (el) =>
			scrolled.push(el)
		);
		expect(scrolled).toEqual([]);
		s.step('next');
		expect(scrolled).toEqual([far]);
		s.step('back');
		// The first place is on screen: no scroll back to it.
		expect(scrolled).toEqual([far]);
	});
});
