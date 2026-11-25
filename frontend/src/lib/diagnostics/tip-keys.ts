/**
 * What the Debug page shows of the feature tips: every tip the backend
 * declares, by key, with its state for this install
 * (`GET /api/onboarding/debug`, backend/src/onboarding), and the words
 * for each. Nothing here decides anything about a tip's window; it
 * names what the backend answered.
 */

import type { OnboardingDebugTip, OnboardingDebugView, OnboardingTip } from '$lib/api';
import { m } from '$lib/paraglide/messages';

/** Where the page's read of the tips stands. */
export type TipKeysState =
	| { kind: 'loading' }
	| { kind: 'read'; view: OnboardingDebugView }
	| { kind: 'failed' };

/** Read the tips; failed when the backend cannot answer. */
export async function readTipKeys(read: () => Promise<OnboardingDebugView>): Promise<TipKeysState> {
	try {
		return { kind: 'read', view: await read() };
	} catch {
		return { kind: 'failed' };
	}
}

/** The tip's window for this install, in words, as the backend
 *  answered it: on demand, its flag off, or the releases it opens and
 *  ends at and whether the running release is inside them. */
export function windowLabel(tip: OnboardingDebugTip): string {
	if (tip.on_demand) return m.diagnostics_tips_on_demand();
	if (tip.flag !== null && !tip.flag_on) return m.diagnostics_tips_flag_off({ flag: tip.flag });
	if (tip.opens === null || tip.ends === null) return m.diagnostics_tips_not_offered();
	const span = { opens: tip.opens, ends: tip.ends };
	return tip.offered ? m.diagnostics_tips_offered(span) : m.diagnostics_tips_outside(span);
}

/** How this install's record began, in words. */
export function installLabel(view: OnboardingDebugView): string {
	return view.installed_in === null
		? m.diagnostics_tips_updated()
		: m.diagnostics_tips_installed_in({ version: view.installed_in });
}

/** The tip as the host runs it on demand. */
export function asRunnable(tip: OnboardingDebugTip): OnboardingTip {
	return {
		id: tip.id,
		kind: tip.kind,
		surface: tip.surface,
		moment: tip.moment,
		since: tip.since,
		places: tip.places,
		how_to: tip.how_to,
		pending: true
	};
}
