/**
 * The player's "Up next" card at a finale (docs/proposals/
 * franchise-watch-order.md, option b): which entries follow the one
 * just finished, in the order the setting chooses. The card names
 * them whether or not the provider carries them, and nothing here asks
 * the provider.
 */
import { isUnaired } from './entry';
import { placement, type WatchOrder } from './order';
import type { WatchEntry, WatchRun } from './types';

export type UpNext =
	| { readonly kind: 'one'; readonly entry: WatchEntry }
	| { readonly kind: 'unaired'; readonly entry: WatchEntry }
	| { readonly kind: 'choices'; readonly entries: readonly WatchEntry[] }
	| { readonly kind: 'none' };

/** The proposal's end-card rule: one aired, one unaired, several, or none. */
export function upNextFor(run: WatchRun, id: string, order: WatchOrder, today: string): UpNext {
	const after = placement(run, id, order).after;
	if (after.length === 0) return { kind: 'none' };
	if (after.length > 1) return { kind: 'choices', entries: after };
	const entry = after[0];
	return isUnaired(entry, today) ? { kind: 'unaired', entry } : { kind: 'one', entry };
}
