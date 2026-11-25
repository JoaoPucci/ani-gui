/**
 * The balloon's actions for the tip on screen, in footer order: a quiet
 * Skip at the start, then Back, then the one primary action. Rendering:
 * which buttons the balloon draws.
 *
 * - A single tip: Got it.
 * - An entry tip with a walkthrough: Show me how, then Got it.
 * - A sequence: Skip (not on its last step), Back (from its second),
 *   and Next, or Done on its last step. A one-step sequence is Got it.
 */
import type { OnboardingHowTo, OnboardingTip } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import { actionsFor, type SeqState } from './sequence';

export type ActionId = 'got-it' | 'show-how' | 'skip' | 'back' | 'next';

export interface BalloonAction {
	id: ActionId;
	label: string;
	kind: 'primary' | 'secondary' | 'quiet';
}

export function balloonActions(tip: OnboardingTip, seq: SeqState): BalloonAction[] {
	const gotIt: BalloonAction = { id: 'got-it', label: m.onboarding_got_it(), kind: 'primary' };
	if (tip.kind === 'entry' && tip.how_to) {
		return [{ id: 'show-how', label: m.onboarding_show_how(), kind: 'secondary' }, gotIt];
	}
	if (tip.kind !== 'sequence' || seq.total === 1) return [gotIt];
	const a = actionsFor(seq);
	return [
		...(a.skip ? [{ id: 'skip', label: m.onboarding_skip(), kind: 'quiet' } as const] : []),
		...(a.back ? [{ id: 'back', label: m.onboarding_back(), kind: 'secondary' } as const] : []),
		{ id: 'next', label: a.last ? m.onboarding_done() : m.onboarding_next(), kind: 'primary' }
	];
}

/** What `runAction` needs of the tip on screen. */
export interface ActionTarget {
	tip: OnboardingTip;
	place: HTMLElement;
	close: (route: 'got-it' | 'skip' | 'show-how') => void;
	step: (to: 'next' | 'back') => void;
}

/** Do what the balloon's action `id` says. Show me how closes the
 *  entry, asks for the feature's walkthrough, then presses the entry
 *  point, which opens the feature's page where the walkthrough runs. */
export function runAction(
	id: ActionId,
	s: ActionTarget,
	store: { showGuide: (howTo: OnboardingHowTo) => void }
): void {
	if (id === 'got-it' || id === 'skip') return s.close(id);
	if (id === 'next' || id === 'back') return s.step(id);
	const howTo = s.tip.how_to;
	const entry = s.place;
	s.close('show-how');
	if (howTo) store.showGuide(howTo);
	entry.click();
}
