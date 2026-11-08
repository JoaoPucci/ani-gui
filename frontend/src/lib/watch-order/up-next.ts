/**
 * The player's "Up next" card at a finale (docs/proposals/
 * franchise-watch-order.md, option b): which entries follow the one
 * just finished, in the order the setting chooses. The card names
 * them whether or not the provider carries them, and nothing here asks
 * the provider.
 */
import { isUnaired } from './entry';
import type { WatchOrderData } from './loader';
import { placement, type WatchOrder } from './order';
import { progressFromHistory } from './progress';
import { runOf } from './strip-model';
import type { Progress, WatchEntry, WatchRun } from './types';

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

/** What an entry says about itself that decides its finale. */
export interface FinaleSelf {
	/** Kitsu's status: `finished` once the entry has finished airing. */
	readonly status: string | null;
	readonly episodes: number | null;
}

/** Whether `episode` is the last of an entry that has finished airing. */
export function isFinale(self: FinaleSelf, episode: number): boolean {
	return self.status === 'finished' && self.episodes !== null && episode >= self.episodes;
}

/** What the card shows: the next entries, and their progress for the meta line. */
export interface FinaleCard {
	readonly upNext: Exclude<UpNext, { readonly kind: 'none' }>;
	readonly progress: Progress;
}

/**
 * The card `id`'s `episode` shows, or null: not a finale, the
 * franchise not read (or read for another id), the settings not
 * settled (`order` null), `id` outside the run, or nothing after it.
 * `self` is the player's own reading of the entry; without one the
 * run's entry decides the finale.
 */
export function finaleCard(
	data: WatchOrderData | null,
	id: string,
	order: WatchOrder | null,
	today: string,
	episode: number,
	self: FinaleSelf | null
): FinaleCard | null {
	if (data?.franchise == null || order === null || data.franchise.anchor !== id) return null;
	const run = runOf(data.franchise);
	const entry = run?.entries.find((e) => e.id === id);
	if (!run || !entry || !isFinale(self ?? entry, episode)) return null;
	const upNext = upNextFor(run, id, order, today);
	if (upNext.kind === 'none') return null;
	const shown = upNext.kind === 'choices' ? upNext.entries : [upNext.entry];
	return { upNext, progress: progressFromHistory(data.rows, shown) };
}
