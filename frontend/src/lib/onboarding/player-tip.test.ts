// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';
import type { OnboardingTip } from '$lib/api';
import { openModal } from '$lib/components/modal-layer';
import { escapeIsTips, focusPlayer, playerTip } from './player-tip';

const tip = (over: Partial<OnboardingTip> = {}): OnboardingTip => ({
	id: 'player-up-next',
	kind: 'single',
	surface: 'play',
	moment: 'stream-load',
	since: '0.19.0',
	places: ['player-frame'],
	how_to: null,
	pending: true,
	...over
});

afterEach(() => {
	document.body.innerHTML = '';
});

describe('playerTip', () => {
	it('shows a tip asked for first, then the first pending one that loads in the player', () => {
		const strip = tip({ id: 'strip', surface: 'show', moment: 'arrive' });
		expect(playerTip([strip, tip()], null)).toEqual({ tip: tip(), asked: false });
		expect(playerTip([tip()], tip({ id: 'asked' }))).toEqual({
			tip: tip({ id: 'asked' }),
			asked: true
		});
		expect(playerTip([tip({ pending: false }), strip], null)).toBeNull();
		expect(playerTip([], strip)).toBeNull();
	});
});

describe('escapeIsTips', () => {
	const esc = (init: KeyboardEventInit = {}) =>
		new KeyboardEvent('keydown', { key: 'Escape', cancelable: true, ...init });

	it('takes an Escape nothing else has', () => {
		expect(escapeIsTips(esc(), false, document)).toBe(true);
		expect(escapeIsTips(new KeyboardEvent('keydown', { key: 'k' }), false, document)).toBe(false);
	});

	it('leaves it to an open menu or popover, a dialog, or whatever took it first', () => {
		expect(escapeIsTips(esc(), true, document)).toBe(false);
		document.body.innerHTML = '<button aria-expanded="true"></button>';
		expect(escapeIsTips(esc(), false, document)).toBe(false);
		document.body.innerHTML = '';
		const taken = esc();
		taken.preventDefault();
		expect(escapeIsTips(taken, false, document)).toBe(false);
		const d = document.createElement('div');
		d.innerHTML = '<button>ok</button>';
		document.body.appendChild(d);
		const h = openModal(d);
		expect(escapeIsTips(esc(), false, document)).toBe(false);
		h.close();
	});
});

describe('focusPlayer', () => {
	it('focuses the error panel’s action, else a control, else the frame', () => {
		document.body.innerHTML = `<section id="f">
			<div class="pc-buttons-row"><button class="pc-btn" disabled>a</button><button class="pc-btn" id="b">b</button></div>
		</section>`;
		const frame = document.getElementById('f')!;
		focusPlayer(frame);
		expect(document.activeElement?.id).toBe('b');
		frame.insertAdjacentHTML('afterbegin', '<button class="player-error-action" id="e">r</button>');
		focusPlayer(frame);
		expect(document.activeElement?.id).toBe('e');
		frame.innerHTML = '<div inert><button class="pc-buttons-row pc-btn">x</button></div>';
		focusPlayer(frame);
		expect(document.activeElement).toBe(frame);
		expect(frame.getAttribute('tabindex')).toBe('-1');
	});
});
