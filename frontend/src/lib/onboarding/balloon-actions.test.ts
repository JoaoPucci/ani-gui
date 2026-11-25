import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import type { OnboardingTip } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import { balloonActions, runAction } from './balloon-actions';
import { vi } from 'vitest';
import { MAX_STEPS, startSequence, stepSequence } from './sequence';

const tip = (over: Partial<OnboardingTip> = {}): OnboardingTip => ({
	id: 't',
	kind: 'single',
	surface: 'show',
	moment: 'arrive',
	since: '0.19.0',
	places: ['a'],
	how_to: null,
	pending: true,
	...over
});
const ids = (t: OnboardingTip, total = 1, index = 0) => {
	let s = startSequence(total);
	for (let i = 0; i < index; i++) s = stepSequence(s, 'next');
	return balloonActions(t, s).map((a) => a.id);
};

describe('balloonActions', () => {
	it('a single tip has Got it', () => {
		expect(ids(tip())).toEqual(['got-it']);
	});

	it('an entry tip with a walkthrough has Show me how, then Got it', () => {
		const entry = tip({
			kind: 'entry',
			how_to: { id: 'g', surface: 'franchise', places: ['x'] }
		});
		expect(ids(entry)).toEqual(['show-how', 'got-it']);
		expect(balloonActions(entry, startSequence(1))[0].label).toBe(m.onboarding_show_how());
		expect(ids(tip({ kind: 'entry' }))).toEqual(['got-it']);
	});

	it('a sequence has Skip and Next first, Back from the second step, and Done last', () => {
		const seq = tip({ kind: 'sequence', places: ['a', 'b', 'c'] });
		expect(ids(seq, 3, 0)).toEqual(['skip', 'next']);
		expect(ids(seq, 3, 1)).toEqual(['skip', 'back', 'next']);
		expect(ids(seq, 3, 2)).toEqual(['back', 'next']);
		const last = balloonActions(seq, stepSequence(stepSequence(startSequence(3), 'next'), 'next'));
		expect(last.at(-1)).toEqual({ id: 'next', label: m.onboarding_done(), kind: 'primary' });
		expect(ids(tip({ kind: 'sequence' }), 1)).toEqual(['got-it']);
	});

	it('always ends with exactly one primary action (property)', () => {
		fc.assert(
			fc.property(
				fc.constantFrom<OnboardingTip['kind']>('single', 'sequence', 'entry'),
				fc.integer({ min: 1, max: MAX_STEPS }),
				fc.nat(),
				(kind, total, at) => {
					const t = tip({
						kind,
						how_to: kind === 'entry' ? { id: 'g', surface: 'franchise', places: ['x'] } : null
					});
					let s = startSequence(total);
					for (let i = 0; i < at % total; i++) s = stepSequence(s, 'next');
					const acts = balloonActions(t, s);
					return (
						acts.filter((a) => a.kind === 'primary').length === 1 && acts.at(-1)!.kind === 'primary'
					);
				}
			)
		);
	});
});

describe('runAction', () => {
	const session = (how_to: OnboardingTip['how_to'] = null) => {
		const calls: string[] = [];
		const place = { click: () => calls.push('click') } as unknown as HTMLElement;
		return {
			calls,
			s: {
				tip: tip({ kind: 'entry', how_to }),
				place,
				close: (route: string) => calls.push(`close:${route}`),
				step: (to: string) => calls.push(`step:${to}`)
			}
		};
	};

	it('routes Got it, Skip, Next and Back', () => {
		const { calls, s } = session();
		const store = { showGuide: vi.fn() };
		for (const id of ['got-it', 'skip', 'next', 'back'] as const) runAction(id, s, store);
		expect(calls).toEqual(['close:got-it', 'close:skip', 'step:next', 'step:back']);
		expect(store.showGuide).not.toHaveBeenCalled();
	});

	it('Show me how closes the entry, asks for the walkthrough, then presses the entry point', () => {
		const howTo = { id: 'g', surface: 'franchise' as const, places: ['x'] };
		const { calls, s } = session(howTo);
		const store = { showGuide: vi.fn(() => calls.push('guide')) };
		runAction('show-how', s, store);
		expect(calls).toEqual(['close:show-how', 'guide', 'click']);
		expect(store.showGuide).toHaveBeenCalledWith(howTo);
	});
});
