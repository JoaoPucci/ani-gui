/**
 * Everything the show page draws from the watch order, in one pure
 * reading: the strip itself, its caption, the masthead eyebrow's
 * position, each tile's progress, the "You're watching" card, and the
 * entries "Similar titles" leaves out because the strip shows them.
 * The page and the strip component only hand these on.
 */
import type { WatchOrder } from './by-start';
import type { WatchOrderData } from './loader';
import type { StripView } from './strip-view';
import type { Progress, WatchEntry, WatchRun } from './types';

/** What the strip's centre says about this entry's place. */
export type StripCaption =
	| { readonly kind: 'position'; readonly position: number; readonly total: number }
	| { readonly kind: 'numeral'; readonly position: number }
	| { readonly kind: 'side' }
	| { readonly kind: 'cycle'; readonly count: number }
	| { readonly kind: 'branch' };

export interface WatchOrderPage {
	readonly order: WatchOrder;
	readonly run: WatchRun;
	readonly view: StripView;
	readonly caption: StripCaption;
	/** "05 of 09" in the masthead eyebrow: only where the run is one line. */
	readonly eyebrow: { readonly position: number; readonly total: number } | null;
	readonly progress: Progress;
	/** The entry the viewer is part-way through, when no tile shows it. */
	readonly watching: { readonly entry: WatchEntry; readonly episode: number } | null;
	/** Ids "Similar titles" leaves out: what the strip shows. */
	readonly similarHidden: ReadonlySet<string>;
	/** The run's titles, which short titles are cut against. */
	readonly titles: readonly string[];
}

/** The order setting as stored: anything but `released` reads as linked. */
export function orderOf(value: string | null | undefined): WatchOrder {
	void value;
	return 'linked';
}

/**
 * The order to draw in once the settings read has settled (`loaded`
 * or `failed`), else null: the strip waits for it rather than
 * re-ordering under the viewer.
 */
export function settledOrder(
	config: { readonly watch_order: string } | null,
	failed: boolean
): WatchOrder | null {
	void config;
	void failed;
	return null;
}

/** The caption for a view; `loop` is the size of the cycle `view` sits in, if any. */
export function captionOf(view: StripView, loop: number): StripCaption {
	void view;
	void loop;
	return { kind: 'branch' };
}

/** The page's reading of `data` for `id`, or null where the page shows no strip. */
export function watchOrderPage(
	data: WatchOrderData,
	id: string,
	order: WatchOrder
): WatchOrderPage | null {
	void data;
	void id;
	void order;
	return null;
}
