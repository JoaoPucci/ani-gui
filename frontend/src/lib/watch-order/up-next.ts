/**
 * The player's "Up next" card at a finale (docs/proposals/
 * franchise-watch-order.md, option b): which entries follow the one
 * just finished, in the order the setting chooses. The card names
 * them whether or not the provider carries them, and nothing here asks
 * the provider.
 */
import { chainFranchise } from './chain-view';
import { isUnaired } from './entry';
import type { WatchOrderData } from './loader';
import { placement, type WatchOrder } from './order';
import { progressFromHistory } from './progress';
import { runOf } from './strip-model';
import { franchiseSummary, type FranchiseSummary } from './strip-view';
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
 * franchise not read (or read for another id, or only in part, where
 * the entry that really comes next may be missing), the settings not
 * settled (`order` null), `id` outside the run, or nothing after it.
 * `self` is the player's own reading of the entry; without one the
 * run's entry decides the finale. An entry outside the main story is
 * read on its own chain, as its strip draws it, so the card names the
 * chain's next title, or the main-story title the chain leads into.
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
	if (!data.franchise.complete) return null;
	const run = runOf(chainFranchise(data.franchise) ?? data.franchise);
	const entry = run?.entries.find((e) => e.id === id);
	if (!run || !entry || !isFinale(self ?? entry, episode)) return null;
	const upNext = upNextFor(run, id, order, today);
	if (upNext.kind === 'none') return null;
	const shown = upNext.kind === 'choices' ? upNext.entries : [upNext.entry];
	return { upNext, progress: progressFromHistory(data.rows, shown) };
}

/**
 * The card at the last title of a chain outside the main story that
 * leads nowhere: the franchise page, by its name, fronted by the entry
 * the name comes from, with the poster stack and the counts the strip's
 * franchise link shows.
 */
export interface ChainEndCard {
	/** The franchise's name. */
	readonly name: string;
	/** The id the franchise page is opened for. */
	readonly id: string;
	/** The franchise link's stack: its posters and its counts. */
	readonly summary: FranchiseSummary;
}

/**
 * The chain-end card `id`'s `episode` shows, or null: anywhere the
 * Up next card names something, or on anything but the finale of a
 * chain's last title that leads nowhere, read whole.
 */
export function chainEndCard(
	data: WatchOrderData | null,
	id: string,
	order: WatchOrder | null,
	today: string,
	episode: number,
	self: FinaleSelf | null
): ChainEndCard | null {
	if (finaleCard(data, id, order, today, episode, self) !== null) return null;
	if (data?.franchise == null || order === null || data.franchise.anchor !== id) return null;
	if (!data.franchise.complete) return null;
	const drawn = chainFranchise(data.franchise);
	const run = drawn ? runOf(drawn) : null;
	const entry = run?.entries.find((e) => e.id === id);
	const main = runOf(data.franchise);
	if (!entry || !main || !isFinale(self ?? entry, episode)) return null;
	return {
		name: data.franchise.name,
		id,
		summary: franchiseSummary(main, data.franchise.name_from)
	};
}
