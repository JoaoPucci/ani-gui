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
import type { OnboardingTip } from '$lib/api';
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
