/**
 * Pure helper deciding what the topbar AccountChip should render.
 * Source of truth is `accountStore.byProvider`; the chip itself only
 * does the rendering, so this helper is unit-testable without any
 * Svelte / DOM dependency.
 *
 * Logic:
 *   - Pick the highest-priority provider with a known identity
 *     (`connected`, `expired`, or `error` with a surviving account).
 *   - Priority order: AniList → MAL → InHouse. Same as
 *     `MERGE_ORDER` in `./watch-later`.
 *   - Disconnected / connecting / orphan-error states surface as
 *     `{kind: 'hidden'}` — the side-rail's /account link is the
 *     primary path for users without a session.
 */

import type { Provider, ProviderState } from './types';

export type ChipWarning = 'expired' | 'error';

export type ChipState =
	| { kind: 'hidden' }
	| {
			kind: 'connected';
			provider: Provider;
			username: string;
			avatarUrl: string | null;
			warning: ChipWarning | null;
	  };

export function chipDescriptor(byProvider: Record<Provider, ProviderState>): ChipState {
	void byProvider;
	return { kind: 'hidden' };
}
